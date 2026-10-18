mod source;
#[cfg(test)]
mod tests;
mod website;

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    net::SocketAddr,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use reqwest::{header, redirect::Policy, Client, Url};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::{io::AsyncWriteExt, sync::watch};

use crate::{image_pipeline::probe_images, models::LoadedImage};
use source::{detect_format, image_filename, is_public_ip, parse_url};

pub(crate) const DEFAULT_MAX_MB: u64 = 50;
pub(crate) const MAX_DOWNLOAD_MB: u64 = 200;
const MAX_URLS: usize = 100;
const MAX_REDIRECTS: usize = 5;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadSettings {
    pub max_megabytes: u64,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            max_megabytes: DEFAULT_MAX_MB,
        }
    }
}

impl DownloadSettings {
    pub(crate) fn validate(self) -> Result<Self, String> {
        if !(1..=MAX_DOWNLOAD_MB).contains(&self.max_megabytes) {
            return Err(format!(
                "Maximum image size must be between 1 and {MAX_DOWNLOAD_MB} MB."
            ));
        }
        Ok(self)
    }

    fn max_bytes(self) -> u64 {
        self.max_megabytes * 1024 * 1024
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ImportStatus {
    Discovering,
    Discovered,
    Downloading,
    Ready,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlImportItem {
    pub index: usize,
    pub url: String,
    pub status: ImportStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
    pub image: Option<LoadedImage>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlImportProgress {
    pub batch_id: String,
    #[serde(flatten)]
    pub item: UrlImportItem,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlImportResponse {
    pub items: Vec<UrlImportItem>,
    pub canceled: bool,
}

#[derive(Clone)]
pub(crate) struct Cancellation(watch::Sender<bool>);

impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}

impl Cancellation {
    pub(crate) fn cancel(&self) {
        self.0.send_replace(true);
    }
    fn is_canceled(&self) -> bool {
        *self.0.borrow()
    }
    async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|canceled| *canceled).await;
    }
}

// A private session directory and lock keep live imports separate from abandoned
// sessions. Normal exit/removal and canceled requests are cleaned up by RAII.
pub(crate) struct DownloadStore {
    files: Mutex<HashMap<String, TempDir>>,
    reserved_names: Mutex<HashSet<String>>,
    _lock: File,
    directory: TempDir,
    #[cfg(test)]
    test_endpoint: Option<SocketAddr>,
    #[cfg(test)]
    timeout: Option<Duration>,
}

impl DownloadStore {
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|_| "The temporary image folder could not be created.")?;
        clean_abandoned_sessions(root);
        let directory = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(root)
            .map_err(|_| "The temporary image folder could not be created.")?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join("session.lock"))
            .map_err(|_| "The temporary image folder could not be protected.")?;
        lock.lock()
            .map_err(|_| "The temporary image folder is unavailable.")?;
        Ok(Self {
            files: Mutex::default(),
            reserved_names: Mutex::default(),
            _lock: lock,
            directory,
            #[cfg(test)]
            test_endpoint: None,
            #[cfg(test)]
            timeout: None,
        })
    }

    pub(crate) fn cli_store() -> Result<Self, String> {
        let root = dirs::cache_dir()
            .ok_or("The image cache folder is unavailable.")?
            .join("com.oli.bulkpixel")
            .join("url-imports");
        Self::new(&root)
    }

    pub(crate) fn release(&self, paths: &[String]) {
        let mut files = self.files.lock().expect("download store poisoned");
        for path in paths {
            // Only paths issued by this service can be removed, never arbitrary files.
            files.remove(path);
        }
    }

    fn cleanup(&self) {
        self.files.lock().expect("download store poisoned").clear();
        // macOS can terminate its event loop without running Rust TLS destructors.
        // Remove partial downloads explicitly as well as the completed files above.
        let _ = fs::remove_dir_all(self.directory.path());
    }

    fn reserve_filename(&self, name: String) -> String {
        let mut names = self.reserved_names.lock().expect("download names poisoned");
        if names.insert(name.clone()) {
            return name;
        }
        let path = Path::new(&name);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = path.extension().unwrap_or_default().to_string_lossy();
        for index in 2_u64.. {
            let candidate = format!("{stem}_{index}.{extension}");
            if names.insert(candidate.clone()) {
                return candidate;
            }
        }
        unreachable!()
    }

    async fn destination(&self, url: &Url) -> Result<Vec<SocketAddr>, String> {
        #[cfg(test)]
        if let Some(endpoint) = self.test_endpoint {
            return Ok(vec![endpoint]);
        }
        let host = url
            .host_str()
            .ok_or("The URL needs a hostname.")?
            .trim_matches(['[', ']']);
        let port = url
            .port_or_known_default()
            .ok_or("The URL has an invalid port.")?;
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| "The image server could not be found. Check the URL and your connection.")?
            .collect::<Vec<_>>();
        validate_addresses(&addresses)?;
        Ok(addresses)
    }
}

