use std::{path::Path, process::Stdio};

use reqwest::Url;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) fn is_direct_image(url: &Url) -> bool {
    let name = url.path().rsplit('/').next().unwrap_or_default();
    let extension = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    extension.is_some_and(|ext| crate::image_pipeline::SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
}

pub(super) fn is_html(bytes: &[u8], content_type: &str) -> bool {
    let prefix = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    let prefix = prefix.trim_start_matches('\u{feff}').trim_start();
    prefix.starts_with("<!doctype html")
        || prefix.starts_with("<html")
        || content_type
            .split(';')
            .next()
            .is_some_and(|mime| matches!(mime.trim(), "text/html" | "application/xhtml+xml"))
}

#[derive(Deserialize)]
struct Extraction {
    #[serde(default)]
    urls: Vec<String>,
    error: Option<String>,
}

pub(super) async fn extract_images(
    path: &Path,
    url: &Url,
    content_type: &str,
) -> Result<Vec<String>, String> {
    let executable =
        std::env::current_exe().map_err(|_| "Website image extraction is unavailable.")?;
    let directory = executable
        .parent()
        .ok_or("Website image extraction is unavailable.")?;
    let bundled = directory.join("bulkpixel-node");
    let (node, script) = if bundled.is_file() {
        let resources = if directory.file_name().is_some_and(|name| name == "MacOS") {
            directory.join("../Resources")
        } else {
            directory.to_path_buf()
        };
        (bundled, resources.join("url-import/extract.cjs"))
    } else {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target = if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        (
            root.join(format!("binaries/bulkpixel-node-{target}")),
            root.join("resources/url-import/extract.cjs"),
        )
    };
    let mut child = tokio::process::Command::new(node)
        .arg(script)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "The bundled Fetchary runtime could not start.")?;
    let request = serde_json::to_vec(&serde_json::json!({
        "path": path, "url": url.as_str(), "contentType": content_type,
    }))
    .map_err(|_| "Website image extraction could not start.")?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Website image extraction could not start.")?;
    stdin
        .write_all(&request)
        .await
        .map_err(|_| "Website image extraction was interrupted.")?;
    drop(stdin);
    let mut output = Vec::new();
    child
        .stdout
        .take()
        .ok_or("Website image extraction could not start.")?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut output)
        .await
        .map_err(|_| "Website image extraction was interrupted.")?;
    if output.len() > 16 * 1024 * 1024 {
        return Err("Image URL list exceeds the 16 MB limit.".into());
    }
    let status = child
        .wait()
        .await
        .map_err(|_| "Website image extraction was interrupted.")?;
    let result: Extraction = serde_json::from_slice(&output)
        .map_err(|_| "Fetchary could not extract images from this website.")?;
    if let Some(error) = result.error {
        return Err(error);
    }
    if !status.success() {
        return Err("Fetchary could not extract images from this website.".into());
    }
    Ok(result.urls)
}
