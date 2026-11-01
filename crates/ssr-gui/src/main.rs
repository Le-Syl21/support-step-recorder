#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! egui/glow interface of Support Step Recorder: record a session, review
//! its steps, save the report.
//!
//! # Community & support
//!
//! Questions, bugs, beta testing — join the Discord: <https://discord.gg/T37DYHmt2j>

mod app;
mod chrome;
mod dialogs;
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("support-step-recorder")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([520.0, 420.0]),
        // Rendering through glow (OpenGL), explicitly.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