fn clean_abandoned_sessions(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("session-")
            || !entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            continue;
        }
        let marker = entry.path().join("session.lock");
        if marker
            .symlink_metadata()
            .is_ok_and(|meta| meta.file_type().is_symlink())
        {
            continue;
        }
        let Ok(lock) = OpenOptions::new().read(true).write(true).open(marker) else {
            continue;
        };
        if lock.try_lock().is_ok() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), String> {
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(
            "The URL resolves to a local or private network address and is blocked.".into(),
        );
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct UrlImportState {
    store: Mutex<Option<Arc<DownloadStore>>>,
    batches: Mutex<HashMap<String, Cancellation>>,
}

impl UrlImportState {
    pub(crate) fn store(&self, root: &Path) -> Result<Arc<DownloadStore>, String> {
        let mut store = self.store.lock().expect("download state poisoned");
        if store.is_none() {
            *store = Some(Arc::new(DownloadStore::new(root)?));
        }
        Ok(store.as_ref().unwrap().clone())
    }

    pub(crate) fn begin(&self, id: &str) -> Result<Cancellation, String> {
        validate_batch_id(id)?;
        let mut batches = self.batches.lock().expect("download batches poisoned");
        // A cancellation may arrive before its import command is scheduled.
        if let Some(token) = batches.get(id) {
            if token.is_canceled() {
                return Ok(token.clone());
            }
            return Err("This import is already running.".into());
        }
        if batches.len() >= 100 {
            return Err("Too many image imports are active. Try again shortly.".into());
        }
        let token = Cancellation::default();
        batches.insert(id.into(), token.clone());
        Ok(token)
    }

    pub(crate) fn cancel(&self, id: &str) -> Result<(), String> {
        validate_batch_id(id)?;
        let mut batches = self.batches.lock().expect("download batches poisoned");
        if batches.len() < 100 || batches.contains_key(id) {
            batches.entry(id.into()).or_default().cancel();
        }
        Ok(())
    }

    pub(crate) fn finish(&self, id: &str) {
        self.batches
            .lock()
            .expect("download batches poisoned")
            .remove(id);
    }

    pub(crate) fn release(&self, paths: &[String]) {
        if let Some(store) = self.store.lock().expect("download state poisoned").as_ref() {
            store.release(paths);
        }
    }

    pub(crate) fn cleanup(&self) {
        for cancel in self
            .batches
            .lock()
            .expect("download batches poisoned")
            .values()
        {
            cancel.cancel();
        }
        if let Some(store) = self.store.lock().expect("download state poisoned").take() {
            store.cleanup();
        }
    }
}

fn validate_batch_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 80 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("The image import identifier is invalid.".into());
    }
    Ok(())
}

