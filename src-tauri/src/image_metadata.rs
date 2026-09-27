use std::{
    collections::HashSet,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use image::{
    codecs::{avif::AvifDecoder, jpeg::JpegDecoder, png::PngDecoder, webp::WebPDecoder},
    ExtendedColorType, ImageDecoder,
};

use crate::models::{
    ColorImageMetadata, ContentCredentialsMetadata, ContentCredentialsStatus, GeneralImageMetadata,
    ImageMetadataResponse, MetadataEntry, MetadataGroup, MetadataGroupStatus, PrivacyMetadata,
};

const SUPPORTED_EXTENSIONS: &[&str] =
    &["jpg", "jpeg", "png", "webp", "avif", "svg", "heic", "heif"];
const MAX_METADATA_BLOCK_BYTES: usize = 2 * 1024 * 1024;
const MAX_METADATA_ENTRIES: usize = 256;
const MAX_METADATA_VALUE_CHARS: usize = 4096;

#[derive(Default)]
struct ContainerMetadata {
    color_type: Option<ExtendedColorType>,
    icc_checked: bool,
    icc: Option<Vec<u8>>,
    xmp: Option<Vec<u8>>,
    iptc: Option<Vec<u8>>,
    xmp_checked: bool,
    iptc_checked: bool,
    animated: Option<bool>,
    warnings: Vec<String>,
}

pub fn inspect_image_metadata(path: String) -> Result<ImageMetadataResponse, String> {
    let path = validate_local_image_path(&path)?;
    let extension =
        extension(&path).ok_or_else(|| "The image has no supported extension.".to_string())?;
    let (format, mime_type) = format_and_mime(&extension);

    let mut container = match read_container_metadata(&path, &extension) {
        Ok(metadata) => metadata,
        Err(error) => ContainerMetadata {
            warnings: vec![format!("Technical metadata could not be read: {error}")],
            ..ContainerMetadata::default()
        },
    };

    let (bit_depth, channels, color_model, alpha) = container
        .color_type
        .map(|color_type| describe_color_type(color_type, &extension))
        .unwrap_or((None, None, None, None));
    let icc_profile_name = container.icc.as_deref().and_then(extract_icc_profile_name);
    let icc_profile_embedded = container.icc.is_some();

    let exif = if extension == "svg" {
        empty_group()
    } else {
        read_exif_metadata(&path)
    };
    let iptc = parse_iptc_metadata(container.iptc.take(), container.iptc_checked);
    let xmp = parse_xmp_metadata(container.xmp.take(), container.xmp_checked);
    let raw = exif
        .entries
        .iter()
        .chain(iptc.entries.iter())
        .chain(xmp.entries.iter())
        .cloned()
        .collect::<Vec<_>>();
    let privacy = build_privacy_metadata(&raw);

    Ok(ImageMetadataResponse {
        general: GeneralImageMetadata {
            format,
            mime_type,
            bit_depth,
            channels,
            animated: container.animated,
            frame_count: None,
        },
        color: ColorImageMetadata {
            color_model,
            icc_profile_checked: container.icc_checked,
            icc_profile_embedded,
            icc_profile_name,
            alpha,
        },
        exif,
        iptc,
        xmp,
        privacy,
        content_credentials: ContentCredentialsMetadata {
            status: ContentCredentialsStatus::NotChecked,
            summary: "C2PA validation is not enabled.".into(),
        },
        raw,
        warnings: container.warnings,
    })
}

fn validate_local_image_path(raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.contains("://") {
        return Err("Image metadata can only be read from local files.".into());
    }

    let path = PathBuf::from(raw_path);
    if !path.is_absolute() {
        return Err("Image metadata requires an absolute local path.".into());
    }
    if !path.is_file() {
        return Err("The selected image no longer exists or is not a file.".into());
    }

    let extension =
        extension(&path).ok_or_else(|| "The image has no supported extension.".to_string())?;
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!("Unsupported image format: {extension}"));
    }

    Ok(path)
}

