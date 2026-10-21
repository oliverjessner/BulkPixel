use std::{collections::HashSet, sync::Mutex};

use reqwest::Url;
use serde::Serialize;

use crate::url_import::validate_image_url;

#[derive(Default, Serialize)]
pub(crate) struct PendingImports {
    pub urls: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub(crate) struct BrowserImportState(Mutex<PendingImports>);

impl BrowserImportState {
    pub(crate) fn enqueue(&self, links: impl IntoIterator<Item = Url>) {
        let mut pending = self.0.lock().expect("browser import state poisoned");
        let mut seen: HashSet<String> = pending.urls.iter().cloned().collect();
        for link in links {
            match parse_import_link(&link) {
                Ok(url) if seen.insert(url.clone()) => {
                    if pending.urls.len() < 100 {
                        pending.urls.push(url);
                    } else if pending.errors.is_empty() {
                        pending.errors.push("The browser import queue is full. Send additional images after the current import finishes.".into());
                    }
                }
                Ok(_) => {}
                Err(error) if pending.errors.len() < 100 => pending.errors.push(error),
                Err(_) => {}
            }
        }
    }

    // Always drain a native queue instead of relying on a frontend-ready flag.
    // This retains requests during startup and webview reloads.
    pub(crate) fn take(&self) -> PendingImports {
        std::mem::take(&mut *self.0.lock().expect("browser import state poisoned"))
    }
}

pub(crate) fn import_link(raw: &str) -> Result<Url, String> {
    let image = validate_image_url(raw)?;
    let mut link = Url::parse("bulkpixel://import").expect("static import URL");
    link.query_pairs_mut().append_pair("url", image.as_str());
    Ok(link)
}

fn parse_import_link(link: &Url) -> Result<String, String> {
    if link.as_str().len() > 32768
        || link.scheme() != "bulkpixel"
        || link.host_str() != Some("import")
        || !matches!(link.path(), "" | "/")
        || !link.username().is_empty()
        || link.password().is_some()
        || link.port().is_some()
        || link.fragment().is_some()
    {
        return Err("Invalid BulkPixel browser import link.".into());
    }
    let pairs: Vec<_> = link.query_pairs().collect();
    if pairs.len() != 1 || pairs[0].0 != "url" {
        return Err("A browser import must contain exactly one image URL.".into());
    }
    validate_image_url(&pairs[0].1).map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_signed_urls_and_unicode_without_double_decoding() {
        let raw = "https://cdn.example.com/ä.png?token=a%2Bb%26c&width=42";
        let expected = validate_image_url(raw).unwrap().to_string();
        assert_eq!(
            parse_import_link(&import_link(raw).unwrap()).unwrap(),
            expected
        );
    }

    #[test]
    fn rejects_invalid_actions_parameters_and_unsafe_sources() {
        for raw in [
            "bulkpixel://convert?url=https%3A%2F%2Fexample.com%2Fa.png",
            "bulkpixel://import/path?url=https%3A%2F%2Fexample.com%2Fa.png",
            "bulkpixel://user@import?url=https%3A%2F%2Fexample.com%2Fa.png",
            "bulkpixel://import?url=https%3A%2F%2Fexample.com%2Fa.png&overwrite=true",
            "bulkpixel://import?url=https%3A%2F%2Fexample.com%2Fa.png&url=https%3A%2F%2Fexample.com%2Fb.png",
            "bulkpixel://import?url=file%3A%2F%2F%2Ftmp%2Fa.png",
            "bulkpixel://import?url=http%3A%2F%2F127.0.0.1%2Fa.png",
            "bulkpixel://import?url=https%3A%2F%2Fu%3Ap%40example.com%2Fa.png",
            "bulkpixel://import",
        ] {
            assert!(parse_import_link(&Url::parse(raw).unwrap()).is_err(), "{raw}");
        }
    }

    #[test]
    fn retains_deduplicated_requests_until_drained_and_bounds_the_queue() {
        let state = BrowserImportState::default();
        state.enqueue(
            (0..105).map(|n| import_link(&format!("https://example.com/{n}.png")).unwrap()),
        );
        state.enqueue([import_link("https://example.com/0.png").unwrap()]);
        let pending = state.take();
        assert_eq!(pending.urls.len(), 100);
        assert_eq!(pending.errors.len(), 1);
        assert!(state.take().urls.is_empty());
        state.enqueue([import_link("https://example.com/next.png").unwrap()]);
        assert_eq!(state.take().urls.len(), 1);
    }
}