pub(crate) async fn import_urls(
    store: &DownloadStore,
    urls: Vec<String>,
    settings: DownloadSettings,
    cancel: &Cancellation,
    progress: impl Fn(UrlImportItem),
) -> Result<UrlImportResponse, String> {
    let settings = settings.validate()?;
    if urls.is_empty() || urls.len() > MAX_URLS {
        return Err(format!(
            "Import between 1 and {MAX_URLS} image or website URLs at a time."
        ));
    }
    let mut seen = HashSet::new();
    let mut pending = urls
        .iter()
        .map(|raw| {
            let parsed = parse_url(raw).and_then(|url| {
                if !seen.insert(url.as_str().to_string()) {
                    return Err("Duplicate URL skipped.".into());
                }
                Ok(url)
            });
            (raw.trim().to_string(), parsed, true)
        })
        .collect::<Vec<_>>();
    let mut items = Vec::with_capacity(urls.len());
    let mut index = 0;
    while index < pending.len() {
        let (raw, parsed, inspect_website) = pending[index].clone();
        let inspect_website = inspect_website
            && parsed
                .as_ref()
                .is_ok_and(|url| !website::is_direct_image(url));
        let mut item = UrlImportItem {
            index,
            url: raw,
            status: if inspect_website {
                ImportStatus::Discovering
            } else {
                ImportStatus::Downloading
            },
            downloaded_bytes: 0,
            total_bytes: None,
            message: if inspect_website {
                "Checking image or website…"
            } else {
                "Downloading…"
            }
            .into(),
            image: None,
        };
        let outcome = if cancel.is_canceled() {
            Err("Import canceled.".into())
        } else if let Ok(url) = parsed {
            progress(item.clone());
            let timeout = REQUEST_TIMEOUT;
            #[cfg(test)]
            let timeout = store.timeout.unwrap_or(timeout);
            let download = download_one(
                store,
                url,
                index,
                settings,
                inspect_website,
                |bytes, total| {
                    let mut update = item.clone();
                    update.downloaded_bytes = bytes;
                    update.total_bytes = total;
                    progress(update);
                },
            );
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("Import canceled.".into()),
                result = tokio::time::timeout(timeout, download) => {
                    result.unwrap_or_else(|_| Err("The download timed out after 30 seconds. Try again.".into()))
                }
            }
        } else {
            Err(parsed.unwrap_err())
        };
        match outcome {
            Ok(DownloadedSource::Image(image)) => {
                item.status = ImportStatus::Ready;
                item.downloaded_bytes = image.file_size;
                item.total_bytes = Some(image.file_size);
                item.message = "Ready".into();
                item.image = Some(image);
            }
            Ok(DownloadedSource::Website(urls)) => {
                let mut added = 0;
                for raw in urls {
                    // Revalidate every extracted URL in the native downloader. Never
                    // recursively crawl images which turn out to be HTML pages.
                    let parsed = parse_url(&raw);
                    let key = parsed.as_ref().map(|url| url.as_str()).unwrap_or(&raw);
                    if seen.insert(key.to_string()) {
                        pending.push((raw, parsed, false));
                        added += 1;
                    }
                }
                if added == 0 {
                    item.status = ImportStatus::Discovered;
                    item.message =
                        "All images on this website are already included in this import.".into();
                } else {
                    item.status = ImportStatus::Discovered;
                    item.message = format!("Found {added} images. Downloading originals…");
                }
            }
            Err(message) => {
                item.status = if cancel.is_canceled() {
                    ImportStatus::Canceled
                } else {
                    ImportStatus::Failed
                };
                item.message = message;
            }
        }
        progress(item.clone());
        items.push(item);
        index += 1;
    }
    Ok(UrlImportResponse {
        items,
        canceled: cancel.is_canceled(),
    })
}

enum DownloadedSource {
    Image(LoadedImage),
    Website(Vec<String>),
}

