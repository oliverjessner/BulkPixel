use std::{fs, path::PathBuf, time::Duration};

use rusqlite::{params, Connection, Error as SqlError, OptionalExtension};
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::models::{
    ConversionPreset, ConversionStatistics, ConversionSummary, ExportFormat, SavePresetRequest,
};

const DATABASE_FILE: &str = "presets.sqlite3";
const APP_DATA_DIR_NAME: &str = "com.oli.bulkpixel";
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
const CREATE_STATISTICS_TABLE_SQL: &str = "
    CREATE TABLE IF NOT EXISTS statistics (
        id INTEGER PRIMARY KEY CHECK (id = 1),

        amount INTEGER GENERATED ALWAYS AS (
            webp + avif + jpeg + png
        ) STORED,

        cli_uses INTEGER NOT NULL DEFAULT 0
            CHECK (cli_uses >= 0),

        webp INTEGER NOT NULL DEFAULT 0
            CHECK (webp >= 0),

        avif INTEGER NOT NULL DEFAULT 0
            CHECK (avif >= 0),

        jpeg INTEGER NOT NULL DEFAULT 0
            CHECK (jpeg >= 0),

        png INTEGER NOT NULL DEFAULT 0
            CHECK (png >= 0),

        input_bytes INTEGER NOT NULL DEFAULT 0 CHECK (input_bytes >= 0),
        output_bytes INTEGER NOT NULL DEFAULT 0 CHECK (output_bytes >= 0),
        processing_time_ms INTEGER NOT NULL DEFAULT 0
            CHECK (processing_time_ms >= 0),

        saved_bytes INTEGER GENERATED ALWAYS AS (
            CASE
                WHEN input_bytes > output_bytes THEN input_bytes - output_bytes
                ELSE 0
            END
        ) STORED,

        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        last_conversion_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
";
const CREATE_MAGIC_DIRECTORIES_TABLES_SQL: &str = "
    CREATE TABLE IF NOT EXISTS magic_directories (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT NOT NULL UNIQUE
            CHECK (length(trim(path)) > 0),
        enabled INTEGER NOT NULL DEFAULT 1
            CHECK (enabled IN (0, 1)),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS magic_directory_formats (
        magic_directory_id INTEGER NOT NULL,
        format TEXT NOT NULL
            CHECK (format IN ('svg', 'png', 'webp', 'avif')),
        PRIMARY KEY (magic_directory_id, format),
        FOREIGN KEY (magic_directory_id) REFERENCES magic_directories(id) ON DELETE CASCADE
    );

";
const CREATE_MAGIC_DIRECTORY_PRESETS_TABLE_SQL: &str = "
    CREATE TABLE IF NOT EXISTS magic_directory_presets (
        magic_directory_id INTEGER NOT NULL,
        preset_id INTEGER NOT NULL,
        position INTEGER NOT NULL CHECK (position >= 0),
        PRIMARY KEY (magic_directory_id, preset_id),
        UNIQUE (magic_directory_id, position),
        FOREIGN KEY (magic_directory_id) REFERENCES magic_directories(id) ON DELETE CASCADE,
        FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE
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
    list_presets_with_connection(&connection)
}

pub fn list_presets_for_cli() -> Result<Vec<ConversionPreset>, PresetError> {
    let connection = open_cli_connection()?;
    list_presets_with_connection(&connection)
}

fn list_presets_with_connection(
    connection: &Connection,
) -> Result<Vec<ConversionPreset>, PresetError> {
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
    request: SavePresetRequest,
) -> Result<ConversionPreset, PresetError> {
    let connection = open_connection(app)?;
    save_preset_with_connection(&connection, request)
}

pub fn save_preset_for_cli(request: SavePresetRequest) -> Result<ConversionPreset, PresetError> {
    let connection = open_cli_connection()?;
    save_preset_with_connection(&connection, request)
}

fn save_preset_with_connection(
    connection: &Connection,
    mut request: SavePresetRequest,
) -> Result<ConversionPreset, PresetError> {
    normalize_request(&mut request);
    validate_request(&request)?;

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
    delete_preset_with_connection(&connection, id)
}

pub fn delete_preset_by_name_for_cli(name: &str) -> Result<(), PresetError> {
    let connection = open_cli_connection()?;
    let preset = get_preset_by_name(&connection, name)?;
    delete_preset_with_connection(&connection, preset.id)
}

fn delete_preset_with_connection(connection: &Connection, id: i64) -> Result<(), PresetError> {
    let changed = connection.execute("DELETE FROM presets WHERE id = ?1", params![id])?;

    if changed == 0 {
        return Err(PresetError::Validation("Preset not found.".into()));
    }

    Ok(())
}

pub fn find_preset_by_name_for_cli(name: &str) -> Result<ConversionPreset, PresetError> {
    let connection = open_cli_connection()?;
    get_preset_by_name(&connection, name)
}

pub fn get_presets_by_ids(
    app: &AppHandle,
    ids: &[i64],
) -> Result<Vec<ConversionPreset>, PresetError> {
    let connection = open_connection(app)?;
    get_presets_by_ids_with_connection(&connection, ids)
}

pub(crate) fn get_presets_by_ids_with_connection(
    connection: &Connection,
    ids: &[i64],
) -> Result<Vec<ConversionPreset>, PresetError> {
    ids.iter().map(|id| get_preset(connection, *id)).collect()
}

pub fn record_conversion_statistics(
    app: &AppHandle,
    format: &ExportFormat,
    summary: &ConversionSummary,
    processing_time_ms: u128,
) -> Result<(), PresetError> {
    let connection = open_connection(app)?;
    record_conversion_statistics_with_connection(&connection, format, summary, processing_time_ms)
}

pub fn record_conversion_statistics_for_cli(
    format: &ExportFormat,
    summary: &ConversionSummary,
    processing_time_ms: u128,
) -> Result<(), PresetError> {
    let connection = open_cli_connection()?;
    record_conversion_statistics_with_connection(&connection, format, summary, processing_time_ms)
}

pub fn record_cli_usage_for_cli() -> Result<(), PresetError> {
    let connection = open_cli_connection()?;
    record_cli_usage_with_connection(&connection)
}

fn record_cli_usage_with_connection(connection: &Connection) -> Result<(), PresetError> {
    connection.execute(
        "UPDATE statistics SET cli_uses = cli_uses + 1 WHERE id = 1",
        [],
    )?;
    Ok(())
}

fn record_conversion_statistics_with_connection(
    connection: &Connection,
    format: &ExportFormat,
    summary: &ConversionSummary,
    processing_time_ms: u128,
) -> Result<(), PresetError> {
    if summary.success_count == 0 {
        return Ok(());
    }

    let format_column = statistics_format_column(format);
    let success_count = saturating_i64_from_usize(summary.success_count);
    let input_bytes = saturating_i64_from_u64(summary.total_original_size);
    let output_bytes = saturating_i64_from_u64(summary.total_converted_size);
    let processing_time_ms = saturating_i64_from_u128(processing_time_ms);

    let statement = format!(
        "UPDATE statistics
         SET {format_column} = {format_column} + ?1,
             input_bytes = input_bytes + ?2,
             output_bytes = output_bytes + ?3,
             processing_time_ms = processing_time_ms + ?4,
             last_conversion_at = CURRENT_TIMESTAMP
         WHERE id = 1",
    );

    connection.execute(
        &statement,
        params![success_count, input_bytes, output_bytes, processing_time_ms],
    )?;

    Ok(())
}

pub fn load_statistics(app: &AppHandle) -> Result<ConversionStatistics, PresetError> {
    let connection = open_connection(app)?;
    load_statistics_with_connection(&connection)
}

pub fn load_statistics_for_cli() -> Result<ConversionStatistics, PresetError> {
    let connection = open_cli_connection()?;
    load_statistics_with_connection(&connection)
}

fn load_statistics_with_connection(
    connection: &Connection,
) -> Result<ConversionStatistics, PresetError> {
    connection
        .query_row(
            "SELECT amount, cli_uses, webp, avif, jpeg, png, input_bytes, output_bytes,
                    processing_time_ms, saved_bytes, created_at, last_conversion_at
             FROM statistics
             WHERE id = 1",
            [],
            |row| {
                Ok(ConversionStatistics {
                    amount: row.get(0)?,
                    cli_uses: row.get(1)?,
                    webp: row.get(2)?,
                    avif: row.get(3)?,
                    jpeg: row.get(4)?,
                    png: row.get(5)?,
                    input_bytes: row.get(6)?,
                    output_bytes: row.get(7)?,
                    processing_time_ms: row.get(8)?,
                    saved_bytes: row.get(9)?,
                    created_at: row.get(10)?,
                    last_conversion_at: row.get(11)?,
                })
            },
        )
        .map_err(PresetError::from)
}

pub(crate) fn open_connection(app: &AppHandle) -> Result<Connection, PresetError> {
    let database_path = database_path(app)?;
    let mut connection = Connection::open(database_path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    initialize_schema(&mut connection)?;
    Ok(connection)
}

pub(crate) fn open_cli_connection() -> Result<Connection, PresetError> {
    let database_path = cli_database_path()?;
    let mut connection = Connection::open(database_path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    initialize_schema(&mut connection)?;
    Ok(connection)
}

fn database_path(app: &AppHandle) -> Result<PathBuf, PresetError> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| PresetError::Storage(error.to_string()))?;
    database_path_from_directory(directory)
}

fn cli_database_path() -> Result<PathBuf, PresetError> {
    let directory = dirs::data_dir()
        .ok_or_else(|| PresetError::Storage("Unable to resolve the data directory.".into()))?
        .join(APP_DATA_DIR_NAME);
    database_path_from_directory(directory)
}

fn database_path_from_directory(directory: PathBuf) -> Result<PathBuf, PresetError> {
    fs::create_dir_all(&directory).map_err(|error| PresetError::Storage(error.to_string()))?;
    Ok(directory.join(DATABASE_FILE))
}

pub(crate) fn initialize_schema(connection: &mut Connection) -> Result<(), PresetError> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
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

    initialize_statistics_schema(connection)?;
    connection.execute_batch(CREATE_MAGIC_DIRECTORIES_TABLES_SQL)?;
    connection.execute_batch(CREATE_MAGIC_DIRECTORY_PRESETS_TABLE_SQL)?;
    migrate_magic_directory_presets_schema(connection)?;

    Ok(())
}

fn migrate_magic_directory_presets_schema(connection: &mut Connection) -> Result<(), PresetError> {
    let mut statement = connection.prepare("PRAGMA table_info(magic_directory_presets)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    if columns.iter().any(|column| column == "position") {
        return Ok(());
    }

    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "ALTER TABLE magic_directory_presets RENAME TO magic_directory_presets_legacy;",
    )?;
    transaction.execute_batch(CREATE_MAGIC_DIRECTORY_PRESETS_TABLE_SQL)?;
    transaction.execute(
        "INSERT INTO magic_directory_presets (magic_directory_id, preset_id, position)
         SELECT
            magic_directory_id,
            preset_id,
            ROW_NUMBER() OVER (
                PARTITION BY magic_directory_id
                ORDER BY preset_id ASC
            ) - 1
         FROM magic_directory_presets_legacy",
        [],
    )?;
    transaction.execute_batch("DROP TABLE magic_directory_presets_legacy;")?;
    transaction.commit()?;

    Ok(())
}

