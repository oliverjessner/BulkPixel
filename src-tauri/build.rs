fn main() {
    println!("cargo:rerun-if-changed=../scripts/extract-website-images.cjs");
    println!("cargo:rerun-if-changed=../scripts/prepare-url-import.mjs");
    println!("cargo:rerun-if-changed=../package-lock.json");
    let status = std::process::Command::new("node")
        .arg("../scripts/prepare-url-import.mjs")
        .env("TAURI_ENV_TARGET_TRIPLE", std::env::var("TARGET").unwrap())
        .status()
        .expect("Node.js is required to prepare Fetchary for website imports");
    assert!(
        status.success(),
        "Website import runtime preparation failed"
    );
    // Keep another preparation from replacing resources while Tauri copies them.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("../.url-import-cache/prepare.lock")
        .expect("Failed to open website runtime lock");
    lock.lock()
        .expect("Failed to lock website runtime resources");
    tauri_build::build()
}
