use super::*;
use crate::models::{
    CollisionMode, ConversionImageInput, ConversionRequest, ExportFormat, ResizeOptions,
};
use image::{ImageFormat, Rgba, RgbaImage};
use std::{
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

fn png() -> Vec<u8> {
    let pixels = RgbaImage::from_pixel(3, 2, Rgba([40, 80, 120, 128]));
    let mut bytes = Cursor::new(Vec::new());
    pixels.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    delay: Duration,
    chunked: bool,
    disconnect: bool,
}

impl Reply {
    fn body(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            headers: vec![],
            body,
            delay: Duration::ZERO,
            chunked: false,
            disconnect: false,
        }
    }
}

struct Server {
    address: SocketAddr,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(handler: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local HTTP fixture server");
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let seen = requests.clone();
    let handler = Arc::new(handler);
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let handler = handler.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).await.is_err() {
                    return;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                seen.fetch_add(1, Ordering::SeqCst);
                let reply = handler(&path);
                if reply.disconnect {
                    return;
                }
                let mut stream = reader.into_inner();
                let mut headers =
                    format!("HTTP/1.1 {} Test\r\nConnection: close\r\n", reply.status);
                for (key, value) in reply.headers {
                    headers.push_str(&format!("{key}: {value}\r\n"));
                }
                if reply.chunked {
                    headers.push_str("Transfer-Encoding: chunked\r\n");
                } else {
                    headers.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
                }
                headers.push_str("\r\n");
                if stream.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }
                tokio::time::sleep(reply.delay).await;
                for bytes in reply.body.chunks(16384) {
                    if reply.chunked
                        && stream
                            .write_all(format!("{:x}\r\n", bytes.len()).as_bytes())
                            .await
                            .is_err()
                    {
                        return;
                    }
                    if stream.write_all(bytes).await.is_err() {
                        return;
                    }
                    if reply.chunked && stream.write_all(b"\r\n").await.is_err() {
                        return;
                    }
                }
                if reply.chunked {
                    let _ = stream.write_all(b"0\r\n\r\n").await;
                }
                let _ = stream.shutdown().await;
            });
        }
    });
    Server {
        address,
        requests,
        task,
    }
}

fn store(root: &Path, server: &Server) -> DownloadStore {
    let mut store = DownloadStore::new(root).unwrap();
    // Tests pin an otherwise public hostname to their local fixture server.
    // This override is absent from production builds and has no CLI/UI option.
    store.test_endpoint = Some(server.address);
    store
}

fn urls(paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .map(|path| format!("http://images.example.com{path}"))
        .collect()
}

fn assert_no_downloads(store: &DownloadStore) {
    let names = fs::read_dir(store.directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names, ["session.lock"]);
    assert!(store.files.lock().unwrap().is_empty());
}

