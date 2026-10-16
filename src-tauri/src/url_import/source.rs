use std::{fs, io::Read, net::IpAddr, path::Path};

use image::ImageFormat;
use reqwest::{header::HeaderMap, Url};

pub(super) fn parse_url(raw: &str) -> Result<Url, String> {
    let raw = raw.trim();
    if raw.len() > 8192 || raw.chars().any(char::is_control) {
        return Err("The URL is too long or contains invalid characters.".into());
    }
    let mut url = Url::parse(raw).map_err(|_| "Enter a complete HTTP or HTTPS image URL.")?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS image URLs are supported.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URLs containing usernames or passwords are not supported.".into());
    }
    let host = url
        .host_str()
        .ok_or("The URL needs a hostname.")?
        .trim_end_matches('.');
    if host.eq_ignore_ascii_case("localhost")
        || [".localhost", ".local", ".internal", ".home.arpa"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
        || !host.contains('.') && !host.contains(':')
    {
        return Err("Local and private-network image URLs are blocked.".into());
    }
    if host
        .trim_matches(['[', ']'])
        .parse::<IpAddr>()
        .ok()
        .is_some_and(|ip| !is_public_ip(ip))
    {
        return Err("Local and private-network image URLs are blocked.".into());
    }
    if url.port() == Some(0) {
        return Err("The URL contains an invalid port.".into());
    }
    url.set_fragment(None);
    Ok(url)
}

pub(super) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || a == 100 && (64..=127).contains(&b)
                || a == 169 && b == 254
                || a == 172 && (16..=31).contains(&b)
                || a == 192 && (b == 168 || b == 0 && c <= 2 || b == 88 && c == 99)
                || a == 198 && (b == 18 || b == 19 || b == 51 && c == 100)
                || a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Global unicast only. Exclude transition, benchmark and documentation ranges.
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && (s[0] & 0xfff0) != 0x3ff0
        }
    }
}

pub(super) fn detect_format(path: &Path, content_type: &str) -> Result<&'static str, String> {
    let mut header = [0_u8; 4096];
    let count = fs::File::open(path)
        .and_then(|mut file| file.read(&mut header))
        .map_err(|_| "The downloaded image could not be read.")?;
    let data = &header[..count];
    if data.starts_with(&[0xff, 0x0a]) || data.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        return Ok("jxl");
    }
    if data.starts_with(b"\0\0\0\x0cjP  \r\n\x87\n") {
        return Ok("jp2");
    }
    if data.len() >= 16 && &data[4..8] == b"ftyp" {
        let box_len = u32::from_be_bytes(data[..4].try_into().unwrap()) as usize;
        let brands = data
            .get(8..box_len.min(data.len()))
            .unwrap_or_default()
            .chunks_exact(4)
            .collect::<Vec<_>>();
        if brands
            .iter()
            .any(|brand| matches!(*brand, b"avif" | b"avis"))
        {
            return Ok("avif");
        }
        if brands.iter().any(|brand| {
            matches!(
                *brand,
                b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1"
            )
        }) {
            return Ok("heic");
        }
    }
    if let Ok(format) = image::guess_format(data) {
        return match format {
            ImageFormat::Jpeg => Ok("jpg"),
            ImageFormat::Png => Ok("png"),
            ImageFormat::WebP => Ok("webp"),
            ImageFormat::Avif => Ok("avif"),
            ImageFormat::Tiff => Ok("tiff"),
            ImageFormat::Gif => Ok("gif"),
            ImageFormat::Ico => Ok("ico"),
            _ => Err("This image format is not supported by BulkPixel.".into()),
        };
    }
    // Content-Type is a hint, never proof of image content. Inspect XML's actual root.
    let starts_xml = String::from_utf8_lossy(data)
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<');
    if starts_xml
        || content_type
            .split(';')
            .next()
            .is_some_and(|mime| mime.trim() == "image/svg+xml")
    {
        let text =
            fs::read_to_string(path).map_err(|_| "The response is not a valid SVG image.")?;
        let doc = roxmltree::Document::parse(&text)
            .map_err(|_| "The response is not a valid SVG image.")?;
        let root = doc.root_element();
        if root.tag_name().name() == "svg"
            && matches!(
                root.tag_name().namespace(),
                None | Some("http://www.w3.org/2000/svg")
            )
        {
            validate_remote_svg(&doc)?;
            return Ok("svg");
        }
    }
    Err("The server did not return a supported image (it may be an HTML or JSON page).".into())
}