async fn download_one(
    store: &DownloadStore,
    mut url: Url,
    index: usize,
    settings: DownloadSettings,
    inspect_website: bool,
    progress: impl Fn(u64, Option<u64>),
) -> Result<DownloadedSource, String> {
    let mut response = None;
    let mut seen_redirects = HashSet::new();
    for redirects in 0..=MAX_REDIRECTS {
        if !seen_redirects.insert(url.as_str().to_string()) {
            return Err("The server returned a redirect loop.".into());
        }
        let addresses = store.destination(&url).await?;
        // Pin the validated addresses to this connection, preventing DNS rebinding.
        // Disable proxy discovery and automatic redirects, which could bypass this check.
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(15))
            .resolve_to_addrs(url.host_str().unwrap(), &addresses)
            .user_agent(concat!("BulkPixel/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| "The image downloader could not start.")?;
        let received = client
            .get(url.clone())
            .header(
                header::ACCEPT,
                if inspect_website {
                    "image/*, text/html, application/xhtml+xml"
                } else {
                    "image/*"
                },
            )
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(network_error)?;
        let is_test_endpoint = {
            #[cfg(test)]
            {
                store.test_endpoint.is_some()
            }
            #[cfg(not(test))]
            {
                false
            }
        };
        if !is_test_endpoint
            && !received.remote_addr().is_some_and(|peer| {
                addresses.iter().any(|address| address.ip() == peer.ip()) && is_public_ip(peer.ip())
            })
        {
            return Err("The image server connected to an unsafe network address.".into());
        }
        if received.status().is_redirection() {
            if redirects == MAX_REDIRECTS {
                return Err("The server returned too many redirects (maximum 5).".into());
            }
            let location = received
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or("The server returned a redirect without a valid destination.")?;
            let destination = url
                .join(location)
                .map_err(|_| "The server returned an invalid redirect destination.")?;
            let destination = parse_url(destination.as_str())?;
            if url.scheme() == "https" && destination.scheme() != "https" {
                return Err("The secure image URL redirected to an insecure HTTP address.".into());
            }
            url = destination;
        } else {
            if !received.status().is_success() {
                return Err(format!(
                    "The image server returned HTTP {}.",
                    received.status().as_u16()
                ));
            }
            response = Some(received);
            break;
        }
    }
    let mut response = response.ok_or("The image download did not reach a valid destination.")?;
    let headers = response.headers().clone();
    let total = response.content_length();
    if total.is_some_and(|size| size > settings.max_bytes()) {
        return Err(size_error(settings));
    }
    if headers
        .get(header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return Err("The server returned compressed transfer data instead of an original image. Try a direct image URL.".into());
    }
    let working = tempfile::Builder::new()
        .prefix("image-")
        .tempdir_in(store.directory.path())
        .map_err(|_| "The image could not be saved temporarily.")?;
    let partial = working.path().join("download.part");
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .await
        .map_err(|_| "The image could not be saved temporarily.")?;
    let mut downloaded = 0_u64;
    let mut last_report = std::time::Instant::now();
    while let Some(bytes) = response.chunk().await.map_err(network_error)? {
        downloaded = downloaded
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| size_error(settings))?;
        if downloaded > settings.max_bytes() {
            return Err(size_error(settings));
        }
        file.write_all(&bytes)
            .await
            .map_err(|_| "The image could not be saved temporarily.")?;
        if last_report.elapsed() >= Duration::from_millis(100) {
            progress(downloaded, total);
            last_report = std::time::Instant::now();
        }
    }
    file.flush()
        .await
        .map_err(|_| "The image could not be saved temporarily.")?;
    drop(file);
    if downloaded == 0 {
        return Err("The image server returned an empty response.".into());
    }
    progress(downloaded, total);
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let detected_path = partial.clone();
    let detected =
        tokio::task::spawn_blocking(move || detect_format(&detected_path, &content_type))
            .await
            .map_err(|_| "The image could not be checked.")?;
    let extension = match detected {
        Ok(extension) => extension,
        Err(error) => {
            let content_type = headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            if inspect_website {
                let mut prefix = vec![0; downloaded.min(4096) as usize];
                use tokio::io::AsyncReadExt;
                tokio::fs::File::open(&partial)
                    .await
                    .map_err(|_| "The website could not be read.")?
                    .read_exact(&mut prefix)
                    .await
                    .map_err(|_| "The website could not be read.")?;
                if website::is_html(&prefix, content_type) {
                    if downloaded > 20 * 1024 * 1024 {
                        return Err("Website exceeds the 20 MB page limit.".into());
                    }
                    let urls = website::extract_images(&partial, &url, content_type).await?;
                    if urls.is_empty() {
                        return Err("No supported image URLs were found on this website.".into());
                    }
                    return Ok(DownloadedSource::Website(urls));
                }
            }
            return Err(error);
        }
    };
    let name = store.reserve_filename(image_filename(&headers, &url, extension, index));
    let final_path = working.path().join(name);
    tokio::fs::rename(&partial, &final_path)
        .await
        .map_err(|_| "The image could not be saved temporarily.")?;
    let path = final_path.to_string_lossy().to_string();
    let probe_path = path.clone();
    let probe = tokio::task::spawn_blocking(move || probe_images(vec![probe_path]))
        .await
        .map_err(|_| "The image could not be checked.")?
        .map_err(|_| "The downloaded file is not a valid supported image.")?;
    let image = probe
        .loaded
        .into_iter()
        .next()
        .ok_or("The downloaded file is damaged or is not a supported image.")?;
    store
        .files
        .lock()
        .expect("download store poisoned")
        .insert(path, working);
    Ok(DownloadedSource::Image(image))
}

fn size_error(settings: DownloadSettings) -> String {
    format!(
        "The image exceeds the {} MB download limit.",
        settings.max_megabytes
    )
}

fn network_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "The image download timed out. Try again.".into()
    } else if error.is_connect() {
        "The image server could not be reached. Check your connection and try again.".into()
    } else {
        "The image download was interrupted. Try again.".into()
    }
}
