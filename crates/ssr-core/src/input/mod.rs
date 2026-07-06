//! Abstraction de capture d'entrées : un backend émet des [`InputEvent`]
//! normalisés, indépendamment de la plateforme.
//!
//! - X11 / Windows / macOS → [`devicequery::DeviceQueryBackend`]
//! - Wayland (Linux) → [`evdev_backend::EvdevBackend`] (lecture de `/dev/input`)

use std::any::Any;
use std::io;
use std::sync::mpsc::Sender;

pub mod devicequery;
#[cfg(target_os = "linux")]
pub mod evdev_backend;

/// Touche normalisée : on réutilise l'enum de `device_query` comme pivot
/// (déjà consommé par [`crate::text`]).
pub use device_query::Keycode;

/// Bouton de souris normalisé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Left,
    Middle,
    Right,
    Other(u16),
}

impl Button {
    /// Libellé du bouton dans la langue donnée.
    pub fn label(self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::Lang::{En, Fr};
        match (lang, self) {
            (Fr, Button::Left) => "gauche".into(),
            (Fr, Button::Middle) => "milieu".into(),
            (Fr, Button::Right) => "droit".into(),
            (Fr, Button::Other(n)) => format!("bouton {n}"),
            (En, Button::Left) => "left".into(),
            (En, Button::Middle) => "middle".into(),
            (En, Button::Right) => "right".into(),
            (En, Button::Other(n)) => format!("button {n}"),
        }
    }
}

/// Événement d'entrée normalisé, indépendant du backend.
#[derive(Clone, Debug)]
pub enum InputEvent {
    /// Clic relâché. `pos` = position absolue du curseur si le backend la connaît
    /// (X11/Windows/macOS), `None` sur Wayland/evdev.
    MouseUp {
        button: Button,
        pos: Option<(i32, i32)>,
    },
    KeyDown(Keycode),
    KeyUp(Keycode),
}

/// Garde de capture : la capture s'arrête quand cette poignée est détruite.
pub struct InputHandle {
    _inner: Box<dyn Any + Send>,
}

impl InputHandle {
    pub(crate) fn new(inner: impl Any + Send) -> Self {
        Self {
            _inner: Box::new(inner),
        }
    }
}

/// Backend de capture d'entrées.
pub trait InputBackend: Send {
    /// Nom lisible (pour les logs/diagnostic).
    fn name(&self) -> &'static str;
    /// Démarre la capture ; les événements sont poussés dans `tx`.
    fn start(&self, tx: Sender<InputEvent>) -> io::Result<InputHandle>;
}

/// Sélectionne le backend adapté à l'environnement courant.
pub fn select() -> Box<dyn InputBackend> {
    #[cfg(target_os = "linux")]
    {
        if is_wayland() {
            return Box::new(evdev_backend::EvdevBackend);
        }
    }
    Box::new(devicequery::DeviceQueryBackend)
}

/// Détecte une session Wayland.
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}
