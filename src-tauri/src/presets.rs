use std::{fs, path::PathBuf};

use rusqlite::{params, Connection, Error as SqlError, OptionalExtension};
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::models::{ConversionPreset, SavePresetRequest};

const DATABASE_FILE: &str = "presets.sqlite3";
const VALID_FORMATS: &[&str] = &["jpeg", "png", "webp", "avif"];
const VALID_RESIZE_MODES: &[&str] = &["none", "width", "height"];
const VALID_FILENAME_MODES: &[&str] = &["prefix", "postfix"];
const CREATE_PRESETS_TABLE_SQL: &str = "
    CREATE TABLE IF NOT EXISTS presets (
        id INTEGER PRIMARY KEY AUTOINCREMENT,

        name TEXT NOT NULL UNIQUE
            CHECK (length(trim(name)) > 3),

        format TEXT NOT NULL
            CHECK (format IN ('png', 'jpeg', 'avif', 'webp')),

        resize_mode TEXT NOT NULL,

        width INTEGER
            CHECK (width IS NULL OR width BETWEEN 1 AND 9999),

        height INTEGER
            CHECK (height IS NULL OR height BETWEEN 1 AND 9999),

        quality INTEGER NOT NULL
            CHECK (quality BETWEEN 1 AND 100),

        filename_component TEXT NOT NULL DEFAULT '',

        filename_mode TEXT NOT NULL,

        output_directory TEXT NOT NULL
            CHECK (length(trim(output_directory)) > 0),

        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
";

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("Preset storage error: {0}")]
    Storage(String),
    #[error("Preset database error: {0}")]
    Database(#[from] SqlError),
    #[error("{0}")]
    Validation(String),
}

pub fn list_presets(app: &AppHandle) -> Result<Vec<ConversionPreset>, PresetError> {
    let connection = open_connection(app)?;
    let mut statement = connection.prepare(
        "SELECT id, name, format, resize_mode, width, height, quality, filename_component,
                filename_mode, output_directory, created_at, updated_at
         FROM presets
         ORDER BY lower(name) ASC, id ASC",
    )?;

    let presets = statement
        .query_map([], map_preset_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(presets)
}

pub fn save_preset(
    app: &AppHandle,
    mut request: SavePresetRequest,
) -> Result<ConversionPreset, PresetError> {
    normalize_request(&mut request);
    validate_request(&request)?;

    let connection = open_connection(app)?;

    match request.id {
        Some(id) => {
            let changed = connection.execute(
                "UPDATE presets
                 SET name = ?1,
                     format = ?2,
                     resize_mode = ?3,
                     width = ?4,
                     height = ?5,
                     quality = ?6,
                     filename_component = ?7,
                     filename_mode = ?8,
                     output_directory = ?9,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?10",
                params![
                    request.name,
                    request.format,
                    request.resize_mode,
                    request.width,
                    request.height,
                    request.quality,
                    request.filename_component,
                    request.filename_mode,
                    request.output_directory,
                    id,
                ],
            )?;

            if changed == 0 {
                return Err(PresetError::Validation("Preset not found.".into()));
            }

            get_preset(&connection, id)
        }
        None => {
            connection.execute(
                "INSERT INTO presets (
                    name, format, resize_mode, width, height, quality, filename_component,
                    filename_mode, output_directory
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    request.name,
                    request.format,
                    request.resize_mode,
                    request.width,
                    request.height,
                    request.quality,
                    request.filename_component,
                    request.filename_mode,
                    request.output_directory,
                ],
            )?;

            get_preset(&connection, connection.last_insert_rowid())
        }
    }
}

pub fn delete_preset(app: &AppHandle, id: i64) -> Result<(), PresetError> {
    let connection = open_connection(app)?;
    let changed = connection.execute("DELETE FROM presets WHERE id = ?1", params![id])?;

    if changed == 0 {
        return Err(PresetError::Validation("Preset not found.".into()));
    }

    Ok(())
}

fn open_connection(app: &AppHandle) -> Result<Connection, PresetError> {
    let database_path = database_path(app)?;
    let mut connection = Connection::open(database_path)?;
    initialize_schema(&mut connection)?;
    Ok(connection)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, PresetError> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| PresetError::Storage(error.to_string()))?;
    fs::create_dir_all(&directory).map_err(|error| PresetError::Storage(error.to_string()))?;
    Ok(directory.join(DATABASE_FILE))
}

fn initialize_schema(connection: &mut Connection) -> Result<(), PresetError> {
    let existing_schema: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'presets'",
            [],
            |row| row.get(0),
        )
        .optional()?;

    match existing_schema {
        Some(schema) if schema.contains("CHECK (length(trim(name)) > 3)") => {}
        Some(_) => rebuild_presets_table(connection)?,
        None => connection.execute_batch(CREATE_PRESETS_TABLE_SQL)?,
    }

    Ok(())
}

