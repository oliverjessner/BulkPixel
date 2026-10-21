mod app_navigation;
mod browser_host;
mod browser_import;
mod cli;
#[cfg(target_os = "macos")]
mod dock_menu;
mod image_metadata;
mod image_pipeline;
mod magic_directories;
mod models;
mod presets;
mod url_import;

use app_navigation::{AppNavigationState, AppView};
use browser_import::{BrowserImportState, PendingImports};
use image_metadata::inspect_image_metadata;
use image_pipeline::{convert_images, default_output_directory, probe_images};
use magic_directories::{
    delete_magic_directory as delete_magic_directory_from_store,
    list_magic_directories as list_magic_directories_from_store, refresh_watcher,
    save_magic_directory as save_magic_directory_to_store, MagicWatcherState,
};
use models::{
    ConversionPreset, ConversionRequest, ConversionResponse, ConversionStatistics,
    ImageMetadataResponse, MagicDirectory, ProbeImagesResponse, SaveMagicDirectoryRequest,
    SavePresetRequest,
};
use presets::{
    delete_preset as delete_preset_from_store, list_presets as list_presets_from_store,
    load_statistics as load_statistics_from_store,
    record_conversion_statistics as record_conversion_statistics_in_store,
    save_preset as save_preset_to_store,
};
use std::{collections::HashSet, path::PathBuf, sync::Mutex, time::Instant};
use tauri::menu::Menu;
use tauri::{Emitter, Manager};
use url_import::{DownloadSettings, UrlImportProgress, UrlImportResponse, UrlImportState};

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
fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn show_in_finder(path: String) -> Result<(), String> {
    let directory = PathBuf::from(path);
    if !directory.is_dir() {
        return Err("The output folder does not exist or is not a directory.".into());
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(&directory)
            .status()
            .map_err(|error| format!("Unable to open Finder: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("Finder could not open the output folder.".into())
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Showing folders in Finder is only available on macOS.".into())
    }
}

#[tauri::command]
async fn probe_images_command(paths: Vec<String>) -> Result<ProbeImagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || probe_images(paths))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn inspect_image_metadata_command(path: String) -> Result<ImageMetadataResponse, String> {
    tauri::async_runtime::spawn_blocking(move || inspect_image_metadata(path))
        .await
        .map_err(|error| error.to_string())?
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
fn get_statistics(app: tauri::AppHandle) -> Result<ConversionStatistics, String> {
    load_statistics_from_store(&app).map_err(|error| error.to_string())
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
    delete_preset_from_store(&app, id).map_err(|error| error.to_string())?;
    refresh_watcher(&app, &app.state::<MagicWatcherState>())?;
    Ok(())
}

#[tauri::command]
fn list_magic_directories(app: tauri::AppHandle) -> Result<Vec<MagicDirectory>, String> {
    list_magic_directories_from_store(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_magic_directory(
    app: tauri::AppHandle,
    request: SaveMagicDirectoryRequest,
) -> Result<MagicDirectory, String> {
    let directory =
        save_magic_directory_to_store(&app, request).map_err(|error| error.to_string())?;
    refresh_watcher(&app, &app.state::<MagicWatcherState>())?;
    Ok(directory)
}

#[tauri::command]
fn delete_magic_directory(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    delete_magic_directory_from_store(&app, id).map_err(|error| error.to_string())?;
    refresh_watcher(&app, &app.state::<MagicWatcherState>())?;
    Ok(())
}

#[tauri::command]
fn get_opened_files(opened_files: tauri::State<'_, OpenedFilesState>) -> Vec<String> {
    opened_files.take_initial_files()
}

#[tauri::command]
fn get_initial_view(navigation: tauri::State<'_, AppNavigationState>) -> Option<AppView> {
    navigation.take_initial_view()
}

#[tauri::command]
fn take_browser_imports(state: tauri::State<'_, BrowserImportState>) -> PendingImports {
    state.take()
}

#[tauri::command]
async fn import_image_urls(
    app: tauri::AppHandle,
    state: tauri::State<'_, UrlImportState>,
    batch_id: String,
    urls: Vec<String>,
    settings: DownloadSettings,
) -> Result<UrlImportResponse, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|_| "The image cache is unavailable.")?
        .join("url-imports");
    let store = state.store(&root)?;
    let cancel = state.begin(&batch_id)?;
    let response = url_import::import_urls(&store, urls, settings, &cancel, |item| {
        let _ = app.emit_to(
            "main",
            "url-import-progress",
            UrlImportProgress {
                batch_id: batch_id.clone(),
                item,
            },
        );
    })
    .await;
    state.finish(&batch_id);
    response
}

#[tauri::command]
fn cancel_url_import(
    state: tauri::State<'_, UrlImportState>,
    batch_id: String,
) -> Result<(), String> {
    state.cancel(&batch_id)
}

#[tauri::command]
fn release_url_images(state: tauri::State<'_, UrlImportState>, paths: Vec<String>) {
    state.release(&paths);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(OpenedFilesState::default())
        .manage(AppNavigationState::default())
        .manage(BrowserImportState::default())
        .manage(UrlImportState::default())
        .manage(MagicWatcherState::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let menu = Menu::default(app.handle())?;
            app.set_menu(menu)?;
            #[cfg(target_os = "macos")]
            dock_menu::install(app.handle())?;
            if let Err(error) = refresh_watcher(app.handle(), &app.state::<MagicWatcherState>()) {
                eprintln!("failed to start watched folder watcher: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_output_directory,
            get_app_version,
            show_in_finder,
            probe_images_command,
            inspect_image_metadata_command,
            bulk_convert_images,
            get_statistics,
            list_presets,
            save_preset,
            delete_preset,
            list_magic_directories,
            save_magic_directory,
            delete_magic_directory,
            get_opened_files,
            get_initial_view,
            take_browser_imports,
            import_image_urls,
            cancel_url_import,
            release_url_images
        ])
        .build(tauri::generate_context!())
        .expect("error while running BulkPixel");

    app.run(|app, event| {
        if matches!(&event, tauri::RunEvent::Exit) {
            app.state::<UrlImportState>().cleanup();
        }
        #[cfg(target_os = "macos")]
        handle_run_event(app, event);

        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            let _ = event;
        }
    });
}

pub fn run_cli_from_env() -> i32 {
    if let Some(origin) = std::env::args()
        .nth(1)
        .filter(|arg| browser_host::is_native_origin(arg))
    {
        return browser_host::run(&origin);
    }
    cli::run_from_env()
}

#[cfg(target_os = "macos")]
fn navigate_to_view(app: &tauri::AppHandle, view: AppView) {
    let frontend_ready = app.state::<AppNavigationState>().request_view(view);
    focus_main_window(app);
    if frontend_ready {
        if let Err(error) = app.emit_to("main", "navigate-view", view) {
            eprintln!("failed to navigate from the Dock menu: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
fn handle_run_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: tauri::RunEvent) {
    if let tauri::RunEvent::Opened { urls } = event {
        handle_opened_urls(app, urls);
    }
}

#[cfg(target_os = "macos")]
fn handle_opened_urls<R: tauri::Runtime>(app: &tauri::AppHandle<R>, urls: Vec<tauri::Url>) {
    let browser_links: Vec<_> = urls
        .iter()
        .filter(|url| url.scheme() == "bulkpixel")
        .cloned()
        .collect();
    if !browser_links.is_empty() {
        app.state::<BrowserImportState>().enqueue(browser_links);
        focus_main_window(app);
        // The queue is retained until the frontend drains it, including cold starts.
        let _ = app.emit_to("main", "browser-import-pending", ());
    }
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
