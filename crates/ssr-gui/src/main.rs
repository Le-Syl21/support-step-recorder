#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! egui/glow interface of Support Step Recorder: record a session, edit it
//! into a guide (and merge earlier recordings), save the project, export the
//! report.
//!
//! # Community & support
//!
//! Questions, bugs, beta testing — join the Discord: <https://discord.gg/T37DYHmt2j>

mod app;
mod chrome;
mod dialogs;
mod editor_sources;
mod icons;
mod prefs;
mod review;
mod screens;
mod steps;
mod textures;
mod theme;
mod viewer;
mod widgets;

use eframe::egui;

/// Name of the window, and of eframe's storage folder for the preferences.
const APP_NAME: &str = "Support Step Recorder";

fn main() -> eframe::Result {
    // `ssr guide.ssrproj` / `ssr recording.zip`: open that file at start.
    let open = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_file());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("support-step-recorder")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([520.0, 420.0]),
        // Rendering through glow (OpenGL), explicitly.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, open)))),
    )
}