// The shared SVG renderer never executes scripts, but can resolve local image hrefs.
// Admit self-contained remote SVGs only, without changing or recompressing their bytes.
fn validate_remote_svg(doc: &roxmltree::Document<'_>) -> Result<(), String> {
    for node in doc.descendants() {
        if node.pi().is_some_and(|pi| pi.target == "xml-stylesheet") {
            return Err("URL-imported SVG images must not reference external resources.".into());
        }
        for attr in node.attributes() {
            if matches!(attr.name(), "href" | "src") && !safe_svg_reference(attr.value()) {
                return Err("URL-imported SVG images must not reference external resources or embedded SVGs.".into());
            }
            if attr.name() == "style" || attr.value().to_ascii_lowercase().contains("url") {
                validate_svg_css(attr.value())?;
            }
        }
        if node.is_element() && node.tag_name().name() == "style" {
            validate_svg_css(
                &node
                    .descendants()
                    .filter_map(|n| n.is_text().then(|| n.text()).flatten())
                    .collect::<String>(),
            )?;
        }
    }
    Ok(())
}

fn safe_svg_reference(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('#')
        || [
            "data:image/png;",
            "data:image/jpeg;",
            "data:image/webp;",
            "data:image/gif;",
            "data:image/avif;",
        ]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

fn validate_svg_css(value: &str) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if lower.contains('\\') {
        return Err(
            "URL-imported SVG styles must be self-contained, without escaped resource references."
                .into(),
        );
    }
    let mut cleaned = String::new();
    let mut pending = lower.as_str();
    while let Some(index) = pending.find("/*") {
        cleaned.push_str(&pending[..index]);
        let (_, rest) = pending[index + 2..]
            .split_once("*/")
            .ok_or("The SVG contains an invalid style comment.")?;
        pending = rest;
    }
    cleaned.push_str(pending);
    if cleaned.contains("@import") {
        return Err("URL-imported SVG styles must not import external resources.".into());
    }
    let mut rest = cleaned.as_str();
    while let Some(index) = rest.find("url") {
        rest = rest[index + 3..].trim_start();
        if let Some(value) = rest.strip_prefix('(') {
            let (reference, remaining) = value
                .split_once(')')
                .ok_or("The SVG contains an invalid resource reference.")?;
            if !safe_svg_reference(reference.trim().trim_matches(['\'', '"'])) {
                return Err(
                    "URL-imported SVG images must not reference external resources.".into(),
                );
            }
            rest = remaining;
        }
    }
    Ok(())
}

pub(super) fn image_filename(
    headers: &HeaderMap,
    url: &Url,
    extension: &str,
    index: usize,
) -> String {
    let disposition = headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok());
    let suggested = disposition.and_then(disposition_filename).or_else(|| {
        url.path_segments()
            .and_then(|mut parts| parts.next_back())
            .filter(|name| !name.is_empty())
            .map(percent_decode)
    });
    let raw = suggested.as_deref().unwrap_or("");
    let basename = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = basename.rsplit_once('.').map_or(basename, |(stem, _)| stem);
    let mut safe = String::new();
    for ch in stem.chars() {
        let ch = if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ') {
            ch
        } else {
            '_'
        };
        if safe.len() + ch.len_utf8() > 160 {
            break;
        }
        safe.push(ch);
    }
    let safe = safe.trim_matches(['.', ' ', '_']);
    let stem = if safe.is_empty() {
        format!("image-{:03}", index + 1)
    } else {
        safe.to_string()
    };
    format!("{stem}.{extension}")
}

fn disposition_filename(header: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in header.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quoted {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch == ';' && !quoted {
            parts.push(&header[start..index]);
            start = index + 1;
        }
    }
    parts.push(&header[start..]);
    let mut plain = None;
    for part in parts {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if key.eq_ignore_ascii_case("filename*") {
            if let Some((encoding, rest)) = value.split_once('\'') {
                if encoding.eq_ignore_ascii_case("utf-8") {
                    if let Some((_, encoded)) = rest.split_once('\'') {
                        return Some(percent_decode(encoded));
                    }
                }
            }
        } else if key.eq_ignore_ascii_case("filename") && !value.is_empty() {
            plain = Some(value.replace("\\\"", "\""));
        }
    }
    plain
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                decoded.push(value);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
