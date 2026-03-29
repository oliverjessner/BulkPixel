use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    ColorType, DynamicImage, GenericImageView, ImageBuffer, ImageEncoder, Rgb, RgbImage,
};
use thiserror::Error;

use crate::models::{
    ConversionItemResult, ConversionRequest, ConversionResponse, ConversionSummary, ExportFormat,
    LoadedImage, ProbeImagesResponse, RejectedImage,
};

const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const THUMBNAIL_WIDTH: u32 = 220;
const THUMBNAIL_HEIGHT: u32 = 140;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
}

pub fn default_output_directory() -> String {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .to_string_lossy()
        .to_string()
}

pub fn probe_images(paths: Vec<String>) -> Result<ProbeImagesResponse, AppError> {
    let mut loaded = Vec::new();
    let mut rejected = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(&raw_path);

        match load_single_image(&path) {
            Ok(image) => loaded.push(image),
            Err(error) => rejected.push(RejectedImage {
                path: raw_path,
                reason: error.to_string(),
            }),
        }
    }

    Ok(ProbeImagesResponse { loaded, rejected })
}

pub fn convert_images(request: ConversionRequest) -> Result<ConversionResponse, AppError> {
    validate_request(&request)?;

    let output_dir = PathBuf::from(&request.output_dir);
    fs::create_dir_all(&output_dir)?;

    let mut reserved_names = HashSet::new();
    let mut results = Vec::with_capacity(request.images.len());

    let mut total_original_size = 0_u64;
    let mut total_converted_size = 0_u64;
    let mut success_count = 0_usize;
    let mut failure_count = 0_usize;

    for input in &request.images {
        let input_path = PathBuf::from(&input.path);

        match convert_single_image(&input_path, &output_dir, &request, &mut reserved_names) {
            Ok(result) => {
                success_count += 1;
                total_original_size += result.original_size.unwrap_or_default();
                total_converted_size += result.converted_size.unwrap_or_default();
                results.push(result);
            }
            Err(error) => {
                failure_count += 1;
                let original_name = input_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| input.path.clone());

                results.push(ConversionItemResult {
                    input_path: input.path.clone(),
                    original_name,
                    success: false,
                    output_name: None,
                    output_path: None,
                    original_size: None,
                    converted_size: None,
                    delta_bytes: None,
                    percent_change: None,
                    converted_width: None,
                    converted_height: None,
                    message: error.to_string(),
                });
            }
        }
    }

    let total_delta_bytes = total_original_size as i64 - total_converted_size as i64;
    let total_percent_change = if total_original_size == 0 {
        0.0
    } else {
        (total_delta_bytes as f64 / total_original_size as f64) * 100.0
    };

    Ok(ConversionResponse {
        results,
        summary: ConversionSummary {
            success_count,
            failure_count,
            total_original_size,
            total_converted_size,
            total_delta_bytes,
            total_percent_change,
        },
    })
}

fn validate_request(request: &ConversionRequest) -> Result<(), AppError> {
    if request.images.is_empty() {
        return Err(AppError::Validation(
            "Add at least one image before converting.".into(),
        ));
    }

    if request.output_dir.trim().is_empty() {
        return Err(AppError::Validation(
            "Choose an output folder before converting.".into(),
        ));
    }

    match (request.resize.width, request.resize.height) {
        (Some(0), _) | (_, Some(0)) => {
            return Err(AppError::Validation(
                "Width and height must be greater than zero.".into(),
            ));
        }
        (Some(_), Some(_)) => {
            return Err(AppError::Validation(
                "Enter either width or height, not both.".into(),
            ));
        }
        _ => {}
    }

    if !(1..=100).contains(&request.quality) {
        return Err(AppError::Validation(
            "Quality must be between 1 and 100.".into(),
        ));
    }

    Ok(())
}

fn load_single_image(path: &Path) -> Result<LoadedImage, AppError> {
    validate_input_extension(path)?;

    let metadata = fs::metadata(path)?;
    let image = image::open(path)?;
    let (width, height) = image.dimensions();
    let preview_data_url = build_preview_data_url(&image)?;

    Ok(LoadedImage {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        file_type: format_label_from_extension(path),
        width,
        height,
        file_size: metadata.len(),
        preview_data_url,
    })
}

fn convert_single_image(
    input_path: &Path,
    output_dir: &Path,
    request: &ConversionRequest,
    reserved_names: &mut HashSet<PathBuf>,
) -> Result<ConversionItemResult, AppError> {
    validate_input_extension(input_path)?;

    let metadata = fs::metadata(input_path)?;
    let original_size = metadata.len();
    let original_name = input_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| input_path.to_string_lossy().to_string());

    let image = image::open(input_path)?;
    let resized = resize_image(&image, request.resize.width, request.resize.height);
    let (converted_width, converted_height) = resized.dimensions();

    let bytes = encode_image(&resized, &request.format, request.quality)?;
    let output_path = next_available_output_path(
        output_dir,
        &request.prefix,
        input_path,
        request.format.extension(),
        reserved_names,
    );

    fs::write(&output_path, &bytes)?;

    let converted_size = bytes.len() as u64;
    let delta_bytes = original_size as i64 - converted_size as i64;
    let percent_change = if original_size == 0 {
        0.0
    } else {
        (delta_bytes as f64 / original_size as f64) * 100.0
    };
    let message = build_result_message(delta_bytes, percent_change);

    Ok(ConversionItemResult {
        input_path: input_path.to_string_lossy().to_string(),
        original_name,
        success: true,
        output_name: output_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string()),
        output_path: Some(output_path.to_string_lossy().to_string()),
        original_size: Some(original_size),
        converted_size: Some(converted_size),
        delta_bytes: Some(delta_bytes),
        percent_change: Some(percent_change),
        converted_width: Some(converted_width),
        converted_height: Some(converted_height),
        message,
    })
}

