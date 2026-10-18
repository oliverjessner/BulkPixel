//! Refresh the source-backed website screenshot fixture with the real importer.
//! cargo run --manifest-path src-tauri/Cargo.toml --example capture_website_import --
//!   https://oliverjessner.at tests/fixtures/screenshots/website-import.json
#![allow(dead_code)]

mod image_pipeline;
mod models;
mod url_import;

use std::{fs::OpenOptions, io::Write, path::PathBuf};
use url_import::{Cancellation, DownloadSettings, DownloadStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        return Err("Usage: capture_website_import <website-url> <new-fixture.json>".into());
    }
    let store = DownloadStore::cli_store()?;
    let response = url_import::import_urls(
        &store,
        vec![arguments[0].clone()],
        DownloadSettings::default(),
        &Cancellation::default(),
        |_| {},
    )
    .await?;
    if !response.items.iter().any(|item| item.image.is_some()) {
        return Err("The website import did not load any images.".into());
    }
    let mut fixture = serde_json::json!({ "sourceUrl": arguments[0], "response": response });
    for item in fixture["response"]["items"].as_array_mut().unwrap() {
        if let Some(image) = item["image"].as_object_mut() {
            // The screenshot uses embedded previews; never persist disposable cache paths.
            image.insert(
                "path".into(),
                format!("/website-import/{}", image["name"].as_str().unwrap()).into(),
            );
        }
    }
    let output = PathBuf::from(&arguments[1]);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?
        .write_all(&serde_json::to_vec_pretty(&fixture)?)?;
    println!(
        "Saved {} imported images to {}",
        fixture["response"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| !item["image"].is_null())
            .count(),
        output.display()
    );
    Ok(())
}
