mod image_pipeline;
mod models;

use image_pipeline::{convert_images, default_output_directory, probe_images};
use models::{ConversionRequest, ConversionResponse, ProbeImagesResponse};
use tauri::menu::Menu;

#[tauri::command]
async fn get_default_output_directory() -> Result<String, String> {
    Ok(default_output_directory())
}

#[tauri::command]
async fn probe_images_command(paths: Vec<String>) -> Result<ProbeImagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || probe_images(paths))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn bulk_convert_images(request: ConversionRequest) -> Result<ConversionResponse, String> {
    tauri::async_runtime::spawn_blocking(move || convert_images(request))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let menu = Menu::default(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_output_directory,
            probe_images_command,
            bulk_convert_images
        ])
        .run(tauri::generate_context!())
        .expect("error while running BulkPixel");
}
