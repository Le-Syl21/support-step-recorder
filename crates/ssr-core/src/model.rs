//! Modèle de données d'une session de capture.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::i18n::Lang;
use crate::input::Button;

/// Informations sur la fenêtre active au moment d'une action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    pub title: String,
    pub app_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Nature d'une étape enregistrée.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Clic souris. Coordonnées globales et relatives à la fenêtre — `None`
    /// quand la plateforme ne les fournit pas (Wayland/evdev).
    Click {
        button: Button,
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rel_x: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rel_y: Option<i32>,
    },
    /// Texte saisi au clavier (agrégé entre deux clics).
    Text { content: String },
}

/// Origine d'une étape : capturée par l'enregistreur, ou rédigée à la main
/// dans l'éditeur (pas de clic inventé).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    #[default]
    Captured,
    Manual,
}

impl StepKind {
    fn is_captured(&self) -> bool {
        *self == StepKind::Captured
    }
}

/// D'où vient une étape importée depuis un autre enregistrement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSource {
    /// Nom de l'archive source (ex. `network-setup.zip`).
    pub source_label: String,
    /// SHA-256 (hex) de l'archive source.
    pub source_digest: String,
    /// Numéro de l'étape dans la source.
    pub original_index: usize,
    /// Identifiant de l'étape dans la source, s'il existait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_step_id: Option<String>,
}

/// Une étape de la session : une action, son contexte et sa capture éventuelle.
///
/// Les champs ajoutés par l'éditeur (`id`, `title_override`, `notes`,
/// `source`, `kind`) ont tous une valeur par défaut : un `steps.json` de la
/// v0.2.0 se relit tel quel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Identifiant stable dans le guide (l'ordre, lui, est donné par `index`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub index: usize,
    /// Horodatage absolu (ms depuis l'epoch Unix).
    pub timestamp_ms: u128,
    /// Temps écoulé depuis le début de la session (ms).
    pub elapsed_ms: u128,
    pub action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowInfo>,
    /// Nom du fichier WebP de la capture, relatif au dossier de session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// Description lisible de l'étape, façon MS PSR (dans la langue de la session).
    /// Toujours le texte généré à la capture ; un titre saisi dans l'éditeur
    /// va dans `title_override`.
    pub description: String,
    /// Titre saisi dans l'éditeur ; vide ou absent = titre généré.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_override: Option<String>,
    /// Consigne détaillée (texte brut, retours à la ligne conservés).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Enregistrement d'origine, pour une étape importée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<StepSource>,
    #[serde(default, skip_serializing_if = "StepKind::is_captured")]
    pub kind: StepKind,
}

impl Step {
    /// Une étape vide rédigée à la main (sans action enregistrée).
    pub fn manual(title: impl Into<String>, screenshot: Option<String>) -> Self {
        let title = title.into();
        Self {
            id: Some(Uuid::new_v4()),
            index: 0,
            timestamp_ms: crate::session::now_ms(),
            elapsed_ms: 0,
            action: Action::Text {
                content: String::new(),
            },
            window: None,
            screenshot,
            description: String::new(),
            title_override: Some(title).filter(|t| !t.trim().is_empty()),
            notes: String::new(),
            source: None,
            kind: StepKind::Manual,
        }
    }

    /// Le titre saisi dans l'éditeur, s'il n'est pas vide.
    pub fn custom_title(&self) -> Option<&str> {
        self.title_override
            .as_deref()
            .filter(|t| !t.trim().is_empty())
    }

    /// Le titre généré, dans la langue donnée. Une fenêtre sans titre (fréquent
    /// sous X11) est nommée d'après son application plutôt que « «  » ».
    pub fn auto_title(&self, lang: Lang) -> String {
        if self.kind == StepKind::Manual {
            return lang.pick("Étape sans titre", "Untitled step").to_string();
        }
        let window = self.window.as_ref().map(|w| {
            if w.title.is_empty() && !w.app_name.is_empty() {
                WindowInfo {
                    title: w.app_name.clone(),
                    app_name: String::new(),
                    ..w.clone()
                }
            } else {
                w.clone()
            }
        });
        Self::build_description(&self.action, window.as_ref(), lang)
    }

    /// Le titre affiché : celui de l'éditeur, sinon le titre généré.
    pub fn display_title(&self, lang: Lang) -> String {
        self.custom_title()
            .map(str::to_owned)
            .unwrap_or_else(|| self.auto_title(lang))
    }

    /// Construit la description lisible à partir de l'action et du contexte,
    /// dans la langue donnée.
    pub fn build_description(action: &Action, window: Option<&WindowInfo>, lang: Lang) -> String {
        let target = window
            .map(|w| {
                if w.app_name.is_empty() {
                    format!("« {} »", w.title)
                } else {
                    format!("« {} » ({})", w.title, w.app_name)
                }
            })
            .unwrap_or_else(|| match lang {
                Lang::Fr => "la fenêtre active".to_string(),
                Lang::En => "the active window".to_string(),
            });

        match action {
            Action::Click { button, .. } => {
                let b = button.label(lang);
                match lang {
                    Lang::Fr => format!("Clic {b} sur {target}"),
                    Lang::En => format!("{} click on {target}", capitalize(&b)),
                }
            }
            Action::Text { content } => {
                let shown = content.replace('\n', "⏎");
                match lang {
                    Lang::Fr => format!("Saisie « {shown} » dans {target}"),
                    Lang::En => format!("Typed “{shown}” in {target}"),
                }
            }
        }
    }
}

/// Met la première lettre en majuscule (« left » → « Left »).
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
