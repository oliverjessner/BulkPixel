use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::browser_import::import_link;

const HOST_NAME: &str = "com.oli.bulkpixel.browser";
const MAX_MESSAGE_BYTES: usize = 32768;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    message: String,
}

fn valid_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

pub(crate) fn is_native_origin(origin: &str) -> bool {
    origin
        .strip_prefix("chrome-extension://")
        .is_some_and(|id| valid_extension_id(id.trim_end_matches('/')))
}

pub(crate) fn run(origin: &str) -> i32 {
    if !is_native_origin(origin) {
        return 1;
    }
    match exchange(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        launch_app,
    ) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn exchange(
    input: &mut impl Read,
    output: &mut impl Write,
    launch: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let result = (|| {
        let mut size = [0; 4];
        input
            .read_exact(&mut size)
            .map_err(|_| "The browser message is incomplete.")?;
        let size = u32::from_ne_bytes(size) as usize;
        if size == 0 || size > MAX_MESSAGE_BYTES {
            return Err("The browser message is empty or too large.".into());
        }
        let mut bytes = vec![0; size];
        input
            .read_exact(&mut bytes)
            .map_err(|_| "The browser message is incomplete.")?;
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|_| "The browser message is invalid.")?;
        if request.action != "import-image" {
            return Err("Unsupported browser action.".into());
        }
        let link = import_link(&request.url)?;
        launch(link.as_str())
    })();
    let response = match result {
        Ok(()) => Response {
            ok: true,
            message: "Opening BulkPixel. Check the app for import results.".into(),
        },
        Err(message) => Response { ok: false, message },
    };
    let bytes = serde_json::to_vec(&response).map_err(|_| "Unable to encode browser response.")?;
    output
        .write_all(&(bytes.len() as u32).to_ne_bytes())
        .and_then(|_| output.write_all(&bytes))
        .and_then(|_| output.flush())
        .map_err(|_| "Unable to send the browser response.".into())
}

fn app_bundle(executable: &Path) -> Result<PathBuf, String> {
    let macos = executable
        .parent()
        .ok_or("The app binary is unavailable.")?;
    let contents = macos.parent().ok_or("The app bundle is unavailable.")?;
    let bundle = contents.parent().ok_or("The app bundle is unavailable.")?;
    if macos.file_name().is_none_or(|name| name != "MacOS")
        || contents.file_name().is_none_or(|name| name != "Contents")
        || bundle
            .extension()
            .is_none_or(|extension| extension != "app")
    {
        return Err(
            "Use the CLI inside an installed BulkPixel.app to connect your browser.".into(),
        );
    }
    Ok(bundle.to_path_buf())
}

fn launch_app(link: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let executable = std::env::current_exe().map_err(|_| "The app binary is unavailable.")?;
        let bundle = app_bundle(&executable)?;
        let result = std::process::Command::new("/usr/bin/open")
            .arg("-a")
            .arg(bundle)
            .arg(link)
            .output()
            .map_err(|_| {
                "BulkPixel could not be opened. Reinstall the app and browser connection."
            })?;
        if result.status.success() {
            Ok(())
        } else {
            Err("BulkPixel could not be opened. Reinstall the app and browser connection.".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = link;
        Err("Browser integration is only available on macOS.".into())
    }
}

fn manifest_path(home: &Path, browser: &str) -> Result<PathBuf, String> {
    let profile = match browser {
        "chrome" => "Google/Chrome",
        "chromium" => "Chromium",
        "brave" => "BraveSoftware/Brave-Browser",
        "edge" => "Microsoft Edge",
        _ => return Err("Choose --browser chrome, chromium, brave, or edge.".into()),
    };
    Ok(home
        .join("Library/Application Support")
        .join(profile)
        .join("NativeMessagingHosts")
        .join(format!("{HOST_NAME}.json")))
}

fn install(home: &Path, browser: &str, id: &str, executable: &Path) -> Result<PathBuf, String> {
    if !valid_extension_id(id) {
        return Err(
            "--extension-id must be the 32-letter ID shown on the browser's extensions page."
                .into(),
        );
    }
    app_bundle(executable)?;
    let target = manifest_path(home, browser)?;
    let directory = target.parent().expect("native host directory");
    fs::create_dir_all(directory)
        .map_err(|_| "The browser connection folder could not be created.")?;
    let manifest = serde_json::json!({
        "name": HOST_NAME,
        "description": "Send images to BulkPixel",
        "path": executable,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{id}/")],
    });
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|_| "The browser connection could not be written.")?;
    serde_json::to_writer_pretty(&mut temporary, &manifest)
        .map_err(|_| "The browser connection could not be encoded.")?;
    temporary
        .persist(&target)
        .map_err(|_| "The browser connection could not be saved.")?;
    Ok(target)
}

