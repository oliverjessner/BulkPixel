mod image_pipeline;
mod models;
mod presets;

use image_pipeline::{convert_images, default_output_directory, probe_images};
use models::{
    ConversionPreset, ConversionRequest, ConversionResponse, ProbeImagesResponse, SavePresetRequest,
};
use presets::{
    delete_preset as delete_preset_from_store, list_presets as list_presets_from_store,
    record_conversion_statistics as record_conversion_statistics_in_store,
    save_preset as save_preset_to_store,
};
use std::{collections::HashSet, sync::Mutex, time::Instant};
use tauri::menu::Menu;
#[cfg(target_os = "macos")]
use tauri::{Emitter, Manager};

#[derive(Default)]
struct OpenedFilesState {
    inner: Mutex<OpenedFilesInner>,
}

#[derive(Default)]
struct OpenedFilesInner {
    pending_paths: Vec<String>,
    frontend_ready: bool,
}

impl OpenedFilesState {
    fn take_initial_files(&self) -> Vec<String> {
        let mut inner = self.inner.lock().expect("opened files state poisoned");
        inner.frontend_ready = true;
        std::mem::take(&mut inner.pending_paths)
    }

    fn queue_before_frontend_ready(&self, paths: &[String]) {
        let mut inner = self.inner.lock().expect("opened files state poisoned");
        if inner.frontend_ready {
            return;
        }

        let mut known_paths = inner
            .pending_paths
            .iter()
            .cloned()
            .collect::<HashSet<String>>();
        for path in paths {
            if known_paths.insert(path.clone()) {
                inner.pending_paths.push(path.clone());
            }
        }
    }
}

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
async fn bulk_convert_images(
    app: tauri::AppHandle,
    request: ConversionRequest,
) -> Result<ConversionResponse, String> {
    let format = request.format.clone();
    let started_at = Instant::now();
    let response = tauri::async_runtime::spawn_blocking(move || convert_images(request))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

    if let Err(error) = record_conversion_statistics_in_store(
        &app,
        &format,
        &response.summary,
        started_at.elapsed().as_millis(),
    ) {
        eprintln!("failed to update conversion statistics: {error}");
    }

    Ok(response)
}

#[tauri::command]
fn list_presets(app: tauri::AppHandle) -> Result<Vec<ConversionPreset>, String> {
    list_presets_from_store(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_preset(
    app: tauri::AppHandle,
    request: SavePresetRequest,
) -> Result<ConversionPreset, String> {
    save_preset_to_store(&app, request).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_preset(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    delete_preset_from_store(&app, id).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_opened_files(opened_files: tauri::State<'_, OpenedFilesState>) -> Vec<String> {
    opened_files.take_initial_files()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(OpenedFilesState::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let menu = Menu::default(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_output_directory,
            probe_images_command,
            bulk_convert_images,
            list_presets,
            save_preset,
            delete_preset,
            get_opened_files
        ])
        .build(tauri::generate_context!())
        .expect("error while running BulkPixel");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        handle_run_event(app, event);

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            let _ = event;
        }
    });
}

#[cfg(target_os = "macos")]
fn handle_run_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: tauri::RunEvent) {
    if let tauri::RunEvent::Opened { urls } = event {
        handle_opened_urls(app, urls);
    }
}

#[cfg(target_os = "macos")]
fn handle_opened_urls<R: tauri::Runtime>(app: &tauri::AppHandle<R>, urls: Vec<tauri::Url>) {
    let paths = local_paths_from_urls(urls);
    if paths.is_empty() {
        return;
    }

    app.state::<OpenedFilesState>()
        .queue_before_frontend_ready(&paths);
    focus_main_window(app);

    if let Err(error) = app.emit("opened-files", &paths) {
        eprintln!("failed to emit opened-files event: {error}");
    }
}

#[cfg(target_os = "macos")]
fn local_paths_from_urls(urls: Vec<tauri::Url>) -> Vec<String> {
    let mut seen_paths = HashSet::new();
    urls.into_iter()
        .filter_map(|url| {
            if url.scheme() != "file" {
                return None;
            }

            let path = url.to_file_path().ok()?.to_string_lossy().to_string();
            if seen_paths.insert(path.clone()) {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