#[tokio::test]
async fn imports_original_bytes_without_extensions_despite_wrong_content_type() {
    let original = png();
    let data = original.clone();
    let server = server(move |path| {
        assert_eq!(path, "/asset?id=42&width=1200");
        let mut reply = Reply::body(data.clone());
        reply.headers.push(("Content-Type", "text/html".into()));
        reply
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let response = import_urls(
        &store,
        urls(&["/asset?id=42&width=1200"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    let image = response.items[0].image.as_ref().unwrap();
    assert_eq!(fs::read(&image.path).unwrap(), original);
    assert_eq!(image.name, "asset.png");
    assert_eq!((image.width, image.height), (3, 2));
    assert!(image.preview_data_url.starts_with("data:image/png;base64,"));
    let path = image.path.clone();
    store.release(&[path.clone(), "/tmp/never-delete-an-arbitrary-file".into()]);
    assert!(!Path::new(&path).exists());
    assert_no_downloads(&store);
}

#[tokio::test]
async fn follows_relative_redirects_and_uses_final_filename_and_disposition() {
    let server = server(|path| {
        if path == "/start" {
            let mut reply = Reply::body(vec![]);
            reply.status = 302;
            reply.headers.push((
                "Location",
                "/final/photo.webp?secret=never-a-filename".into(),
            ));
            reply
        } else {
            let mut reply = Reply::body(png());
            reply.headers.push((
                "Content-Disposition",
                "attachment; filename=ignored.jpg; filename*=UTF-8''My%20photo.gif".into(),
            ));
            reply
        }
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let response = import_urls(
        &store,
        urls(&["/start"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(
        response.items[0].image.as_ref().unwrap().name,
        "My photo.png"
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn continues_a_partial_batch_and_deduplicates_before_downloading() {
    let server = server(|path| match path {
        "/good" | "/other" => Reply::body(png()),
        "/404" => {
            let mut reply = Reply::body(vec![]);
            reply.status = 404;
            reply
        }
        "/empty" => Reply::body(vec![]),
        _ => {
            let mut reply = Reply::body(b"<html>not an image</html>".to_vec());
            reply.headers.push(("Content-Type", "image/png".into()));
            reply
        }
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let response = import_urls(
        &store,
        urls(&[
            "/good",
            "/good#duplicate",
            "/404",
            "/html",
            "/empty",
            "/other",
        ]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(
        response
            .items
            .iter()
            .filter(|item| item.status == ImportStatus::Ready)
            .count(),
        2
    );
    assert!(response.items[1].message.contains("Duplicate"));
    assert!(response.items[2].message.contains("404"));
    assert!(response.items[3].message.contains("supported image"));
    assert!(response.items[4].message.contains("empty response"));
    assert_eq!(server.requests.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn rejects_redirects_to_private_resources_and_redirect_loops() {
    let server = server(|path| {
        let mut reply = Reply::body(vec![]);
        reply.status = 302;
        let destination = match path {
            "/private" => "http://127.0.0.1/private".to_string(),
            "/file" => "file:///etc/passwd".to_string(),
            "/loop" => "/loop".to_string(),
            _ => format!(
                "/{}",
                path.trim_start_matches('/').parse::<usize>().unwrap_or(0) + 1
            ),
        };
        reply.headers.push(("Location", destination));
        reply
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let response = import_urls(
        &store,
        urls(&["/private", "/file", "/loop", "/0"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert!(response
        .items
        .iter()
        .all(|item| item.status == ImportStatus::Failed));
    assert!(response.items[0].message.contains("blocked"));
    assert!(response.items[1].message.contains("HTTP"));
    assert!(response.items[2].message.contains("loop"));
    assert!(response.items[3].message.contains("too many redirects"));
    assert_no_downloads(&store);
}

#[tokio::test]
async fn enforces_declared_and_streamed_download_limits_and_rejects_bad_images() {
    let server = server(|path| {
        let mut reply = Reply::body(if path == "/broken" {
            b"\x89PNG\r\n\x1a\ninvalid".to_vec()
        } else {
            vec![42; 1024 * 1024 + 1]
        });
        reply.chunked = path == "/chunked";
        reply
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let response = import_urls(
        &store,
        urls(&["/length", "/chunked", "/broken"]),
        DownloadSettings { max_megabytes: 1 },
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert!(response.items[0].message.contains("1 MB"));
    assert!(response.items[1].message.contains("1 MB"));
    assert!(response.items[2].message.contains("damaged"));
    assert_no_downloads(&store);
}

#[tokio::test]
async fn cancel_stops_pending_requests_removes_partial_files_and_keeps_ready_images() {
    let server = server(|path| {
        let mut reply = Reply::body(png());
        if path == "/slow" {
            reply.delay = Duration::from_secs(5);
        }
        reply
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let pending = import_urls(
        &store,
        urls(&["/ready", "/slow", "/not-requested"]),
        DownloadSettings::default(),
        &cancel,
        |_| {},
    );
    let cancellation = async {
        tokio::time::sleep(Duration::from_millis(80)).await;
        signal.cancel();
    };
    let (response, ()) = tokio::join!(pending, cancellation);
    let response = response.unwrap();
    assert!(response.canceled);
    assert_eq!(response.items[0].status, ImportStatus::Ready);
    assert_eq!(response.items[1].status, ImportStatus::Canceled);
    assert_eq!(response.items[2].status, ImportStatus::Canceled);
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    let ready = response.items[0].image.as_ref().unwrap();
    assert!(Path::new(&ready.path).exists());
    assert_eq!(fs::read_dir(store.directory.path()).unwrap().count(), 2);
    store.release(&[ready.path.clone()]);
    assert_no_downloads(&store);
}

#[tokio::test]
async fn handles_timeouts_network_failures_and_retry() {
    let server = server(|path| {
        let mut reply = Reply::body(png());
        if path == "/slow" {
            reply.delay = Duration::from_secs(1);
        }
        if path == "/disconnect" {
            reply.disconnect = true;
        }
        reply
    })
    .await;
    let root = tempfile::tempdir().unwrap();
    let mut store = store(root.path(), &server);
    store.timeout = Some(Duration::from_millis(40));
    let response = import_urls(
        &store,
        urls(&["/slow", "/disconnect", "/retry"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(response.items[0].status, ImportStatus::Failed);
    assert!(response.items[0].message.contains("timed out"));
    assert_eq!(response.items[1].status, ImportStatus::Failed);
    assert_eq!(response.items[2].status, ImportStatus::Ready);
    let response = import_urls(
        &store,
        urls(&["/retry"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(
        response.items[0].image.as_ref().unwrap().name,
        "retry_2.png"
    );
}

#[tokio::test]
async fn imported_images_and_local_images_share_real_conversion_options() {
    let server = server(|_| Reply::body(png())).await;
    let root = tempfile::tempdir().unwrap();
    let store = store(root.path(), &server);
    let imported = import_urls(
        &store,
        urls(&["/remote"]),
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    let local = root.path().join("local.png");
    fs::write(&local, png()).unwrap();
    let paths = vec![
        local.to_string_lossy().to_string(),
        imported.items[0].image.as_ref().unwrap().path.clone(),
    ];
    assert_eq!(probe_images(paths.clone()).unwrap().loaded.len(), 2);
    let output = root.path().join("export");
    let response = crate::image_pipeline::convert_images(ConversionRequest {
        images: paths
            .into_iter()
            .map(|path| ConversionImageInput { path })
            .collect(),
        format: ExportFormat::Png,
        resize: ResizeOptions {
            width: Some(6),
            height: None,
        },
        quality: 90,
        filename_component: "test_".into(),
        filename_mode: "prefix".into(),
        output_dir: output.to_string_lossy().to_string(),
        collision_mode: CollisionMode::Error,
    })
    .unwrap();
    assert_eq!(response.summary.success_count, 2);
    for name in ["test_local.png", "test_remote.png"] {
        let image = image::open(output.join(name)).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (6, 4));
        assert_eq!(image.get_pixel(0, 0).0, [40, 80, 120, 128]);
    }
}

#[test]
fn validates_schemes_credentials_and_private_or_special_networks() {
    for raw in [
        "file:///tmp/a.png",
        "javascript:alert(1)",
        "ftp://example.com/a",
        "http://user:secret@example.com/a",
        "http://localhost/a",
        "http://printer.local/a",
        "http://metadata.google.internal/a",
        "http://127.1/a",
        "http://2130706433/a",
        "http://0x7f000001/a",
        "http://[::ffff:127.0.0.1]/a",
        "http://[::1]/a",
        "http://10.0.0.1/a",
        "http://169.254.169.254/a",
        "http://example.com:0/a",
    ] {
        assert!(parse_url(raw).is_err(), "{raw}");
    }
    for raw in [
        "0.0.0.0",
        "100.64.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "192.0.2.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "fd00::1",
        "fe80::1",
        "2001:db8::1",
        "2002:7f00:1::1",
        "64:ff9b::7f00:1",
    ] {
        assert!(!is_public_ip(raw.parse().unwrap()), "{raw}");
    }
    for raw in [
        "1.1.1.1",
        "8.8.8.8",
        "2001:4860:4860::8888",
        "2606:4700:4700::1111",
    ] {
        assert!(is_public_ip(raw.parse().unwrap()), "{raw}");
    }
    assert!(validate_addresses(&[
        "8.8.8.8:80".parse().unwrap(),
        "10.1.2.3:80".parse().unwrap()
    ])
    .is_err());
    assert!(validate_addresses(&[]).is_err());
    assert_eq!(
        parse_url(" https://example.com/a?id=42#fragment ")
            .unwrap()
            .as_str(),
        "https://example.com/a?id=42"
    );
}

#[test]
fn detects_all_existing_formats_from_content_and_rejects_unsafe_svg_resources() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("download");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (file, expected) in [
        ("multipage16.tiff", "tiff"),
        ("animated.gif", "gif"),
        ("rgb8-codestream.jxl", "jxl"),
        ("rgb8-container.jxl", "jxl"),
        ("multi-size.ico", "ico"),
    ] {
        fs::copy(fixtures.join(file), &path).unwrap();
        assert_eq!(
            detect_format(&path, "application/octet-stream").unwrap(),
            expected
        );
    }
    for (file, expected) in [
        ("test.jp2", "jp2"),
        ("test.heic", "heic"),
        ("ayn_hor_akkutes.webp", "webp"),
    ] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/test_images")
                .join(file),
            &path,
        )
        .unwrap();
        assert_eq!(detect_format(&path, "text/plain").unwrap(), expected);
    }
    for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::Avif] {
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30])))
            .write_to(&mut encoded, format)
            .unwrap();
        fs::write(&path, encoded.into_inner()).unwrap();
        assert_eq!(
            detect_format(&path, "text/html").unwrap(),
            if format == ImageFormat::Jpeg {
                "jpg"
            } else if format == ImageFormat::Png {
                "png"
            } else {
                "avif"
            }
        );
    }
    fs::write(&path, b"<svg xmlns='http://www.w3.org/2000/svg' width='3' height='2'><script>alert(1)</script><rect width='3' height='2'/></svg>").unwrap();
    assert_eq!(
        detect_format(&path, "application/octet-stream").unwrap(),
        "svg"
    );
    for svg in [
        "<svg xmlns='http://www.w3.org/2000/svg'><image href='file:///etc/passwd'/></svg>",
        "<svg xmlns='http://www.w3.org/2000/svg'><image href='http://127.0.0.1/private'/></svg>",
        "<svg xmlns='http://www.w3.org/2000/svg'><image href='data:image/svg+xml;base64,AAAA'/></svg>",
        "<svg xmlns='http://www.w3.org/2000/svg'><style>image{fill:url(file:///secret)}</style></svg>",
        "<svg xmlns='http://www.w3.org/2000/svg'><rect style='fill:u\\72l(file:///secret)'/></svg>",
        "<!DOCTYPE svg SYSTEM 'file:///etc/passwd'><svg xmlns='http://www.w3.org/2000/svg'/>",
        "<html><svg xmlns='http://www.w3.org/2000/svg'/></html>",
    ] {
        fs::write(&path, svg).unwrap(); assert!(detect_format(&path, "image/svg+xml").is_err(), "{svg}");
    }
    fs::write(&path, b"\0\0\0\x01ftyp........").unwrap();
    assert!(detect_format(&path, "image/avif").is_err());
}

#[test]
fn filenames_cannot_escape_storage_or_inherit_url_queries() {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"../../escape.jpg\"".parse().unwrap(),
    );
    let url = Url::parse("https://example.com/path/original.php?name=secret.png").unwrap();
    assert_eq!(image_filename(&headers, &url, "png", 0), "escape.png");
    headers.clear();
    assert_eq!(image_filename(&headers, &url, "png", 0), "original.png");
    assert_eq!(
        image_filename(
            &headers,
            &Url::parse("https://example.com/?name=secret.png").unwrap(),
            "jpg",
            4
        ),
        "image-005.jpg"
    );
}

#[test]
fn removes_only_abandoned_sessions_and_cleans_up_when_the_store_drops() {
    let root = tempfile::tempdir().unwrap();
    let abandoned = root.path().join("session-abandoned");
    fs::create_dir(&abandoned).unwrap();
    fs::write(abandoned.join("session.lock"), b"").unwrap();
    fs::write(abandoned.join("download.part"), b"partial").unwrap();
    let first = DownloadStore::new(root.path()).unwrap();
    assert!(!abandoned.exists());
    let active = first.directory.path().to_path_buf();
    let second = DownloadStore::new(root.path()).unwrap();
    assert!(active.exists());
    drop(first);
    assert!(!active.exists());
    let other = second.directory.path().to_path_buf();
    drop(second);
    assert!(!other.exists());
}

#[test]
fn handles_cancel_before_start_and_rejects_reused_active_batch_ids() {
    let state = UrlImportState::default();
    state.cancel("batch-before-start").unwrap();
    assert!(state.begin("batch-before-start").unwrap().is_canceled());
    state.finish("batch-before-start");
    let token = state.begin("batch-running").unwrap();
    assert!(state.begin("batch-running").is_err());
    state.cancel("batch-running").unwrap();
    assert!(token.is_canceled());
    assert!(state.begin("../../invalid").is_err());
    assert!(DownloadSettings { max_megabytes: 0 }.validate().is_err());
    assert!(DownloadSettings { max_megabytes: 201 }.validate().is_err());
}

#[tokio::test]
async fn shutdown_cancels_active_batches_and_removes_session_files_with_live_handles() {
    let server = server(|_| Reply::body(png())).await;
    let root = tempfile::tempdir().unwrap();
    let mut store = store(root.path(), &server);
    store.test_endpoint = Some(server.address);
    let store = Arc::new(store);
    let state = UrlImportState::default();
    *state.store.lock().unwrap() = Some(store.clone());
    let token = state.begin("shutdown-test").unwrap();
    let response = import_urls(
        &store,
        urls(&["/ready"]),
        DownloadSettings::default(),
        &token,
        |_| {},
    )
    .await
    .unwrap();
    let image = response.items[0].image.as_ref().unwrap();
    assert!(Path::new(&image.path).exists());
    state.cleanup();
    assert!(token.is_canceled());
    assert!(!Path::new(&image.path).exists());
    assert!(!store.directory.path().exists());
}
