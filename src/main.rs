pub mod app;
pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod launcher;
pub mod ping;

fn main() {
    if let Err(e) = app::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