fn read_container_metadata(path: &Path, extension: &str) -> Result<ContainerMetadata, String> {
    match extension {
        "jpg" | "jpeg" => {
            let decoder = JpegDecoder::new(BufReader::new(open_file(path)?))
                .map_err(|error| error.to_string())?;
            collect_decoder_metadata(decoder, Some(false), extension)
        }
        "png" => {
            let decoder = PngDecoder::new(BufReader::new(open_file(path)?))
                .map_err(|error| error.to_string())?;
            let animated = decoder.is_apng().map_err(|error| error.to_string())?;
            collect_decoder_metadata(decoder, Some(animated), extension)
        }
        "webp" => {
            let decoder = WebPDecoder::new(BufReader::new(open_file(path)?))
                .map_err(|error| error.to_string())?;
            let animated = decoder.has_animation();
            collect_decoder_metadata(decoder, Some(animated), extension)
        }
        "avif" => {
            let decoder = AvifDecoder::new(BufReader::new(open_file(path)?))
                .map_err(|error| error.to_string())?;
            collect_decoder_metadata(decoder, None, extension)
        }
        // HEIC decoding currently uses Core Image in the conversion pipeline. EXIF is still read
        // below through kamadak-exif, but color/profile claims are intentionally omitted here.
        "heic" | "heif" | "svg" => Ok(ContainerMetadata::default()),
        _ => Err(format!("Unsupported image format: {extension}")),
    }
}

fn open_file(path: &Path) -> Result<File, String> {
    File::open(path).map_err(|error| format!("Unable to open image: {error}"))
}

fn collect_decoder_metadata<D: ImageDecoder>(
    mut decoder: D,
    animated: Option<bool>,
    extension: &str,
) -> Result<ContainerMetadata, String> {
    let color_type = decoder.original_color_type();
    let mut warnings = Vec::new();
    let icc = collect_metadata_block(decoder.icc_profile(), "ICC", &mut warnings);
    let xmp_checked = matches!(extension, "jpg" | "jpeg" | "png" | "webp");
    let iptc_checked = matches!(extension, "jpg" | "jpeg" | "png");
    let xmp = xmp_checked
        .then(|| collect_metadata_block(decoder.xmp_metadata(), "XMP", &mut warnings))
        .flatten();
    let iptc = iptc_checked
        .then(|| collect_metadata_block(decoder.iptc_metadata(), "IPTC", &mut warnings))
        .flatten();

    Ok(ContainerMetadata {
        color_type: Some(color_type),
        icc_checked: true,
        icc,
        xmp,
        iptc,
        xmp_checked,
        iptc_checked,
        animated,
        warnings,
    })
}

