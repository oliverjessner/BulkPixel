use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Emitter};

use crate::{
    image_pipeline::convert_images,
    models::{
        validate_multiple_preset_markers, CollisionMode, ConversionImageInput, ConversionPreset,
        ConversionRequest, ExportFormat, MagicDirectory, MagicDirectoryEvent, ResizeOptions,
        SaveMagicDirectoryRequest,
    },
    presets::{
        get_presets_by_ids, get_presets_by_ids_with_connection, open_cli_connection,
        open_connection, record_conversion_statistics, record_watched_folder_conversions,
        PresetError,
    },
};

const VALID_WATCH_FORMATS: &[&str] = &[
    "svg", "jpeg", "png", "webp", "avif", "heic", "tiff", "gif", "jxl", "jp2",
];
const EVENT_DEBOUNCE: Duration = Duration::from_millis(800);
const FILE_READY_POLL: Duration = Duration::from_millis(250);
const FILE_READY_ATTEMPTS: usize = 40;
const IGNORED_OUTPUT_TTL: Duration = Duration::from_secs(60);
const MAX_CASCADE_DEPTH: usize = 8;

static NEXT_CHAIN_ID: AtomicU64 = AtomicU64::new(1);

type PendingPaths = Arc<Mutex<HashMap<PathBuf, u64>>>;
type IgnoredPaths = Arc<Mutex<HashMap<PathBuf, Instant>>>;
type ConversionLock = Arc<Mutex<()>>;

#[derive(Clone, Debug)]
struct CascadeContext {
    id: u64,
    visited_rule_ids: Vec<i64>,
}

impl CascadeContext {
    fn start() -> Self {
        Self {
            id: NEXT_CHAIN_ID.fetch_add(1, Ordering::Relaxed),
            visited_rule_ids: Vec::new(),
        }
    }

