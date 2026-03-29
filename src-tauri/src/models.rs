use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Jpeg,
    Png,
    Webp,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
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
    pub prefix: String,
    pub output_dir: String,
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
