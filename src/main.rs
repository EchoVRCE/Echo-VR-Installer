#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod ui;
mod version;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Elevated helper mode: the app relaunches itself with this flag (as admin) to perform
    // privileged operations for the normal process. Never starts the GUI.
    if args.get(1).map(String::as_str) == Some(core::elevation::HELPER_FLAG) {
        core::log::init("admin-helper.log");
        std::process::exit(core::elevation::helper_main(&args));
    }

    core::log::init("EchoVR_Installer.log");
    let result = ui::run();
    core::elevation::shutdown();
    if let Err(e) = result {
        tracing::error!("fatal: {e}");
        eprintln!("{e}");
        std::process::exit(1);
    }
}
