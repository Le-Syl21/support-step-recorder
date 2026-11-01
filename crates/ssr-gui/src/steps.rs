//! What the GUI shows of a session: a lock-free copy of its steps, rebuilt
//! each frame, and the formatting of their metadata.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ssr_core::{session::Session, Action, Lang, WindowInfo};

/// One step copied out of the session for display (no lock held).
///
/// The raw action and window are kept (not the already translated text) so
/// the description can be rebuilt in the current language.
#[derive(Clone)]
pub struct StepRow {
    pub index: usize,
    pub screenshot: Option<String>,
    /// Absolute capture time (ms since the Unix epoch).
    pub timestamp_ms: u128,
    /// Time since the start of the session (ms).
    pub elapsed_ms: u128,
    pub action: Action,
    pub window: Option<WindowInfo>,
}

impl StepRow {
    /// Readable description in the given language.
    ///
    /// An untitled window (common on X11 for small tools) is named after its
    /// application rather than shown as empty quotes.
    pub fn description(&self, lang: Lang) -> String {
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
        ssr_core::Step::build_description(&self.action, window.as_ref(), lang)
    }

    pub fn is_click(&self) -> bool {
        matches!(self.action, Action::Click { .. })
    }

    /// A short title for the list: "Left click" / "Typing".
    pub fn short_title(&self, lang: Lang) -> String {
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

/// The session as seen by one frame.
#[derive(Clone)]
pub struct Snapshot {
    pub dir: PathBuf,
    pub started_at: chrono::DateTime<chrono::Local>,
    pub rows: Vec<StepRow>,
}

impl Snapshot {
    /// Copy the session out; `None` when it is locked (export in progress), so
    /// the caller keeps the previous snapshot rather than blocking the GUI.
    pub fn try_take(session: &Mutex<Session>) -> Option<Self> {
        let g = session.try_lock().ok()?;
        let rows = g
            .steps
            .iter()
            .map(|st| StepRow {
                index: st.index,
                screenshot: st.screenshot.clone(),
                timestamp_ms: st.timestamp_ms,
                elapsed_ms: st.elapsed_ms,
                action: st.action.clone(),
                window: st.window.clone(),
            })
            .collect();
        Some(Self {
            dir: g.dir.clone(),
            started_at: g.started_at,
            rows,
        })
    }

    /// Position of the step `index` in the list.
    pub fn position(&self, index: usize) -> Option<usize> {
        self.rows.iter().position(|r| r.index == index)
    }
}

/// Default ZIP file name, derived from the session folder.
pub fn default_zip_name(session: &Arc<Mutex<Session>>) -> String {
    session
        .lock()
        .ok()
        .and_then(|s| s.dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|n| format!("{n}.zip"))
        .unwrap_or_else(|| "session.zip".to_string())
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
    fn offset_formats() {
        assert_eq!(offset(12_400, Lang::En), "+12.4 s");
        assert_eq!(offset(12_400, Lang::Fr), "+12,4 s");
    }
}
