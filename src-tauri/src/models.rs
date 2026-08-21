use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Jpeg,
    Png,
    Webp,
    Avif,
}

impl ExportFormat {
    pub fn from_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "jpeg" | "jpg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Avif => "avif",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Png => "PNG",
            Self::Webp => "WEBP",
            Self::Avif => "AVIF",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CollisionMode {
    Rename,
    Error,
    Overwrite,
}

impl Default for CollisionMode {
    fn default() -> Self {
        Self::Rename
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResizeOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionImageInput {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    pub images: Vec<ConversionImageInput>,
    pub format: ExportFormat,
    pub resize: ResizeOptions,
    pub quality: u8,
    pub filename_component: String,
    pub filename_mode: String,
    pub output_dir: String,
    #[serde(default)]
    pub collision_mode: CollisionMode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedImage {
    pub path: String,
    pub name: String,
    pub file_type: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub preview_data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedImage {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeImagesResponse {
    pub loaded: Vec<LoadedImage>,
    pub rejected: Vec<RejectedImage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionItemResult {
    pub input_path: String,
    pub original_name: String,
    pub success: bool,
    pub output_name: Option<String>,
    pub output_path: Option<String>,
    pub original_size: Option<u64>,
    pub converted_size: Option<u64>,
    pub delta_bytes: Option<i64>,
    pub percent_change: Option<f64>,
    pub converted_width: Option<u32>,
    pub converted_height: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionSummary {
    pub success_count: usize,
    pub failure_count: usize,
    pub total_original_size: u64,
    pub total_converted_size: u64,
    pub total_delta_bytes: i64,
    pub total_percent_change: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResponse {
    pub results: Vec<ConversionItemResult>,
    pub summary: ConversionSummary,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePresetRequest {
    pub id: Option<i64>,
    pub name: String,
    pub format: String,
    pub resize_mode: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: u8,
    pub filename_component: String,
    pub filename_mode: String,
    pub output_directory: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionPreset {
    pub id: i64,
    pub name: String,
    pub format: String,
    pub resize_mode: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: u8,
    pub filename_component: String,
    pub filename_mode: String,
    pub output_directory: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn validate_multiple_preset_markers(presets: &[ConversionPreset]) -> Result<(), String> {
    let mut markers = HashSet::new();

    for preset in presets {
        let component = preset.filename_component.trim();
        if component.is_empty() {
            return Err(format!(
                "Preset collision risk: '{}' has no prefix or postfix.",
                preset.name
            ));
        }

        let marker = format!("{}:{component}", preset.filename_mode);
        if !markers.insert(marker) {
            return Err(
                "Preset collision risk: multiple presets use the same prefix/postfix.".into(),
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagicDirectory {
    pub id: i64,
    pub path: String,
    pub formats: Vec<String>,
    pub preset_ids: Vec<i64>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMagicDirectoryRequest {
    pub id: Option<i64>,
    pub path: String,
    pub formats: Vec<String>,
    pub preset_ids: Vec<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MagicDirectoryEvent {
    pub kind: String,
    pub message: String,
    pub path: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionStatistics {
    pub amount: i64,
    pub cli_uses: i64,
    pub webp: i64,
    pub avif: i64,
    pub jpeg: i64,
    pub png: i64,
    pub input_bytes: i64,
    pub output_bytes: i64,
    pub processing_time_ms: i64,
    pub saved_bytes: i64,
    pub created_at: String,
    pub last_conversion_at: String,
}
