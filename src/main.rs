//! WinHelper: a tiny native Windows panel showing system load and network identity.
//! See docs/PLAN.md for the full design.

#![windows_subsystem = "windows"]

mod app;
mod ui;
mod win;

fn main() {
    if let Err(err) = app::run() {
        win::error_box("WinHelper failed to start", &err.message());
        std::process::exit(1);
    }
}