    fn advance(&self, rule_id: i64) -> Result<Self, CascadeStop> {
        if self.visited_rule_ids.contains(&rule_id) {
            return Err(CascadeStop::Cycle);
        }
        if self.visited_rule_ids.len() >= MAX_CASCADE_DEPTH {
            return Err(CascadeStop::MaximumDepth);
        }

        let mut visited_rule_ids = self.visited_rule_ids.clone();
        visited_rule_ids.push(rule_id);
        Ok(Self {
            id: self.id,
            visited_rule_ids,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CascadeStop {
    Cycle,
    MaximumDepth,
}

struct CascadeJob {
    path: PathBuf,
    context: CascadeContext,
}

struct PresetRunResult {
    success_count: usize,
    failure_count: usize,
    output_paths: Vec<PathBuf>,
}

pub struct MagicWatcherState {
    watcher: Mutex<Option<RecommendedWatcher>>,
    pending_paths: PendingPaths,
    ignored_paths: IgnoredPaths,
    conversion_lock: ConversionLock,
}

impl Default for MagicWatcherState {
    fn default() -> Self {
        Self {
            watcher: Mutex::new(None),
            pending_paths: Arc::new(Mutex::new(HashMap::new())),
            ignored_paths: Arc::new(Mutex::new(HashMap::new())),
            conversion_lock: Arc::new(Mutex::new(())),
        }
    }
}

pub fn list_magic_directories(app: &AppHandle) -> Result<Vec<MagicDirectory>, PresetError> {
    let connection = open_connection(app)?;
    list_magic_directories_with_connection(&connection)
}

pub fn list_magic_directories_for_cli() -> Result<Vec<MagicDirectory>, PresetError> {
    let connection = open_cli_connection()?;
    list_magic_directories_with_connection(&connection)
}

fn list_magic_directories_with_connection(
    connection: &Connection,
) -> Result<Vec<MagicDirectory>, PresetError> {
    let mut statement = connection.prepare(
        "SELECT id, name, path, enabled, overwrite, created_at, updated_at
         FROM magic_directories
         ORDER BY lower(name) ASC, id ASC",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    rows.into_iter()
        .map(
            |(id, name, path, enabled, overwrite, created_at, updated_at)| {
                Ok(MagicDirectory {
                    id,
                    name,
                    path,
                    formats: load_formats(connection, id)?,
                    preset_ids: load_preset_ids(connection, id)?,
                    enabled,
                    overwrite,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect()
}

pub fn save_magic_directory(
    app: &AppHandle,
    request: SaveMagicDirectoryRequest,
) -> Result<MagicDirectory, PresetError> {
    let mut connection = open_connection(app)?;
    save_magic_directory_with_connection(&mut connection, request)
}

pub fn save_magic_directory_for_cli(
    request: SaveMagicDirectoryRequest,
) -> Result<MagicDirectory, PresetError> {
    let mut connection = open_cli_connection()?;
    save_magic_directory_with_connection(&mut connection, request)
}

fn save_magic_directory_with_connection(
    connection: &mut Connection,
    mut request: SaveMagicDirectoryRequest,
) -> Result<MagicDirectory, PresetError> {
    normalize_and_validate_request(connection, &mut request)?;

    let transaction = connection.transaction()?;
    let id = match request.id {
        Some(id) => {
            let changed = transaction.execute(
                "UPDATE magic_directories
                 SET name = ?1, path = ?2, enabled = ?3, overwrite = ?4,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![
                    request.name,
                    request.path,
                    request.enabled,
                    request.overwrite,
                    id
                ],
            )?;
            if changed == 0 {
                return Err(PresetError::Validation("Watched folder not found.".into()));
            }
            id
        }
        None => {
            transaction.execute(
                "INSERT INTO magic_directories (name, path, enabled, overwrite)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    request.name,
                    request.path,
                    request.enabled,
                    request.overwrite
                ],
            )?;
            transaction.last_insert_rowid()
        }
    };

    transaction.execute(
        "DELETE FROM magic_directory_formats WHERE magic_directory_id = ?1",
        params![id],
    )?;
    transaction.execute(
        "DELETE FROM magic_directory_presets WHERE magic_directory_id = ?1",
        params![id],
    )?;

    for format in &request.formats {
        transaction.execute(
            "INSERT INTO magic_directory_formats (magic_directory_id, format) VALUES (?1, ?2)",
            params![id, format],
        )?;
    }
    for (position, preset_id) in request.preset_ids.iter().enumerate() {
        transaction.execute(
            "INSERT INTO magic_directory_presets (magic_directory_id, preset_id, position)
             VALUES (?1, ?2, ?3)",
            params![id, preset_id, position as i64],
        )?;
    }

    transaction.commit()?;
    get_magic_directory(connection, id)
}

pub fn delete_magic_directory(app: &AppHandle, id: i64) -> Result<(), PresetError> {
    let connection = open_connection(app)?;
    delete_magic_directory_with_connection(&connection, id)
}

pub fn delete_magic_directory_for_cli(id: i64) -> Result<(), PresetError> {
    let connection = open_cli_connection()?;
    delete_magic_directory_with_connection(&connection, id)
}

fn delete_magic_directory_with_connection(
    connection: &Connection,
    id: i64,
) -> Result<(), PresetError> {
    let changed = connection.execute("DELETE FROM magic_directories WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(PresetError::Validation("Watched folder not found.".into()));
    }
    Ok(())
}

fn get_magic_directory(connection: &Connection, id: i64) -> Result<MagicDirectory, PresetError> {
    let row = connection
        .query_row(
            "SELECT id, name, path, enabled, overwrite, created_at, updated_at
             FROM magic_directories WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| PresetError::Validation("Watched folder not found.".into()))?;

    Ok(MagicDirectory {
        id: row.0,
        name: row.1,
        path: row.2,
        formats: load_formats(connection, id)?,
        preset_ids: load_preset_ids(connection, id)?,
        enabled: row.3,
        overwrite: row.4,
        created_at: row.5,
        updated_at: row.6,
    })
}

fn load_formats(connection: &Connection, id: i64) -> Result<Vec<String>, PresetError> {
    let mut statement = connection.prepare(
        "SELECT format FROM magic_directory_formats
         WHERE magic_directory_id = ?1
         ORDER BY CASE format
            WHEN 'svg' THEN 1 WHEN 'jpeg' THEN 2 WHEN 'png' THEN 3
            WHEN 'webp' THEN 4 WHEN 'avif' THEN 5 WHEN 'heic' THEN 6
            WHEN 'tiff' THEN 7 WHEN 'gif' THEN 8 WHEN 'jxl' THEN 9 WHEN 'jp2' THEN 10 END",
    )?;
    let formats = statement
        .query_map(params![id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(formats)
}

fn load_preset_ids(connection: &Connection, id: i64) -> Result<Vec<i64>, PresetError> {
    let mut statement = connection.prepare(
        "SELECT preset_id FROM magic_directory_presets
         WHERE magic_directory_id = ?1 ORDER BY position ASC",
    )?;
    let preset_ids = statement
        .query_map(params![id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(preset_ids)
}

fn normalize_and_validate_request(
    connection: &Connection,
    request: &mut SaveMagicDirectoryRequest,
) -> Result<(), PresetError> {
    request.name = request.name.trim().to_string();
    if request.name.is_empty() {
        return Err(PresetError::Validation(
            "Enter a name for the watched folder.".into(),
        ));
    }

    let raw_path = PathBuf::from(request.path.trim());
    let canonical_path = fs::canonicalize(&raw_path).map_err(|error| {
        PresetError::Validation(format!("Unable to access the watched folder: {error}"))
    })?;
    if !canonical_path.is_dir() {
        return Err(PresetError::Validation(
            "Choose an existing directory to watch.".into(),
        ));
    }
    request.path = canonical_path.to_string_lossy().to_string();

    let mut seen_formats = HashSet::new();
    request.formats = request
        .formats
        .iter()
        .map(|format| normalize_watch_format(format))
        .filter(|format| seen_formats.insert(format.clone()))
        .collect();
    if request.formats.is_empty() {
        return Err(PresetError::Validation(
            "Choose at least one file format to watch.".into(),
        ));
    }
    if request
        .formats
        .iter()
        .any(|format| !VALID_WATCH_FORMATS.contains(&format.as_str()))
    {
        return Err(PresetError::Validation(
            "Choose only SVG, JPEG (JPG), PNG, WEBP, AVIF, HEIC (HEIF), TIFF (TIF), GIF, JPEG XL (JXL), or JPEG 2000 (JP2) as watched formats."
                .into(),
        ));
    }

    let mut seen_presets = HashSet::new();
    request.preset_ids.retain(|id| seen_presets.insert(*id));
    if request.preset_ids.is_empty() {
        return Err(PresetError::Validation(
            "Choose at least one preset to run.".into(),
        ));
    }
    let presets = get_presets_by_ids_with_connection(connection, &request.preset_ids)?;
    if presets.len() > 1 {
        validate_multiple_preset_markers(&presets).map_err(PresetError::Validation)?;
    }

    Ok(())
}

pub fn refresh_watcher(app: &AppHandle, state: &MagicWatcherState) -> Result<(), String> {
    let directories = list_magic_directories(app).map_err(|error| error.to_string())?;
    let app_handle = app.clone();
    let pending_paths = Arc::clone(&state.pending_paths);
    let ignored_paths = Arc::clone(&state.ignored_paths);
    let conversion_lock = Arc::clone(&state.conversion_lock);
    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<Event>| match result {
            Ok(event) if should_process_event(&event.kind) => {
                for path in event.paths {
                    schedule_path(
                        app_handle.clone(),
                        Arc::clone(&pending_paths),
                        Arc::clone(&ignored_paths),
                        Arc::clone(&conversion_lock),
                        path,
                    );
                }
            }
            Ok(_) => {}
            Err(error) => emit_event(
                &app_handle,
                "error",
                format!("Watched folder error: {error}"),
                None,
                false,
            ),
        },
        Config::default(),
    )
    .map_err(|error| error.to_string())?;

    let mut watched_count = 0_usize;
    for directory in directories.iter().filter(|directory| directory.enabled) {
        match watcher.watch(Path::new(&directory.path), RecursiveMode::NonRecursive) {
            Ok(()) => watched_count += 1,
            Err(error) => emit_event(
                app,
                "error",
                format!("Unable to watch {}: {error}", directory.path),
                Some(directory.path.clone()),
                false,
            ),
        }
    }

    *state
        .watcher
        .lock()
        .map_err(|_| "Watched folder state is unavailable.")? = if watched_count == 0 {
        None
    } else {
        Some(watcher)
    };
    Ok(())
}

fn should_process_event(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_))
}

fn schedule_path(
    app: AppHandle,
    pending_paths: PendingPaths,
    ignored_paths: IgnoredPaths,
    conversion_lock: ConversionLock,
    path: PathBuf,
) {
    if watched_extension(&path).is_none() {
        return;
    }

    static NEXT_EVENT_TOKEN: AtomicU64 = AtomicU64::new(1);
    let token = NEXT_EVENT_TOKEN.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut pending) = pending_paths.lock() {
        pending.insert(path.clone(), token);
    } else {
        return;
    }

    thread::spawn(move || {
        thread::sleep(EVENT_DEBOUNCE);
        let is_latest = pending_paths
            .lock()
            .ok()
            .and_then(|pending| pending.get(&path).copied())
            == Some(token);
        if !is_latest {
            return;
        }

        if wait_until_ready(&path) {
            let normalized = normalize_existing_path(&path);
            if !is_ignored_path(&ignored_paths, &normalized) {
                if let Ok(_conversion_guard) = conversion_lock.lock() {
                    if !is_ignored_path(&ignored_paths, &normalized) {
                        process_magic_path(&app, &normalized, &ignored_paths);
                    }
                }
            }
        }

        if let Ok(mut pending) = pending_paths.lock() {
            if pending.get(&path).copied() == Some(token) {
                pending.remove(&path);
            }
        }
    });
}

fn wait_until_ready(path: &Path) -> bool {
    let mut stable_observations = 0_usize;
    let mut previous_size = None;

    for _ in 0..FILE_READY_ATTEMPTS {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => {
                let size = metadata.len();
                if Some(size) == previous_size {
                    stable_observations += 1;
                    if stable_observations >= 2 {
                        return true;
                    }
                } else {
                    stable_observations = 0;
                    previous_size = Some(size);
                }
            }
            _ => {
                stable_observations = 0;
                previous_size = None;
            }
        }
        thread::sleep(FILE_READY_POLL);
    }

    false
}

fn process_magic_path(app: &AppHandle, path: &Path, ignored_paths: &IgnoredPaths) {
    let mut queue = VecDeque::from([CascadeJob {
        path: path.to_path_buf(),
        context: CascadeContext::start(),
    }]);

    while let Some(job) = queue.pop_front() {
        process_magic_job(app, job, ignored_paths, &mut queue);
    }
}

fn process_magic_job(
    app: &AppHandle,
    job: CascadeJob,
    ignored_paths: &IgnoredPaths,
    queue: &mut VecDeque<CascadeJob>,
) {
    let directories = match list_magic_directories(app) {
        Ok(directories) => directories,
        Err(error) => {
            emit_event(
                app,
                "error",
                error.to_string(),
                path_string(&job.path),
                false,
            );
            return;
        }
    };
    let Some(directory) = matching_magic_directory(&directories, &job.path) else {
        return;
    };
    let context = match job.context.advance(directory.id) {
        Ok(context) => context,
        Err(CascadeStop::Cycle) => {
            emit_event(
                app,
                "warning",
                format!(
                    "Cascade stopped for {}: the '{}' rule was already used in this chain.",
                    display_file_name(&job.path),
                    directory.name
                ),
                path_string(&job.path),
                false,
            );
            eprintln!(
                "watched folder cascade {} stopped because rule {} would repeat",
                job.context.id, directory.id
            );
            return;
        }
        Err(CascadeStop::MaximumDepth) => {
            emit_event(
                app,
                "warning",
                format!(
                    "Cascade stopped for {} after {MAX_CASCADE_DEPTH} Watched Folder rules.",
                    display_file_name(&job.path)
                ),
                path_string(&job.path),
                false,
            );
            eprintln!(
                "watched folder cascade {} reached the maximum depth",
                job.context.id
            );
            return;
        }
    };

    let presets = match get_presets_by_ids(app, &directory.preset_ids) {
        Ok(presets) => presets,
        Err(error) => {
            emit_event(
                app,
                "error",
                error.to_string(),
                path_string(&job.path),
                false,
            );
            return;
        }
    };
    if presets.is_empty() {
        emit_event(
            app,
            "error",
            "Watched folder has no available presets. Edit the rule before using it.".into(),
            path_string(&job.path),
            false,
        );
        return;
    }
    if presets.len() > 1 {
        if let Err(error) = validate_multiple_preset_markers(&presets) {
            emit_event(app, "error", error, path_string(&job.path), false);
            return;
        }
    }

    emit_event(
        app,
        "info",
        format!(
            "Processing {} with {} preset(s)...",
            display_file_name(&job.path),
            presets.len()
        ),
        path_string(&job.path),
        true,
    );

    let mut success_count = 0_usize;
    let mut failure_count = 0_usize;
    let mut output_paths = Vec::new();
    for preset in presets {
        match run_preset(app, &job.path, &preset, directory.overwrite, ignored_paths) {
            Ok(result) => {
                success_count += result.success_count;
                failure_count += result.failure_count;
                output_paths.extend(result.output_paths);
            }
            Err(error) => {
                failure_count += 1;
                emit_event(app, "error", error, path_string(&job.path), true);
            }
        }
    }

    let (kind, message) = if failure_count == 0 {
        (
            "success",
            format!("Watched folder converted {success_count} output(s)."),
        )
    } else if success_count > 0 {
        (
            "warning",
            format!("Watched folder created {success_count} output(s); {failure_count} failed."),
        )
    } else {
        ("error", "Watched folder conversion failed.".into())
    };
    emit_event(app, kind, message, path_string(&job.path), false);

    queue.extend(output_paths.into_iter().map(|path| CascadeJob {
        path,
        context: context.clone(),
    }));
}

fn run_preset(
    app: &AppHandle,
    path: &Path,
    preset: &ConversionPreset,
    overwrite: bool,
    ignored_paths: &IgnoredPaths,
) -> Result<PresetRunResult, String> {
    let request = preset_conversion_request(path, preset, overwrite)?;
    let format = request.format.clone();
    let started_at = Instant::now();
    let response = convert_images(request).map_err(|error| error.to_string())?;
    let output_paths = response
        .results
        .iter()
        .filter_map(|result| result.output_path.as_deref())
        .map(Path::new)
        .map(normalize_existing_path)
        .collect::<Vec<_>>();
    for output_path in &output_paths {
        ignore_output_path(ignored_paths, output_path);
    }
    if let Err(error) = record_conversion_statistics(
        app,
        &format,
        &response.summary,
        started_at.elapsed().as_millis(),
    ) {
        eprintln!("failed to update watched folder statistics: {error}");
    }
    if let Err(error) = record_watched_folder_conversions(app, response.summary.success_count) {
        eprintln!("failed to update watched folder usage statistics: {error}");
    }

    Ok(PresetRunResult {
        success_count: response.summary.success_count,
        failure_count: response.summary.failure_count,
        output_paths,
    })
}

fn preset_conversion_request(
    path: &Path,
    preset: &ConversionPreset,
    overwrite: bool,
) -> Result<ConversionRequest, String> {
    let format = ExportFormat::from_value(&preset.format)
        .ok_or_else(|| format!("Unsupported preset format: {}", preset.format))?;
    let resize = match preset.resize_mode.as_str() {
        "width" => ResizeOptions {
            width: preset.width,
            height: None,
        },
        "height" => ResizeOptions {
            width: None,
            height: preset.height,
        },
        "none" => ResizeOptions {
            width: None,
            height: None,
        },
        mode => return Err(format!("Unsupported preset resize mode: {mode}")),
    };
    Ok(ConversionRequest {
        images: vec![ConversionImageInput {
            path: path.to_string_lossy().to_string(),
        }],
        format,
        resize,
        quality: preset.quality,
        filename_component: preset.filename_component.clone(),
        filename_mode: preset.filename_mode.clone(),
        output_dir: preset.output_directory.clone(),
        collision_mode: if overwrite {
            CollisionMode::Overwrite
        } else {
            CollisionMode::Rename
        },
    })
}

fn matching_magic_directory<'a>(
    directories: &'a [MagicDirectory],
    path: &Path,
) -> Option<&'a MagicDirectory> {
    let extension = watched_extension(path)?;
    let parent = path.parent().map(normalize_existing_path)?;
    directories.iter().find(|directory| {
        directory.enabled
            && directory.formats.iter().any(|format| format == &extension)
            && normalize_existing_path(Path::new(&directory.path)) == parent
    })
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn watched_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let format = normalize_watch_format(&extension);
    VALID_WATCH_FORMATS
        .contains(&format.as_str())
        .then_some(format)
}

fn normalize_watch_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".into(),
        "heic" | "heif" => "heic".into(),
        "tif" | "tiff" => "tiff".into(),
        format => format.into(),
    }
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn ignore_output_path(ignored_paths: &IgnoredPaths, path: &Path) {
    if let Ok(mut ignored) = ignored_paths.lock() {
        ignored.insert(normalize_existing_path(path), Instant::now());
    }
}

fn is_ignored_path(ignored_paths: &IgnoredPaths, path: &Path) -> bool {
    let Ok(mut ignored) = ignored_paths.lock() else {
        return false;
    };
    ignored.retain(|_, created_at| created_at.elapsed() < IGNORED_OUTPUT_TTL);
    ignored.contains_key(path)
}

fn emit_event(app: &AppHandle, kind: &str, message: String, path: Option<String>, active: bool) {
    if let Err(error) = app.emit(
        "magic-directory-event",
        MagicDirectoryEvent {
            kind: kind.into(),
            message,
            path,
            active,
        },
    ) {
        eprintln!("failed to emit watched folder event: {error}");
    }
}

fn path_string(path: &Path) -> Option<String> {
    Some(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        list_magic_directories_with_connection, load_formats, matching_magic_directory,
        preset_conversion_request, save_magic_directory_with_connection, watched_extension,
        CascadeContext, CascadeStop, SaveMagicDirectoryRequest, MAX_CASCADE_DEPTH,
    };
    use crate::{
        image_pipeline::convert_images,
        models::{
            CollisionMode, ConversionImageInput, ConversionPreset, ConversionRequest, ExportFormat,
            MagicDirectory, ResizeOptions,
        },
        presets::initialize_schema,
    };
    use rusqlite::{params, Connection};
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[test]
    fn persists_formats_and_presets_for_a_magic_directory() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("schema");
        let preset_id = insert_preset(&connection, "Watcher WEBP", "_webp");
        let directory = temporary_directory("persists");

        let saved = save_magic_directory_with_connection(
            &mut connection,
            SaveMagicDirectoryRequest {
                id: None,
                name: "Incoming Photos".into(),
                path: directory.to_string_lossy().to_string(),
                formats: vec![
                    "SVG".into(),
                    "JPG".into(),
                    "jpeg".into(),
                    "png".into(),
                    "HEIF".into(),
                    "heic".into(),
                    "svg".into(),
                    "TIF".into(),
                    "tiff".into(),
                    "GIF".into(),
                    "JXL".into(),
                    "JP2".into(),
                ],
                preset_ids: vec![preset_id, preset_id],
                enabled: true,
                overwrite: false,
            },
        )
        .expect("saved magic directory");

        assert_eq!(saved.name, "Incoming Photos");
        assert_eq!(
            saved.formats,
            vec!["svg", "jpeg", "png", "heic", "tiff", "gif", "jxl", "jp2"]
        );
        assert_eq!(saved.preset_ids, vec![preset_id]);
        assert!(saved.enabled);
        assert!(!saved.overwrite);
        assert_eq!(
            list_magic_directories_with_connection(&connection)
                .expect("listed magic directories")
                .len(),
            1
        );

        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn defaults_missing_overwrite_setting_to_false() {
        let request: SaveMagicDirectoryRequest = serde_json::from_value(serde_json::json!({
            "id": null,
            "name": "Legacy Folder",
            "path": "/tmp/incoming",
            "formats": ["png"],
            "presetIds": [1],
            "enabled": true
        }))
        .expect("legacy watched folder request");
        assert!(!request.overwrite);
    }

    #[test]
    fn persists_overwrite_on_create_and_update() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("schema");
        let preset_id = insert_preset(&connection, "Overwrite Preset", "_webp");
        let directory = temporary_directory("persists-overwrite");
        let mut request = SaveMagicDirectoryRequest {
            id: None,
            name: "Replace Output".into(),
            path: directory.to_string_lossy().to_string(),
            formats: vec!["png".into()],
            preset_ids: vec![preset_id],
            enabled: true,
            overwrite: true,
        };

        let saved = save_magic_directory_with_connection(&mut connection, request.clone())
            .expect("create overwrite rule");
        assert!(saved.overwrite);
        assert!(
            list_magic_directories_with_connection(&connection).expect("list overwrite rule")[0]
                .overwrite
        );

        request.id = Some(saved.id);
        request.overwrite = false;
        let updated = save_magic_directory_with_connection(&mut connection, request.clone())
            .expect("disable overwrite");
        assert!(!updated.overwrite);
        assert!(
            !list_magic_directories_with_connection(&connection).expect("list rename rule")[0]
                .overwrite
        );

        request.overwrite = true;
        assert!(
            save_magic_directory_with_connection(&mut connection, request)
                .expect("enable overwrite again")
                .overwrite
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn requires_a_watched_folder_name() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("schema");
        let preset_id = insert_preset(&connection, "Named Watcher", "_named");
        let directory = temporary_directory("requires-name");

        let error = save_magic_directory_with_connection(
            &mut connection,
            SaveMagicDirectoryRequest {
                id: None,
                name: "   ".into(),
                path: directory.to_string_lossy().to_string(),
                formats: vec!["png".into()],
                preset_ids: vec![preset_id],
                enabled: true,
                overwrite: false,
            },
        )
        .expect_err("missing watched folder name");

        assert!(error.to_string().contains("Enter a name"));
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn recognizes_watched_extension_aliases() {
        assert_eq!(
            watched_extension(Path::new("photo.jpg")).as_deref(),
            Some("jpeg")
        );
        assert_eq!(
            watched_extension(Path::new("photo.JPEG")).as_deref(),
            Some("jpeg")
        );
        assert_eq!(
            watched_extension(Path::new("photo.heif")).as_deref(),
            Some("heic")
        );
        assert_eq!(
            watched_extension(Path::new("photo.HEIC")).as_deref(),
            Some("heic")
        );
        assert_eq!(watched_extension(Path::new("photo.txt")), None);
        for (name, format) in [
            ("scan.TIF", "tiff"),
            ("scan.tiff", "tiff"),
            ("animation.GIF", "gif"),
            ("photo.JXL", "jxl"),
            ("photo.JP2", "jp2"),
        ] {
            assert_eq!(watched_extension(Path::new(name)).as_deref(), Some(format));
        }
    }

    #[test]
    fn converts_a_jpeg_path_accepted_by_the_watcher() {
        let directory = temporary_directory("jpeg-conversion");
        let output_directory = directory.join("output");
        fs::create_dir_all(&output_directory).expect("create output directory");
        let input_path = directory.join("incoming.jpg");
        image::RgbImage::from_pixel(2, 2, image::Rgb([32, 64, 96]))
            .save(&input_path)
            .expect("write JPEG input");

        assert_eq!(watched_extension(&input_path).as_deref(), Some("jpeg"));
        let response = convert_images(ConversionRequest {
            images: vec![ConversionImageInput {
                path: input_path.to_string_lossy().to_string(),
            }],
            format: ExportFormat::Png,
            resize: ResizeOptions {
                width: None,
                height: None,
            },
            quality: 90,
            filename_component: "_magic".into(),
            filename_mode: "postfix".into(),
            output_dir: output_directory.to_string_lossy().to_string(),
            collision_mode: CollisionMode::Rename,
        })
        .expect("convert watched JPEG");

        assert_eq!(response.summary.success_count, 1);
        assert_eq!(response.summary.failure_count, 0);
        assert!(output_directory.join("incoming_magic.png").is_file());
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn watcher_renames_by_default_and_can_repeatedly_overwrite_existing_outputs() {
        let directory = temporary_directory("overwrite-conversion");
        let output_directory = directory.join("output");
        fs::create_dir_all(&output_directory).expect("create output directory");
        let input_path = directory.join("1.png");
        let output_path = output_directory.join("1.png");
        image::RgbImage::from_pixel(2, 3, image::Rgb([32, 64, 96]))
            .save(&input_path)
            .expect("write PNG input");
        let original_output = b"existing output must be preserved by default";
        fs::write(&output_path, original_output).expect("write existing output");
        let preset = ConversionPreset {
            id: 1,
            name: "PNG Output".into(),
            format: "png".into(),
            resize_mode: "none".into(),
            width: None,
            height: None,
            quality: 90,
            filename_component: String::new(),
            filename_mode: "postfix".into(),
            output_directory: output_directory.to_string_lossy().into_owned(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let renamed = convert_images(
            preset_conversion_request(&input_path, &preset, false).expect("rename request"),
        )
        .expect("convert with rename mode");
        assert_eq!(renamed.summary.success_count, 1);
        assert!(output_directory.join("1_1.png").is_file());
        assert_eq!(
            fs::read(&output_path).expect("read existing output"),
            original_output
        );
        fs::remove_file(output_directory.join("1_1.png")).expect("remove renamed output");

        for (width, height, color) in [(4, 5, [255, 0, 0]), (6, 7, [0, 255, 0])] {
            image::RgbImage::from_pixel(width, height, image::Rgb(color))
                .save(&input_path)
                .expect("replace watched input");
            let overwritten = convert_images(
                preset_conversion_request(&input_path, &preset, true).expect("overwrite request"),
            )
            .expect("convert with overwrite mode");
            assert_eq!(overwritten.summary.success_count, 1);
            assert_eq!(overwritten.summary.failure_count, 0);
            assert_eq!(
                overwritten.results[0].output_path.as_deref(),
                output_path.to_str()
            );
            let output = image::open(&output_path)
                .expect("read overwritten output")
                .to_rgb8();
            assert_eq!(output.dimensions(), (width, height));
            assert_eq!(*output.get_pixel(0, 0), image::Rgb(color));
            assert!(!output_directory.join("1_1.png").exists());
        }
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn migrates_watched_formats_schema_to_accept_new_inputs() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("initial schema");
        connection
            .execute_batch(
                "DROP TABLE magic_directory_formats;
                 CREATE TABLE magic_directory_formats (
                    magic_directory_id INTEGER NOT NULL,
                    format TEXT NOT NULL
                        CHECK (format IN ('svg', 'jpeg', 'png', 'webp', 'avif', 'heic', 'tiff', 'gif', 'jxl')),
                    PRIMARY KEY (magic_directory_id, format),
                    FOREIGN KEY (magic_directory_id)
                        REFERENCES magic_directories(id) ON DELETE CASCADE
                 );
                 INSERT INTO magic_directories (path, enabled)
                    VALUES ('/tmp/legacy-magic-formats', 1);
                 INSERT INTO magic_directory_formats (magic_directory_id, format)
                    VALUES (1, 'png');",
            )
            .expect("legacy watched formats schema");

        initialize_schema(&mut connection).expect("migrated schema");

        let existing_format: String = connection
            .query_row(
                "SELECT format FROM magic_directory_formats
                 WHERE magic_directory_id = 1",
                [],
                |row| row.get(0),
            )
            .expect("preserved watched format");
        assert_eq!(existing_format, "png");
        for format in ["heic", "tiff", "gif", "jxl", "jp2"] {
            connection.execute(
                "INSERT INTO magic_directory_formats (magic_directory_id, format) VALUES (1, ?1)",
                params![format],
            ).expect("new watched format");
        }
        initialize_schema(&mut connection).expect("idempotent migration");
        assert_eq!(
            load_formats(&connection, 1).expect("all formats"),
            vec!["png", "heic", "tiff", "gif", "jxl", "jp2"]
        );
    }

    #[test]
    fn rejects_multiple_presets_without_unique_filename_markers() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("schema");
        let first_id = insert_preset(&connection, "First Preset", "");
        let second_id = insert_preset(&connection, "Second Preset", "_second");
        let directory = temporary_directory("collision");

        let error = save_magic_directory_with_connection(
            &mut connection,
            SaveMagicDirectoryRequest {
                id: None,
                name: "Collision Test".into(),
                path: directory.to_string_lossy().to_string(),
                formats: vec!["svg".into()],
                preset_ids: vec![first_id, second_id],
                enabled: true,
                overwrite: false,
            },
        )
        .expect_err("collision validation");

        assert!(error.to_string().contains("has no prefix or postfix"));
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn migrates_legacy_preset_links_without_losing_data() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        initialize_schema(&mut connection).expect("initial schema");
        let first_id = insert_preset(&connection, "First Legacy", "_first");
        let second_id = insert_preset(&connection, "Second Legacy", "_second");
        connection
            .execute_batch(
                "DROP TABLE magic_directory_presets;
                 CREATE TABLE magic_directory_presets (
                    magic_directory_id INTEGER NOT NULL,
                    preset_id INTEGER NOT NULL,
                    PRIMARY KEY (magic_directory_id, preset_id),
                    FOREIGN KEY (magic_directory_id) REFERENCES magic_directories(id) ON DELETE CASCADE,
                    FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE
                 );
                 INSERT INTO magic_directories (path, enabled) VALUES ('/tmp/legacy-magic', 1);",
            )
            .expect("legacy schema");
        let directory_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO magic_directory_presets (magic_directory_id, preset_id)
                 VALUES (?1, ?2), (?1, ?3)",
                params![directory_id, second_id, first_id],
            )
            .expect("legacy preset links");

        initialize_schema(&mut connection).expect("migrated schema");

        let directory = list_magic_directories_with_connection(&connection)
            .expect("magic directories")
            .into_iter()
            .next()
            .expect("legacy magic directory");
        assert_eq!(directory.preset_ids, vec![first_id, second_id]);
        let positions = connection
            .prepare(
                "SELECT position FROM magic_directory_presets
                 WHERE magic_directory_id = ?1 ORDER BY position",
            )
            .expect("position query")
            .query_map(params![directory_id], |row| row.get::<_, i64>(0))
            .expect("positions")
            .collect::<Result<Vec<_>, _>>()
            .expect("collected positions");
        assert_eq!(positions, vec![0, 1]);
    }

    #[test]
    fn routes_outputs_to_downstream_rules_and_stops_cycles() {
        let root = temporary_directory("cascade-cycle");
        let directory_a = root.join("a");
        let directory_b = root.join("b");
        fs::create_dir_all(&directory_a).expect("create directory A");
        fs::create_dir_all(&directory_b).expect("create directory B");
        let directories = vec![
            watched_directory(1, "Rule A", &directory_a, "png"),
            watched_directory(2, "Rule B", &directory_b, "webp"),
        ];

        let input = directory_a.join("source.png");
        let rule_a = matching_magic_directory(&directories, &input).expect("match rule A");
        let after_a = CascadeContext::start()
            .advance(rule_a.id)
            .expect("enter rule A");

        let first_output = directory_b.join("source-web.webp");
        let rule_b = matching_magic_directory(&directories, &first_output).expect("match rule B");
        let after_b = after_a.advance(rule_b.id).expect("enter rule B");

        let loop_output = directory_a.join("source-loop.png");
        let repeated_rule =
            matching_magic_directory(&directories, &loop_output).expect("match rule A again");
        assert_eq!(
            after_b.advance(repeated_rule.id).expect_err("stop cycle"),
            CascadeStop::Cycle
        );

        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn cascade_branches_are_independent_and_depth_is_bounded() {
        let after_first_rule = CascadeContext::start()
            .advance(1)
            .expect("enter first rule");
        assert!(after_first_rule.advance(2).is_ok());
        assert!(after_first_rule.advance(2).is_ok());

        let mut context = CascadeContext::start();
        for rule_id in 1..=MAX_CASCADE_DEPTH as i64 {
            context = context.advance(rule_id).expect("advance cascade");
        }
        assert_eq!(
            context.advance(99).expect_err("enforce maximum depth"),
            CascadeStop::MaximumDepth
        );
    }

    fn watched_directory(id: i64, name: &str, path: &Path, format: &str) -> MagicDirectory {
        MagicDirectory {
            id,
            name: name.into(),
            path: path.to_string_lossy().into_owned(),
            formats: vec![format.into()],
            preset_ids: vec![id],
            enabled: true,
            overwrite: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn insert_preset(connection: &Connection, name: &str, component: &str) -> i64 {
        connection
            .execute(
                "INSERT INTO presets (
                    name, format, resize_mode, width, height, quality, filename_component,
                    filename_mode, output_directory
                 ) VALUES (?1, 'webp', 'none', NULL, NULL, 90, ?2, 'postfix', '/tmp')",
                params![name, component],
            )
            .expect("insert preset");
        connection.last_insert_rowid()
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("bulkpixel-magic-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }
}
