use std::{
    collections::{HashMap, HashSet},
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
        open_connection, record_conversion_statistics, PresetError,
    },
};

const VALID_WATCH_FORMATS: &[&str] = &["svg", "png", "webp", "avif"];
const EVENT_DEBOUNCE: Duration = Duration::from_millis(800);
const FILE_READY_POLL: Duration = Duration::from_millis(250);
const FILE_READY_ATTEMPTS: usize = 40;
const IGNORED_OUTPUT_TTL: Duration = Duration::from_secs(60);

type PendingPaths = Arc<Mutex<HashMap<PathBuf, u64>>>;
type IgnoredPaths = Arc<Mutex<HashMap<PathBuf, Instant>>>;
type ConversionLock = Arc<Mutex<()>>;

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
        "SELECT id, path, enabled, created_at, updated_at
         FROM magic_directories
         ORDER BY lower(path) ASC, id ASC",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    rows.into_iter()
        .map(|(id, path, enabled, created_at, updated_at)| {
            Ok(MagicDirectory {
                id,
                path,
                formats: load_formats(connection, id)?,
                preset_ids: load_preset_ids(connection, id)?,
                enabled,
                created_at,
                updated_at,
            })
        })
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
                 SET path = ?1, enabled = ?2, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![request.path, request.enabled, id],
            )?;
            if changed == 0 {
                return Err(PresetError::Validation("Magic directory not found.".into()));
            }
            id
        }
        None => {
            transaction.execute(
                "INSERT INTO magic_directories (path, enabled) VALUES (?1, ?2)",
                params![request.path, request.enabled],
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
        return Err(PresetError::Validation("Magic directory not found.".into()));
    }
    Ok(())
}

fn get_magic_directory(connection: &Connection, id: i64) -> Result<MagicDirectory, PresetError> {
    let row = connection
        .query_row(
            "SELECT id, path, enabled, created_at, updated_at
             FROM magic_directories WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| PresetError::Validation("Magic directory not found.".into()))?;

    Ok(MagicDirectory {
        id: row.0,
        path: row.1,
        formats: load_formats(connection, id)?,
        preset_ids: load_preset_ids(connection, id)?,
        enabled: row.2,
        created_at: row.3,
        updated_at: row.4,
    })
}

