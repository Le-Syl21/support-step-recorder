//! What the GUI shows of a session or a guide: a lock-free copy of its
//! steps, and the formatting of their metadata.

use std::path::PathBuf;
use std::sync::Mutex;

use ssr_core::{
    session::Session, Action, EditorDocument, Lang, Step, StepKind, StepSource, WindowInfo,
};
use uuid::Uuid;

/// One step copied out for display (no lock held).
///
/// The raw action and window are kept (not the already translated text) so
/// the description can be rebuilt in the current language.
#[derive(Clone)]
pub struct StepRow {
    pub id: Uuid,
    pub index: usize,
    pub screenshot: Option<String>,
    /// Absolute capture time (ms since the Unix epoch).
    pub timestamp_ms: u128,
    /// Time since the start of the session (ms).
    pub elapsed_ms: u128,
    pub action: Action,
    pub window: Option<WindowInfo>,
    pub title_override: Option<String>,
    pub notes: String,
    pub kind: StepKind,
    pub source: Option<StepSource>,
}

impl StepRow {
    fn from_step(st: &Step) -> Self {
        Self {
            // Steps recorded by this version always have an id; a missing one
            // gets a throw-away id (display only).
            id: st.id.unwrap_or_else(Uuid::new_v4),
            index: st.index,
            screenshot: st.screenshot.clone(),
            timestamp_ms: st.timestamp_ms,
            elapsed_ms: st.elapsed_ms,
            action: st.action.clone(),
            window: st.window.clone(),
            title_override: st.title_override.clone(),
            notes: st.notes.clone(),
            kind: st.kind,
            source: st.source.clone(),
        }
    }

    fn as_step(&self) -> Step {
        Step {
            id: Some(self.id),
            index: self.index,
            timestamp_ms: self.timestamp_ms,
            elapsed_ms: self.elapsed_ms,
            action: self.action.clone(),
            window: self.window.clone(),
            screenshot: None,
            description: String::new(),
            title_override: self.title_override.clone(),
            notes: String::new(),
            source: None,
            kind: self.kind,
        }
    }

    /// The generated description in the given language (custom title aside).
    pub fn auto_description(&self, lang: Lang) -> String {
        self.as_step().auto_title(lang)
    }

    /// Readable title: the custom one, else the generated description.
    pub fn description(&self, lang: Lang) -> String {
        self.as_step().display_title(lang)
    }

    pub fn custom_title(&self) -> Option<&str> {
        self.title_override
            .as_deref()
            .filter(|t| !t.trim().is_empty())
    }

    pub fn is_click(&self) -> bool {
        matches!(self.action, Action::Click { .. })
    }

    pub fn is_manual(&self) -> bool {
        self.kind == StepKind::Manual
    }

    /// A short title for the list: the custom title, or "Left click" /
    /// "Typing" / "Manual step".
    pub fn short_title(&self, lang: Lang) -> String {
        if let Some(t) = self.custom_title() {
            return t.to_string();
        }
        if self.is_manual() {
            return lang.pick("Étape rédigée", "Written step").to_string();
        }
        match &self.action {
            Action::Click { button, .. } => {
                let b = button.label(lang);
                match lang {
                    Lang::Fr => format!("Clic {b}"),
                    Lang::En => format!("{} click", capitalize(&b)),
                }
            }
            Action::Text { .. } => lang.pick("Saisie au clavier", "Typing").to_string(),
        }
    }

    /// The window title, or the application, or nothing: the list's subtitle.
    pub fn target(&self) -> Option<String> {
        let w = self.window.as_ref()?;
        if !w.title.is_empty() {
            Some(w.title.clone())
        } else if !w.app_name.is_empty() {
            Some(w.app_name.clone())
        } else {
            None
        }
    }
}

/// The session or guide as seen by one frame.
#[derive(Clone)]
pub struct Snapshot {
    /// Folder the screenshot names are relative to.
    pub dir: PathBuf,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub rows: Vec<StepRow>,
    /// A stopped guide (editable) rather than a live recording.
    pub editable: bool,
}

impl Snapshot {
    /// Copy the session out; `None` when it is locked, so the caller keeps
    /// the previous snapshot rather than blocking the GUI.
    pub fn try_take(session: &Mutex<Session>) -> Option<Self> {
        let g = session.try_lock().ok()?;
        Some(Self {
            dir: g.dir.clone(),
            started_at: g.started_at,
            rows: g.steps.iter().map(StepRow::from_step).collect(),
            editable: false,
        })
    }

    /// Copy a guide out.
    pub fn of_document(doc: &EditorDocument) -> Self {
        let started_at = chrono::DateTime::parse_from_rfc3339(&doc.metadata().created_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or_else(|_| chrono::Local::now());
        Self {
            dir: doc.asset_root().to_path_buf(),
            started_at,
            rows: doc.steps().iter().map(StepRow::from_step).collect(),
            editable: true,
        }
    }

    /// Position of step `id` in the list.
    pub fn position(&self, id: Uuid) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    /// The selected row: `selected` if it still exists, else the last one.
    pub fn current(&self, selected: Option<Uuid>) -> Option<usize> {
        selected
            .and_then(|id| self.position(id))
            .or_else(|| self.rows.len().checked_sub(1))
    }
}

/// A file name from a guide title: letters, digits, spaces and dashes.
pub fn file_stem(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim().trim_matches('-').to_string();
    if s.is_empty() {
        "guide".into()
    } else {
        s.chars().take(80).collect()
    }
}

/// A Unix timestamp (ms) as a readable local date.
pub fn human_date(ms: u128, lang: Lang) -> String {
    let fmt = match lang {
        Lang::Fr => "%d/%m/%Y %H:%M:%S",
        Lang::En => "%Y-%m-%d %H:%M:%S",
    };
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|dt| dt.with_timezone(&chrono::Local).format(fmt).to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// A duration as a clock: "4:07", or "1:02:09" past an hour.
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// An offset from the start of the session: "+12.4 s" ("+12,4 s" in French).
pub fn offset(ms: u128, lang: Lang) -> String {
    let s = format!("+{:.1} s", ms as f64 / 1000.0);
    match lang {
        Lang::Fr => s.replace('.', ","),
        Lang::En => s,
    }
}

/// Typed text shown on one line, with the Enter key made visible.
pub fn shown_text(content: &str) -> String {
    content.replace('\n', " \u{23ce} ")
}

/// Upper-case the first letter ("left" → "Left").
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(247), "4:07");
        assert_eq!(clock(3729), "1:02:09");
    }

    #[test]
    fn file_stems_are_safe() {
        assert_eq!(file_stem("Network setup"), "Network setup");
        assert_eq!(file_stem("a/b\\c:d"), "a-b-c-d");
        assert_eq!(file_stem("../"), "guide");
    }

    #[test]
    fn offset_formats() {
        assert_eq!(offset(12_400, Lang::En), "+12.4 s");
        assert_eq!(offset(12_400, Lang::Fr), "+12,4 s");
    }
}
