//! Ferrum Browser — a desktop browser written in Rust.
//!
//! Phase 1 · Foundation.
//!
//! This file is the *entry point*: it owns the process, configures the native
//! window, and hands control to eframe's event loop. Everything the browser
//! actually does lives in `app.rs`; address parsing lives in `url_input.rs`.

// Hide the console window in release builds on Windows. Debug builds keep it
// so `println!` and panic messages stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod url_input;

use app::BrowserApp;
use eframe::egui;

fn main() -> eframe::Result {
    // NativeOptions configures the OS window and the rendering backend.
    // `..Default::default()` keeps the defaults for everything we don't touch.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ferrum Browser")
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    // `run_native` blocks for the lifetime of the app:
    //   1. create the window + graphics context,
    //   2. call our creator closure once -> `BrowserApp::new`,
    //   3. call `App::ui` for every frame until the window closes,
    //   4. call `App::save` / `App::on_exit` on shutdown.
    eframe::run_native(
        "Ferrum Browser",
        options,
        Box::new(|cc| Ok(Box::new(BrowserApp::new(cc)))),
    )
}
