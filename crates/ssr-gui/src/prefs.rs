//! Preferences remembered across runs: theme, zoom, language, the privacy
//! notice and the last folder used in a file dialog (never any content).
//!
//! They go through eframe's own storage (the `persistence` feature): it
//! already knows the per-OS application data folder (XDG on Linux,
//! `%APPDATA%` on Windows, `Application Support` on macOS), writes atomically
//! on exit and on a timer, and also brings back the window size and position.
//! A hand-rolled config file would duplicate all of that.

use egui::ThemePreference;
use serde::{Deserialize, Serialize};
use ssr_core::Lang;

use crate::theme;

const KEY: &str = "ssr-prefs";

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Prefs {
    /// "light", "dark" or "system".
    pub theme: String,
    pub zoom: f32,
    /// "fr" / "en" once the user picked one; absent while the language
    /// still follows the system locale.
    pub lang: Option<String>,
    /// The keyboard-capture privacy notice was acknowledged.
    pub privacy_ack: bool,
    /// Folder of the last project / recording / export picked.
    pub last_dir: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            zoom: 1.0,
            lang: None,
            privacy_ack: false,
            last_dir: None,
        }
    }
}

impl Prefs {
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|s| eframe::get_value(s, KEY))
            .unwrap_or_default()
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, KEY, self);
    }

    pub fn theme(&self) -> ThemePreference {
        match self.theme.as_str() {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        }
    }

    pub fn set_theme(&mut self, pref: ThemePreference) {
        self.theme = match pref {
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
            ThemePreference::System => "system",
        }
        .into();
    }

    /// The remembered language, else the one of the system locale.
    pub fn lang(&self) -> Lang {
        match self.lang.as_deref() {
            Some("fr") => Lang::Fr,
            Some("en") => Lang::En,
            _ => Lang::detect(),
        }
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = Some(lang.code().to_string());
    }

    /// The folder to open file dialogs in, if it still exists.
    pub fn last_dir(&self) -> Option<std::path::PathBuf> {
        self.last_dir
            .as_ref()
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_dir())
    }

    /// Remember the folder of `path` for the next dialog.
    pub fn remember_dir(&mut self, path: &std::path::Path) {
        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            self.last_dir = Some(d.to_string_lossy().into_owned());
        }
    }

    pub fn zoom(&self) -> f32 {
        theme::clamp_zoom(self.zoom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_round_trips() {
        let mut p = Prefs::default();
        for t in [
            ThemePreference::Light,
            ThemePreference::Dark,
            ThemePreference::System,
        ] {
            p.set_theme(t);
            assert_eq!(p.theme(), t);
        }
    }

    #[test]
    fn chosen_language_wins_over_the_locale() {
        let mut p = Prefs::default();
        p.set_lang(Lang::En);
        assert_eq!(p.lang(), Lang::En);
        p.set_lang(Lang::Fr);
        assert_eq!(p.lang(), Lang::Fr);
    }
}