fn load_formats(connection: &Connection, id: i64) -> Result<Vec<String>, PresetError> {
    let mut statement = connection.prepare(
        "SELECT format FROM magic_directory_formats
         WHERE magic_directory_id = ?1
         ORDER BY CASE format
            WHEN 'svg' THEN 1 WHEN 'png' THEN 2 WHEN 'webp' THEN 3 WHEN 'avif' THEN 4 END",
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
    let raw_path = PathBuf::from(request.path.trim());
    let canonical_path = fs::canonicalize(&raw_path).map_err(|error| {
        PresetError::Validation(format!("Unable to access the magic directory: {error}"))
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
        .map(|format| format.trim().to_ascii_lowercase())
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
            "Choose only SVG, PNG, WEBP, or AVIF as watched formats.".into(),
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
                format!("Magic directory watcher error: {error}"),
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
        .map_err(|_| "Magic watcher state is unavailable.")? = if watched_count == 0 {
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
    let extension = match watched_extension(path) {
        Some(extension) => extension,
        None => return,
    };
    let directories = match list_magic_directories(app) {
        Ok(directories) => directories,
        Err(error) => {
            emit_event(app, "error", error.to_string(), path_string(path), false);
            return;
        }
    };
    let parent = path.parent().map(normalize_existing_path);
    let Some(directory) = directories.into_iter().find(|directory| {
        directory.enabled
            && directory.formats.iter().any(|format| format == &extension)
            && parent.as_ref().is_some_and(|parent| {
                normalize_existing_path(Path::new(&directory.path)) == *parent
            })
    }) else {
        return;
    };

    let presets = match get_presets_by_ids(app, &directory.preset_ids) {
        Ok(presets) => presets,
        Err(error) => {
            emit_event(app, "error", error.to_string(), path_string(path), false);
            return;
        }
    };
    if presets.is_empty() {
        emit_event(
            app,
            "error",
            "Magic directory has no available presets. Edit the rule before using it.".into(),
            path_string(path),
            false,
        );
        return;
    }
    if presets.len() > 1 {
        if let Err(error) = validate_multiple_preset_markers(&presets) {
            emit_event(app, "error", error, path_string(path), false);
            return;
        }
    }

    emit_event(
        app,
        "info",
        format!(
            "Processing {} with {} preset(s)...",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_else(|| path.to_string_lossy()),
            presets.len()
        ),
        path_string(path),
        true,
    );

    let mut success_count = 0_usize;
    let mut failure_count = 0_usize;
    for preset in presets {
        match run_preset(app, path, &preset, ignored_paths) {
            Ok((successes, failures)) => {
                success_count += successes;
                failure_count += failures;
            }
            Err(error) => {
                failure_count += 1;
                emit_event(app, "error", error, path_string(path), true);
            }
        }
    }

    let (kind, message) = if failure_count == 0 {
        (
            "success",
            format!("Magic directory converted {success_count} output(s)."),
        )
    } else if success_count > 0 {
        (
            "warning",
            format!("Magic directory created {success_count} output(s); {failure_count} failed."),
        )
    } else {
        ("error", "Magic directory conversion failed.".into())
    };
    emit_event(app, kind, message, path_string(path), false);
}

fn run_preset(
    app: &AppHandle,
    path: &Path,
    preset: &ConversionPreset,
    ignored_paths: &IgnoredPaths,
) -> Result<(usize, usize), String> {
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
    let request = ConversionRequest {
        images: vec![ConversionImageInput {
            path: path.to_string_lossy().to_string(),
        }],
        format: format.clone(),
        resize,
        quality: preset.quality,
        filename_component: preset.filename_component.clone(),
        filename_mode: preset.filename_mode.clone(),
        output_dir: preset.output_directory.clone(),
        collision_mode: CollisionMode::Rename,
    };

    let started_at = Instant::now();
    let response = convert_images(request).map_err(|error| error.to_string())?;
    for output_path in response
        .results
        .iter()
        .filter_map(|result| result.output_path.as_deref())
    {
        ignore_output_path(ignored_paths, Path::new(output_path));
    }
    if let Err(error) = record_conversion_statistics(
        app,
        &format,
        &response.summary,
        started_at.elapsed().as_millis(),
    ) {
        eprintln!("failed to update magic directory statistics: {error}");
    }

    Ok((
        response.summary.success_count,
        response.summary.failure_count,
    ))
}

fn watched_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    VALID_WATCH_FORMATS
        .contains(&extension.as_str())
        .then_some(extension)
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
        eprintln!("failed to emit magic directory event: {error}");
    }
}

fn path_string(path: &Path) -> Option<String> {
    Some(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        list_magic_directories_with_connection, save_magic_directory_with_connection,
        SaveMagicDirectoryRequest,
    };
    use crate::presets::initialize_schema;
    use rusqlite::{params, Connection};
    use std::{fs, path::PathBuf};

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
                path: directory.to_string_lossy().to_string(),
                formats: vec!["SVG".into(), "png".into(), "svg".into()],
                preset_ids: vec![preset_id, preset_id],
                enabled: true,
            },
        )
        .expect("saved magic directory");

        assert_eq!(saved.formats, vec!["svg", "png"]);
        assert_eq!(saved.preset_ids, vec![preset_id]);
        assert!(saved.enabled);
        assert_eq!(
            list_magic_directories_with_connection(&connection)
                .expect("listed magic directories")
                .len(),
            1
        );

        fs::remove_dir_all(directory).expect("remove temporary directory");
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
                path: directory.to_string_lossy().to_string(),
                formats: vec!["svg".into()],
                preset_ids: vec![first_id, second_id],
                enabled: true,
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