fn collect_metadata_block(
    result: image::ImageResult<Option<Vec<u8>>>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<Vec<u8>> {
    match result {
        Ok(Some(data)) if data.len() > MAX_METADATA_BLOCK_BYTES => {
            warnings.push(format!(
                "{label} metadata exceeds the safe inspection limit."
            ));
            None
        }
        Ok(data) => data,
        Err(error) => {
            warnings.push(format!("{label} metadata could not be read: {error}"));
            None
        }
    }
}

fn describe_color_type(
    color_type: ExtendedColorType,
    extension: &str,
) -> (Option<u8>, Option<u8>, Option<String>, Option<bool>) {
    let channels = match color_type {
        ExtendedColorType::Unknown(_) => None,
        _ => Some(color_type.channel_count()),
    };
    let bit_depth = channels.and_then(|channel_count| {
        let bits_per_pixel = color_type.bits_per_pixel();
        let channel_count = u16::from(channel_count);
        (channel_count > 0 && bits_per_pixel % channel_count == 0)
            .then_some((bits_per_pixel / channel_count) as u8)
    });

    // image decodes AVIF into an RGBA8/RGBA16 buffer even when the source has no alpha channel,
    // and its 16-bit buffer can represent either 10- or 12-bit input. Keep only the exact 8-bit
    // case and avoid presenting decoder output characteristics as source-file facts.
    if extension == "avif" {
        return ((bit_depth == Some(8)).then_some(8), None, None, None);
    }

    let color_model = match color_type {
        ExtendedColorType::A8 => Some("Alpha"),
        ExtendedColorType::L1
        | ExtendedColorType::L2
        | ExtendedColorType::L4
        | ExtendedColorType::L8
        | ExtendedColorType::L16
        | ExtendedColorType::La1
        | ExtendedColorType::La2
        | ExtendedColorType::La4
        | ExtendedColorType::La8
        | ExtendedColorType::La16 => Some("Grayscale"),
        ExtendedColorType::Cmyk8 | ExtendedColorType::Cmyk16 => Some("CMYK"),
        ExtendedColorType::Unknown(_) => None,
        _ => Some("RGB"),
    }
    .map(str::to_string);
    let alpha = match color_type {
        ExtendedColorType::A8
        | ExtendedColorType::La1
        | ExtendedColorType::La2
        | ExtendedColorType::La4
        | ExtendedColorType::La8
        | ExtendedColorType::La16
        | ExtendedColorType::Rgba1
        | ExtendedColorType::Rgba2
        | ExtendedColorType::Rgba4
        | ExtendedColorType::Rgba8
        | ExtendedColorType::Rgba16
        | ExtendedColorType::Rgba32F
        | ExtendedColorType::Bgra8 => Some(true),
        ExtendedColorType::Unknown(_) => None,
        _ => Some(false),
    };

    (bit_depth, channels, color_model, alpha)
}

fn read_exif_metadata(path: &Path) -> MetadataGroup {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return unreadable_group(format!("Unable to open EXIF data: {error}")),
    };
    let mut reader = BufReader::new(file);
    let mut exif_reader = exif::Reader::new();
    exif_reader.continue_on_error(true);
    let exif = match exif_reader.read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(exif::Error::NotFound(_)) => return empty_group(),
        Err(exif::Error::PartialResult(partial)) => partial.into_inner().0,
        Err(error) => return unreadable_group(clean_error(&error.to_string())),
    };

    let mut entries = Vec::new();
    for field in exif.fields().take(MAX_METADATA_ENTRIES) {
        let key = field.tag.to_string();
        let value = sanitize_metadata_value(&field.display_value().with_unit(&exif).to_string());
        if value.is_empty() {
            continue;
        }
        entries.push(MetadataEntry {
            key: format!("Exif.{}.{}", field.ifd_num, key),
            label: humanize_key(&key),
            value,
            group: "EXIF".into(),
        });
    }

    present_group(entries)
}

fn parse_xmp_metadata(data: Option<Vec<u8>>, checked: bool) -> MetadataGroup {
    let Some(data) = data else {
        return if checked {
            empty_group()
        } else {
            not_checked_group()
        };
    };
    let text = String::from_utf8_lossy(&data);
    let document = match roxmltree::Document::parse(&text) {
        Ok(document) => document,
        Err(error) => return unreadable_group(format!("Malformed XMP metadata: {error}")),
    };

    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        for attribute in node.attributes() {
            if entries.len() >= MAX_METADATA_ENTRIES {
                break;
            }
            let key = attribute.name();
            if key.starts_with("xmlns") || key == "about" {
                continue;
            }
            push_metadata_entry(
                &mut entries,
                &mut seen,
                format!("XMP.{key}"),
                humanize_key(key),
                attribute.value(),
                "XMP",
            );
        }

        if entries.len() >= MAX_METADATA_ENTRIES || node.children().any(|child| child.is_element())
        {
            continue;
        }
        let Some(value) = node.text() else {
            continue;
        };
        let mut key = node.tag_name().name();
        if key == "li" {
            if let Some(parent) = node.parent_element() {
                if let Some(property) = parent.parent_element() {
                    key = property.tag_name().name();
                }
            }
        }
        push_metadata_entry(
            &mut entries,
            &mut seen,
            format!("XMP.{key}"),
            humanize_key(key),
            value,
            "XMP",
        );
    }

    present_group(entries)
}

