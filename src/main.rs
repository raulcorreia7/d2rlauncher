#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod app;
pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod launcher;
pub mod logger;
pub mod ping;

fn main() {
    logger::init();

    if let Err(e) = app::run() {
        logln!("Error: {e}");
        std::process::exit(1);
    }
}