fn resize_image(image: &DynamicImage, width: Option<u32>, height: Option<u32>) -> DynamicImage {
    match (width, height) {
        (Some(width), None) => {
            let (_, original_height) = image.dimensions();
            let computed_height =
                ((original_height as f64 * width as f64) / image.width() as f64).round() as u32;
            image.resize_exact(
                width.max(1),
                computed_height.max(1),
                image::imageops::FilterType::Lanczos3,
            )
        }
        (None, Some(height)) => {
            let (original_width, _) = image.dimensions();
            let computed_width =
                ((original_width as f64 * height as f64) / image.height() as f64).round() as u32;
            image.resize_exact(
                computed_width.max(1),
                height.max(1),
                image::imageops::FilterType::Lanczos3,
            )
        }
        _ => image.clone(),
    }
}

fn encode_image(
    image: &DynamicImage,
    format: &ExportFormat,
    quality: u8,
) -> Result<Vec<u8>, AppError> {
    let mut buffer = Vec::new();

    match format {
        ExportFormat::Jpeg => {
            let flattened = flatten_to_rgb8(image);
            let mut encoder = JpegEncoder::new_with_quality(&mut buffer, quality);
            encoder.encode(
                flattened.as_raw(),
                flattened.width(),
                flattened.height(),
                ColorType::Rgb8.into(),
            )?;
        }
        ExportFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = PngEncoder::new(&mut buffer);
            encoder.write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ColorType::Rgba8.into(),
            )?;
        }
        ExportFormat::Webp => {
            let rgba = image.to_rgba8();
            let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
            let webp_data = encoder.encode(quality as f32);
            buffer.extend_from_slice(webp_data.as_ref());
        }
    }

    Ok(buffer)
}

fn build_preview_data_url(image: &DynamicImage) -> Result<String, AppError> {
    let thumb = image.thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
    let rgba = thumb.to_rgba8();
    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    encoder.write_image(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        ColorType::Rgba8.into(),
    )?;

    Ok(format!("data:image/png;base64,{}", BASE64.encode(bytes)))
}

fn flatten_to_rgb8(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    ImageBuffer::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y);
        let alpha = pixel[3] as f32 / 255.0;
        let r = blend_channel(pixel[0], alpha);
        let g = blend_channel(pixel[1], alpha);
        let b = blend_channel(pixel[2], alpha);
        Rgb([r, g, b])
    })
}

fn blend_channel(channel: u8, alpha: f32) -> u8 {
    ((channel as f32 * alpha) + (255.0 * (1.0 - alpha))).round() as u8
}

fn validate_input_extension(path: &Path) -> Result<(), AppError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::Validation("Unsupported file type.".into()))?;

    if SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{} is not a supported image type.",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_else(|| path.to_string_lossy())
        )))
    }
}

fn format_label_from_extension(path: &Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "JPEG".into(),
        Some("png") => "PNG".into(),
        Some("webp") => "WEBP".into(),
        _ => "Image".into(),
    }
}

fn next_available_output_path(
    output_dir: &Path,
    prefix: &str,
    source_path: &Path,
    target_extension: &str,
    reserved_names: &mut HashSet<PathBuf>,
) -> PathBuf {
    let stem = source_path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".into());
    let sanitized_prefix = sanitize_component(prefix);
    let sanitized_stem = sanitize_component(&stem);
    let base_name = format!(
        "{}{}",
        sanitized_prefix,
        if sanitized_stem.is_empty() {
            "image".to_string()
        } else {
            sanitized_stem
        }
    );

    let mut counter = 0_usize;

    loop {
        let file_name = if counter == 0 {
            format!("{base_name}.{target_extension}")
        } else {
            format!("{base_name}_{counter}.{target_extension}")
        };

        let candidate = output_dir.join(file_name);
        if !candidate.exists() && !reserved_names.contains(&candidate) {
            reserved_names.insert(candidate.clone());
            return candidate;
        }

        counter += 1;
    }
}

fn sanitize_component(value: &str) -> String {
    let trimmed = value.trim();
    let mut sanitized = String::with_capacity(trimmed.len());

    for character in trimmed.chars() {
        if character.is_ascii_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }

    sanitized.trim_matches(['.', ' ']).to_string()
}

fn build_result_message(delta_bytes: i64, percent_change: f64) -> String {
    if delta_bytes >= 0 {
        format!(
            "{} saved ({:.1}%)",
            human_size(delta_bytes as u64),
            percent_change.abs()
        )
    } else {
        format!(
            "{} larger ({:.1}%)",
            human_size(delta_bytes.unsigned_abs()),
            percent_change.abs()
        )
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_index = 0_usize;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::{next_available_output_path, sanitize_component};
    use std::{collections::HashSet, path::Path};

    #[test]
    fn sanitizes_file_components() {
        assert_eq!(sanitize_component(" bulk:/demo* "), "bulk__demo_");
        assert_eq!(sanitize_component(".."), "");
    }

    #[test]
    fn increments_duplicate_names() {
        let mut reserved = HashSet::new();
        let first = next_available_output_path(
            Path::new("/tmp"),
            "bulk_",
            Path::new("/images/photo.png"),
            "jpg",
            &mut reserved,
        );
        let second = next_available_output_path(
            Path::new("/tmp"),
            "bulk_",
            Path::new("/images/photo.png"),
            "jpg",
            &mut reserved,
        );

        assert!(first.ends_with("bulk_photo.jpg"));
        assert!(second.ends_with("bulk_photo_1.jpg"));
    }
}