fn parse_iptc_metadata(data: Option<Vec<u8>>, checked: bool) -> MetadataGroup {
    let Some(data) = data else {
        return if checked {
            empty_group()
        } else {
            not_checked_group()
        };
    };
    let mut entries = Vec::new();
    let mut index = 0;
    while index + 5 <= data.len() && entries.len() < MAX_METADATA_ENTRIES {
        if data[index] != 0x1c {
            index += 1;
            continue;
        }
        let record = data[index + 1];
        let dataset = data[index + 2];
        let length = u16::from_be_bytes([data[index + 3], data[index + 4]]);
        if length & 0x8000 != 0 {
            index += 1;
            continue;
        }
        let start = index + 5;
        let end = start.saturating_add(usize::from(length));
        if end > data.len() {
            break;
        }
        if record == 2 {
            if let Some(label) = iptc_label(dataset) {
                let value = String::from_utf8_lossy(&data[start..end]);
                let value = sanitize_metadata_value(&value);
                if !value.is_empty() {
                    entries.push(MetadataEntry {
                        key: format!("IPTC.2.{dataset}"),
                        label: label.into(),
                        value,
                        group: "IPTC".into(),
                    });
                }
            }
        }
        index = end.max(index + 1);
    }

    present_group(entries)
}

fn iptc_label(dataset: u8) -> Option<&'static str> {
    match dataset {
        5 => Some("Title"),
        25 => Some("Keywords"),
        55 => Some("Date Created"),
        80 => Some("Creator"),
        85 => Some("Creator Job Title"),
        90 => Some("City"),
        95 => Some("State / Province"),
        101 => Some("Country Code"),
        103 => Some("Job Identifier"),
        105 => Some("Headline"),
        110 => Some("Credit"),
        115 => Some("Source"),
        116 => Some("Copyright"),
        120 => Some("Caption"),
        122 => Some("Writer / Editor"),
        _ => None,
    }
}

fn push_metadata_entry(
    entries: &mut Vec<MetadataEntry>,
    seen: &mut HashSet<(String, String)>,
    key: String,
    label: String,
    raw_value: &str,
    group: &str,
) {
    let value = sanitize_metadata_value(raw_value);
    if value.is_empty() || !seen.insert((key.clone(), value.clone())) {
        return;
    }
    entries.push(MetadataEntry {
        key,
        label,
        value,
        group: group.into(),
    });
}

fn present_group(entries: Vec<MetadataEntry>) -> MetadataGroup {
    MetadataGroup {
        status: MetadataGroupStatus::Present,
        entries,
        error: None,
    }
}

fn empty_group() -> MetadataGroup {
    MetadataGroup {
        status: MetadataGroupStatus::None,
        entries: Vec::new(),
        error: None,
    }
}

fn unreadable_group(error: String) -> MetadataGroup {
    MetadataGroup {
        status: MetadataGroupStatus::Unreadable,
        entries: Vec::new(),
        error: Some(error),
    }
}

fn not_checked_group() -> MetadataGroup {
    MetadataGroup {
        status: MetadataGroupStatus::NotChecked,
        entries: Vec::new(),
        error: None,
    }
}

fn build_privacy_metadata(entries: &[MetadataEntry]) -> PrivacyMetadata {
    let mut privacy = PrivacyMetadata::default();
    for entry in entries {
        let key = format!("{} {}", entry.key, entry.label).to_ascii_lowercase();
        privacy.gps |= key.contains("gpslatitude")
            || key.contains("gpslongitude")
            || key.contains("gpsposition")
            || key.contains("latitude")
            || key.contains("longitude");
        privacy.serial_number |= key.contains("serialnumber") || key.contains("serial number");
        privacy.device_model |= key.contains("cameramodel")
            || key.contains("camera model")
            || key.ends_with(" model")
            || key.contains("lensmodel")
            || key.contains("lens model");
        privacy.creator |= key.contains("artist")
            || key.contains("creator")
            || key.contains("ownername")
            || key.contains("owner name")
            || key.contains("copyright");
        privacy.software |= key.contains("software") || key.contains("creatortool");
        privacy.timestamps |= key.contains("datetime")
            || key.contains("date created")
            || key.contains("createdate")
            || key.contains("modifydate");
    }
    privacy
}