fn rebuild_presets_table(connection: &mut Connection) -> Result<(), PresetError> {
    let transaction = connection.transaction()?;
    transaction.execute_batch("ALTER TABLE presets RENAME TO presets_legacy;")?;
    transaction.execute_batch(CREATE_PRESETS_TABLE_SQL)?;
    transaction.execute(
        "INSERT OR IGNORE INTO presets (
            id, name, format, resize_mode, width, height, quality, filename_component,
            filename_mode, output_directory, created_at, updated_at
         )
         SELECT
            id,
            trim(name),
            lower(format),
            resize_mode,
            width,
            height,
            quality,
            COALESCE(filename_component, ''),
            filename_mode,
            trim(output_directory),
            COALESCE(created_at, CURRENT_TIMESTAMP),
            COALESCE(updated_at, CURRENT_TIMESTAMP)
         FROM presets_legacy
         WHERE length(trim(name)) > 3
            AND lower(format) IN ('png', 'jpeg', 'avif', 'webp')
            AND resize_mode IN ('none', 'width', 'height')
            AND (width IS NULL OR width BETWEEN 1 AND 9999)
            AND (height IS NULL OR height BETWEEN 1 AND 9999)
            AND quality BETWEEN 1 AND 100
            AND filename_mode IN ('prefix', 'postfix')
            AND length(trim(output_directory)) > 0",
        [],
    )?;
    transaction.execute_batch("DROP TABLE presets_legacy;")?;
    transaction.commit()?;

    Ok(())
}

fn get_preset(connection: &Connection, id: i64) -> Result<ConversionPreset, PresetError> {
    connection
        .query_row(
            "SELECT id, name, format, resize_mode, width, height, quality, filename_component,
                    filename_mode, output_directory, created_at, updated_at
             FROM presets
             WHERE id = ?1",
            params![id],
            map_preset_row,
        )
        .map_err(PresetError::from)
}

fn map_preset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversionPreset> {
    let width: Option<i64> = row.get(4)?;
    let height: Option<i64> = row.get(5)?;
    let quality: i64 = row.get(6)?;

    Ok(ConversionPreset {
        id: row.get(0)?,
        name: row.get(1)?,
        format: row.get(2)?,
        resize_mode: row.get(3)?,
        width: width.and_then(|value| u32::try_from(value).ok()),
        height: height.and_then(|value| u32::try_from(value).ok()),
        quality: u8::try_from(quality).unwrap_or(100),
        filename_component: row.get(7)?,
        filename_mode: row.get(8)?,
        output_directory: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn normalize_request(request: &mut SavePresetRequest) {
    request.name = request.name.trim().to_string();
    request.format = request.format.trim().to_lowercase();
    request.resize_mode = request.resize_mode.trim().to_lowercase();
    request.filename_mode = request.filename_mode.trim().to_lowercase();
    request.output_directory = request.output_directory.trim().to_string();

    if request.resize_mode != "width" {
        request.width = None;
    }

    if request.resize_mode != "height" {
        request.height = None;
    }
}

fn validate_request(request: &SavePresetRequest) -> Result<(), PresetError> {
    if request.name.len() <= 3 {
        return Err(PresetError::Validation(
            "Preset name must be longer than 3 characters.".into(),
        ));
    }

    if !VALID_FORMATS.contains(&request.format.as_str()) {
        return Err(PresetError::Validation(
            "Choose a valid export format.".into(),
        ));
    }

    if !VALID_RESIZE_MODES.contains(&request.resize_mode.as_str()) {
        return Err(PresetError::Validation(
            "Choose a valid resolution mode.".into(),
        ));
    }

    match request.resize_mode.as_str() {
        "width" if !is_valid_dimension(request.width) => {
            return Err(PresetError::Validation(
                "Preset width must be between 1 and 9999.".into(),
            ));
        }
        "height" if !is_valid_dimension(request.height) => {
            return Err(PresetError::Validation(
                "Preset height must be between 1 and 9999.".into(),
            ));
        }
        _ => {}
    }

    if !(1..=100).contains(&request.quality) {
        return Err(PresetError::Validation(
            "Preset quality must be between 1 and 100.".into(),
        ));
    }

    if !VALID_FILENAME_MODES.contains(&request.filename_mode.as_str()) {
        return Err(PresetError::Validation("Choose prefix or postfix.".into()));
    }

    if request.output_directory.is_empty() {
        return Err(PresetError::Validation("Choose an output folder.".into()));
    }

    Ok(())
}

fn is_valid_dimension(value: Option<u32>) -> bool {
    value.is_some_and(|dimension| (1..=9999).contains(&dimension))
}
