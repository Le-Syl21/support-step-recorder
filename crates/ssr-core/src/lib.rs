//! Coeur de **Support Step Recorder** : capture des actions utilisateur
//! (clics, fenêtres, texte saisi) façon *Problem Steps Recorder* de Windows.
//!
//! - [`recorder::Recorder`] branche les événements clavier/souris sur une session.
//! - [`session::Session`] accumule les [`model::Step`] et leurs captures.
//! - [`export`] produit un rapport HTML rejouable, un `steps.json` et une archive ZIP.
//! - [`editor::EditorDocument`] est le guide en cours d'édition après l'arrêt :
//!   renommage, consignes, réordonnancement, import d'anciens enregistrements,
//!   annulation ; [`project`] le sauvegarde en `.ssrproj` éditable.
//!
//! # Community & support
//!
//! Questions, bugs, beta testing — join the Discord: <https://discord.gg/T37DYHmt2j>

pub mod archive;
pub mod assets;
pub mod capture;
#[cfg(target_os = "macos")]
pub mod capture_macos;
#[cfg(target_os = "windows")]
pub mod capture_windows;
pub mod editor;
pub mod export;
pub mod i18n;
pub mod import;
pub mod input;
pub mod model;
pub mod progress;
pub mod project;
pub mod recorder;
#[cfg(target_os = "linux")]
pub mod screencast;
pub mod session;
pub mod text;
pub mod workspace;
pub mod writer;

pub use editor::{EditKind, EditorDocument, GuideSnapshot};
pub use i18n::Lang;
pub use input::{Button, InputBackend, InputEvent};
pub use model::{Action, Step, StepKind, StepSource, WindowInfo};
pub use progress::Progress;
pub use recorder::Recorder;
pub use session::Session;