pub(crate) fn handle_cli(args: &[String]) -> Result<i32, String> {
    if !cfg!(target_os = "macos") {
        return Err("Browser integration is only available on macOS.".into());
    }
    let command = args.first().map(String::as_str).unwrap_or("");
    if !matches!(command, "install" | "uninstall") {
        return Err("Use browser install --extension-id <id> [--browser chrome|chromium|brave|edge], or browser uninstall [--browser ...].".into());
    }
    let mut browser = "chrome";
    let mut id = None;
    let mut index = 1;
    while index < args.len() {
        let value = args.get(index + 1).ok_or("Missing browser option value.")?;
        match args[index].as_str() {
            "--browser" => browser = value.as_str(),
            "--extension-id" if command == "install" => id = Some(value.as_str()),
            _ => return Err("Unknown browser option.".into()),
        }
        index += 2;
    }
    let home = dirs::home_dir().ok_or("The home directory is unavailable.")?;
    if command == "install" {
        let executable = std::env::current_exe().map_err(|_| "The app binary is unavailable.")?;
        let target = install(
            &home,
            browser,
            id.ok_or("--extension-id is required.")?,
            &executable,
        )?;
        println!(
            "Browser connected: {browser}\nManifest: {}",
            target.display()
        );
    } else {
        let target = manifest_path(&home, browser)?;
        match fs::remove_file(&target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("The browser connection could not be removed.".into()),
        }
        println!("Browser disconnected: {browser}");
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frame(value: serde_json::Value) -> Cursor<Vec<u8>> {
        let bytes = serde_json::to_vec(&value).unwrap();
        Cursor::new([(bytes.len() as u32).to_ne_bytes().as_slice(), &bytes].concat())
    }

    fn response(bytes: &[u8]) -> Response {
        let size = u32::from_ne_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(size, bytes.len() - 4);
        serde_json::from_slice(&bytes[4..]).unwrap()
    }

    #[test]
    fn native_protocol_hands_off_one_image_and_returns_framed_json() {
        let raw = "https://example.com/image?token=a%2Bb&width=42";
        let mut output = Vec::new();
        exchange(
            &mut frame(serde_json::json!({"action":"import-image", "url":raw})),
            &mut output,
            |link| {
                assert_eq!(link, import_link(raw).unwrap().as_str());
                Ok(())
            },
        )
        .unwrap();
        assert!(response(&output).ok);
    }

    #[test]
    fn rejects_invalid_frames_and_payloads_without_launching() {
        let mut inputs = vec![
            Cursor::new(0_u32.to_ne_bytes().to_vec()),
            Cursor::new(u32::MAX.to_ne_bytes().to_vec()),
            Cursor::new(vec![1, 0]),
            Cursor::new(vec![10, 0, 0, 0, b'{']),
        ];
        for value in [
            serde_json::json!({"action":"convert", "url":"https://example.com/a.png"}),
            serde_json::json!({"action":"import-image", "url":"file:///tmp/a.png"}),
            serde_json::json!({"action":"import-image", "url":"http://127.0.0.1/a.png"}),
            serde_json::json!({"action":"import-image", "url":"https://example.com/a.png", "overwrite":true}),
        ] {
            inputs.push(frame(value));
        }
        for mut input in inputs {
            let mut output = Vec::new();
            exchange(&mut input, &mut output, |_| panic!("must not launch")).unwrap();
            assert!(!response(&output).ok);
        }
    }

    #[test]
    fn reports_launch_failures_to_the_browser() {
        let mut output = Vec::new();
        exchange(
            &mut frame(
                serde_json::json!({"action":"import-image", "url":"https://example.com/a.png"}),
            ),
            &mut output,
            |_| Err("Cannot open app.".into()),
        )
        .unwrap();
        assert_eq!(response(&output).message, "Cannot open app.");
    }

    #[test]
    fn installs_only_the_selected_origin_and_browser_in_an_isolated_home() {
        let home = tempfile::tempdir().unwrap();
        let id = "abcdefghijklmnopabcdefghijklmnop";
        let executable = Path::new("/Applications/BulkPixel.app/Contents/MacOS/bulkpixel");
        for browser in ["chrome", "chromium", "brave", "edge"] {
            let target = install(home.path(), browser, id, executable).unwrap();
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(target).unwrap()).unwrap();
            assert_eq!(value["path"], executable.to_str().unwrap());
            assert_eq!(
                value["allowed_origins"],
                serde_json::json!([format!("chrome-extension://{id}/")])
            );
        }
        assert!(install(home.path(), "unknown", id, executable).is_err());
        assert!(install(home.path(), "chrome", "../../invalid", executable).is_err());
        assert!(install(home.path(), "chrome", id, Path::new("/tmp/bulkpixel")).is_err());
        assert!(is_native_origin(&format!("chrome-extension://{id}/")));
        assert!(!is_native_origin("chrome-extension://invalid/"));
    }
}