fn extract_icc_profile_name(profile: &[u8]) -> Option<String> {
    if profile.len() < 132 {
        return None;
    }
    let tag_count = read_be_u32(profile, 128)? as usize;
    for index in 0..tag_count.min(256) {
        let record = 132 + index * 12;
        let signature = profile.get(record..record + 4)?;
        if signature != b"desc" {
            continue;
        }
        let offset = read_be_u32(profile, record + 4)? as usize;
        let size = read_be_u32(profile, record + 8)? as usize;
        let tag = profile.get(offset..offset.checked_add(size)?)?;
        if tag.starts_with(b"desc") {
            let length = read_be_u32(tag, 8)? as usize;
            let text = tag.get(12..12usize.checked_add(length.saturating_sub(1))?)?;
            let value = sanitize_metadata_value(&String::from_utf8_lossy(text));
            if !value.is_empty() {
                return Some(value);
            }
        } else if tag.starts_with(b"mluc") {
            let record_count = read_be_u32(tag, 8)? as usize;
            let record_size = read_be_u32(tag, 12)? as usize;
            if record_count == 0 || record_size < 12 {
                continue;
            }
            let length = read_be_u32(tag, 20)? as usize;
            let offset = read_be_u32(tag, 24)? as usize;
            let bytes = tag.get(offset..offset.checked_add(length)?)?;
            let utf16 = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            let value = sanitize_metadata_value(&String::from_utf16_lossy(&utf16));
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn read_be_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn sanitize_metadata_value(value: &str) -> String {
    let normalized = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect::<String>();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_METADATA_VALUE_CHARS {
        normalized
    } else {
        format!(
            "{}…",
            normalized
                .chars()
                .take(MAX_METADATA_VALUE_CHARS)
                .collect::<String>()
        )
    }
}

fn humanize_key(key: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lowercase = false;
    for character in key.replace(['_', '-'], " ").chars() {
        if character.is_uppercase() && previous_was_lowercase {
            output.push(' ');
        }
        previous_was_lowercase = character.is_lowercase() || character.is_ascii_digit();
        output.push(character);
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_error(error: &str) -> String {
    sanitize_metadata_value(error)
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

fn format_and_mime(extension: &str) -> (String, String) {
    match extension {
        "jpg" | "jpeg" => ("JPEG".into(), "image/jpeg".into()),
        "png" => ("PNG".into(), "image/png".into()),
        "webp" => ("WEBP".into(), "image/webp".into()),
        "avif" => ("AVIF".into(), "image/avif".into()),
        "svg" => ("SVG".into(), "image/svg+xml".into()),
        "heic" => ("HEIC".into(), "image/heic".into()),
        "heif" => ("HEIF".into(), "image/heif".into()),
        other => (
            other.to_ascii_uppercase(),
            "application/octet-stream".into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, ImageFormat, Rgb};
    use std::{fs, io::Cursor, process, time::SystemTime};

    fn temporary_image_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("bulkpixel-{name}-{}-{nonce}.jpg", process::id()))
    }

    fn jpeg_bytes() -> Vec<u8> {
        let image = ImageBuffer::<Rgb<u8>, _>::from_pixel(2, 2, Rgb([20, 40, 60]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("test JPEG should encode");
        bytes
    }

    fn jpeg_with_exif(tiff: &[u8]) -> Vec<u8> {
        let source = jpeg_bytes();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(tiff);
        let segment_length = u16::try_from(payload.len() + 2).expect("small test EXIF");
        let mut output = source[..2].to_vec();
        output.extend_from_slice(&[0xff, 0xe1]);
        output.extend_from_slice(&segment_length.to_be_bytes());
        output.extend_from_slice(&payload);
        output.extend_from_slice(&source[2..]);
        output
    }

    #[test]
    fn inspects_an_image_without_exif_or_icc() {
        let path = temporary_image_path("plain");
        fs::write(&path, jpeg_bytes()).expect("test image should be written");

        let metadata = inspect_image_metadata(path.to_string_lossy().into_owned())
            .expect("plain image should be inspected");

        assert_eq!(metadata.exif.status, MetadataGroupStatus::None);
        assert!(!metadata.color.icc_profile_embedded);
        fs::remove_file(path).expect("test image should be removed");
    }

    #[test]
    fn reads_exif_and_detects_gps_fields() {
        // Big-endian TIFF with one GPSInfo pointer and GPSLatitudeRef in the GPS IFD.
        let tiff = [
            b'M', b'M', 0, 42, 0, 0, 0, 8, // TIFF header
            0, 1, // one primary IFD entry
            0x88, 0x25, 0, 4, 0, 0, 0, 1, 0, 0, 0, 26, // GPSInfo -> offset 26
            0, 0, 0, 0, // next IFD
            0, 1, // one GPS IFD entry
            0, 1, 0, 2, 0, 0, 0, 2, b'N', 0, 0, 0, // GPSLatitudeRef
            0, 0, 0, 0, // next IFD
        ];
        let path = temporary_image_path("gps");
        fs::write(&path, jpeg_with_exif(&tiff)).expect("test image should be written");

        let metadata = inspect_image_metadata(path.to_string_lossy().into_owned())
            .expect("EXIF image should be inspected");

        assert_eq!(metadata.exif.status, MetadataGroupStatus::Present);
        assert!(metadata.privacy.gps);
        fs::remove_file(path).expect("test image should be removed");
    }

    #[test]
    fn reads_an_embedded_icc_description() {
        let description = b"Display P3\0";
        let tag_size = 12 + description.len();
        let mut profile = vec![0_u8; 144 + tag_size];
        profile[128..132].copy_from_slice(&1_u32.to_be_bytes());
        profile[132..136].copy_from_slice(b"desc");
        profile[136..140].copy_from_slice(&144_u32.to_be_bytes());
        profile[140..144].copy_from_slice(&(tag_size as u32).to_be_bytes());
        profile[144..148].copy_from_slice(b"desc");
        profile[152..156].copy_from_slice(&(description.len() as u32).to_be_bytes());
        profile[156..156 + description.len()].copy_from_slice(description);

        assert_eq!(
            extract_icc_profile_name(&profile).as_deref(),
            Some("Display P3")
        );
        assert_eq!(extract_icc_profile_name(&[]), None);
    }

    #[test]
    fn malformed_xmp_is_reported_without_panicking() {
        let group = parse_xmp_metadata(Some(b"<rdf:Description".to_vec()), true);

        assert_eq!(group.status, MetadataGroupStatus::Unreadable);
        assert!(group.error.is_some());
    }

    #[test]
    fn distinguishes_absent_metadata_from_metadata_that_was_not_checked() {
        assert_eq!(
            parse_xmp_metadata(None, true).status,
            MetadataGroupStatus::None
        );
        assert_eq!(
            parse_xmp_metadata(None, false).status,
            MetadataGroupStatus::NotChecked
        );
    }

    #[test]
    fn parses_common_iptc_fields() {
        let data = vec![0x1c, 2, 80, 0, 5, b'A', b'l', b'i', b'c', b'e'];
        let group = parse_iptc_metadata(Some(data), true);

        assert_eq!(group.status, MetadataGroupStatus::Present);
        assert_eq!(group.entries[0].label, "Creator");
        assert_eq!(group.entries[0].value, "Alice");
    }
}