fn initialize_statistics_schema(connection: &Connection) -> Result<(), PresetError> {
    connection.execute_batch(CREATE_STATISTICS_TABLE_SQL)?;

    let mut statement = connection.prepare("PRAGMA table_info(statistics)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    if !columns.iter().any(|column| column == "cli_uses") {
        connection.execute(
            "ALTER TABLE statistics
             ADD COLUMN cli_uses INTEGER NOT NULL DEFAULT 0 CHECK (cli_uses >= 0)",
            [],
        )?;
    }

    connection.execute("INSERT OR IGNORE INTO statistics (id) VALUES (1)", [])?;
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

fn get_preset_by_name(
    connection: &Connection,
    name: &str,
) -> Result<ConversionPreset, PresetError> {
    let normalized_name = name.trim();
    connection
        .query_row(
            "SELECT id, name, format, resize_mode, width, height, quality, filename_component,
                    filename_mode, output_directory, created_at, updated_at
             FROM presets
             WHERE lower(name) = lower(?1)
             LIMIT 1",
            params![normalized_name],
            map_preset_row,
        )
        .optional()?
        .ok_or_else(|| PresetError::Validation(format!("Preset not found: {normalized_name}")))
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

fn statistics_format_column(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::Jpeg => "jpeg",
        ExportFormat::Png => "png",
        ExportFormat::Webp => "webp",
        ExportFormat::Avif => "avif",
    }
}

fn saturating_i64_from_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn saturating_i64_from_u128(value: u128) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn saturating_i64_from_usize(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        initialize_statistics_schema, load_statistics_with_connection,
        record_cli_usage_with_connection,
    };

    #[test]
    fn migrates_cli_usage_without_resetting_existing_statistics() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE statistics (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    amount INTEGER GENERATED ALWAYS AS (
                        webp + avif + jpeg + png
                    ) STORED,
                    webp INTEGER NOT NULL DEFAULT 0 CHECK (webp >= 0),
                    avif INTEGER NOT NULL DEFAULT 0 CHECK (avif >= 0),
                    jpeg INTEGER NOT NULL DEFAULT 0 CHECK (jpeg >= 0),
                    png INTEGER NOT NULL DEFAULT 0 CHECK (png >= 0),
                    input_bytes INTEGER NOT NULL DEFAULT 0 CHECK (input_bytes >= 0),
                    output_bytes INTEGER NOT NULL DEFAULT 0 CHECK (output_bytes >= 0),
                    processing_time_ms INTEGER NOT NULL DEFAULT 0
                        CHECK (processing_time_ms >= 0),
                    saved_bytes INTEGER GENERATED ALWAYS AS (
                        CASE
                            WHEN input_bytes > output_bytes THEN input_bytes - output_bytes
                            ELSE 0
                        END
                    ) STORED,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_conversion_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO statistics (
                    id, webp, avif, jpeg, png, input_bytes, output_bytes, processing_time_ms
                ) VALUES (1, 4, 3, 2, 1, 1000, 600, 250);",
            )
            .expect("legacy statistics");

        initialize_statistics_schema(&connection).expect("migration");
        let statistics = load_statistics_with_connection(&connection).expect("statistics");

        assert_eq!(statistics.amount, 10);
        assert_eq!(statistics.cli_uses, 0);
        assert_eq!(statistics.saved_bytes, 400);

        record_cli_usage_with_connection(&connection).expect("first CLI use");
        record_cli_usage_with_connection(&connection).expect("second CLI use");

        let statistics = load_statistics_with_connection(&connection).expect("updated statistics");
        assert_eq!(statistics.amount, 10);
        assert_eq!(statistics.cli_uses, 2);
    }
}
