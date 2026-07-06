//! Session de capture : accumule les étapes et gère le tampon de texte.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};

use crate::model::{Action, Step, WindowInfo};
use crate::text::KeyToken;

/// Une session d'enregistrement vivante.
pub struct Session {
    /// Dossier où sont écrites les captures et le rapport.
    pub dir: PathBuf,
    /// Date de début (pour le rapport).
    pub started_at: DateTime<Local>,
    /// Étapes enregistrées dans l'ordre chronologique.
    pub steps: Vec<Step>,
    /// `true` tant que la capture est active.
    pub recording: bool,
    /// État de la touche Maj (suivi pour la casse du texte saisi).
    pub shift_down: bool,
    /// Langue dans laquelle les descriptions d'étapes sont construites.
    pub lang: crate::i18n::Lang,

    start_ms: u128,
    pending_text: String,
    screenshot_counter: usize,
}

impl Session {
    /// Crée une session et son dossier de sortie.
    pub fn new(dir: impl AsRef<Path>, lang: crate::i18n::Lang) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            started_at: Local::now(),
            steps: Vec::new(),
            recording: true,
            shift_down: false,
            lang,
            start_ms: now_ms(),
            pending_text: String::new(),
            screenshot_counter: 0,
        })
    }

    /// Applique un jeton clavier au tampon de texte courant.
    pub fn apply_key(&mut self, token: KeyToken) {
        match token {
            KeyToken::Insert(s) => self.pending_text.push_str(&s),
            KeyToken::Backspace => {
                self.pending_text.pop();
            }
        }
    }

    /// Vide le tampon de texte en une étape `Text`, s'il n'est pas vide.
    pub fn flush_text(&mut self, window: Option<WindowInfo>) {
        let content = std::mem::take(&mut self.pending_text);
        if content.trim().is_empty() {
            return;
        }
        let action = Action::Text { content };
        self.push_step(action, window, None);
    }

    /// Enregistre un clic (le texte en attente est d'abord vidé).
    pub fn record_click(
        &mut self,
        button: crate::input::Button,
        global: Option<(i32, i32)>,
        window: Option<WindowInfo>,
        screenshot: Option<String>,
    ) {
        // Le texte tapé avant ce clic constitue une étape distincte qui le précède.
        self.flush_text(window.clone());

        // Position connue sur X11/Windows/macOS ; absente sur Wayland (evdev).
        let pos = global.map(|(x, y)| {
            let (rx, ry) = match &window {
                Some(w) => (x - w.x, y - w.y),
                None => (x, y),
            };
            (x, y, rx, ry)
        });
        let action = match pos {
            Some((x, y, rel_x, rel_y)) => Action::Click {
                button,
                x: Some(x),
                y: Some(y),
                rel_x: Some(rel_x),
                rel_y: Some(rel_y),
            },
            None => Action::Click {
                button,
                x: None,
                y: None,
                rel_x: None,
                rel_y: None,
            },
        };
        self.push_step(action, window, screenshot);
    }

    /// Réserve le prochain nom de fichier de capture.
    pub fn next_screenshot_name(&mut self) -> String {
        self.screenshot_counter += 1;
        format!("step-{:04}.webp", self.screenshot_counter)
    }

    /// Stoppe la session et vide le texte restant.
    pub fn stop(&mut self) {
        self.flush_text(None);
        self.recording = false;
    }

    fn push_step(
        &mut self,
        action: Action,
        window: Option<WindowInfo>,
        screenshot: Option<String>,
    ) {
        let now = now_ms();
        let description = Step::build_description(&action, window.as_ref(), self.lang);
        self.steps.push(Step {
            index: self.steps.len() + 1,
            timestamp_ms: now,
            elapsed_ms: now.saturating_sub(self.start_ms),
            action,
            window,
            screenshot,
            description,
        });
    }
}

pub(crate) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
