#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os().nth(1).is_some() {
        std::process::exit(bulkpixel_lib::run_cli_from_env());
    }

    bulkpixel_lib::run()
}
