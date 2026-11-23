//! Backend de capture par-fenêtre natif Windows via Windows.Graphics.Capture.
//!
//! `windows-capture` gère sa propre boucle WinRT + pompe de messages en interne
//! (`start` est bloquant et se termine quand `capture_control.stop()` est appelé),
//! ce qui évite le blocage du thread consumer qu'on avait avec la capture xcap.

use std::sync::{Arc, Mutex};

use image::RgbaImage;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use crate::capture::{draw_click_marker, Grab, GrabError};
use crate::model::WindowInfo;

type Slot = Arc<Mutex<Option<RgbaImage>>>;

/// Handler qui capture UNE frame RGBA puis arrête la session.
struct OneShot {
    slot: Slot,
}

impl GraphicsCaptureApiHandler for OneShot {
    type Flags = Slot;
    type Error = std::convert::Infallible;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { slot: ctx.flags })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if let Ok(fb) = frame.buffer() {
            let (w, h) = (fb.width(), fb.height());
            let mut scratch = Vec::new();
            let rgba = fb.as_nopadding_buffer(&mut scratch);
            if let Some(img) = RgbaImage::from_raw(w, h, rgba.to_vec()) {
                if let Ok(mut slot) = self.slot.lock() {
                    *slot = Some(img);
                }
            }
        }
        capture_control.stop();
        Ok(())
    }
}

/// Capture la fenêtre au premier plan pour un clic. `OwnWindow` si c'est notre
/// propre fenêtre (l'IHM du recorder), `Failed` si la capture échoue. Appelée
/// depuis un thread surveillé (timeout) : `start` peut ne jamais rendre la main.
pub fn capture_for_click(pos: Option<(i32, i32)>) -> Grab {
    let window = Window::foreground().map_err(|_| GrabError::Failed)?;

    // Ne jamais capturer notre propre fenêtre (Stop, panneau de revue…).
    let pid = window.process_id().map_err(|_| GrabError::Failed)?;
    if pid == std::process::id() {
        return Err(GrabError::OwnWindow);
    }

    let rect = window.rect().map_err(|_| GrabError::Failed)?;
    // Fenêtre réduite (Windows la range à -32000) ou vide : WGC ne livrerait
    // jamais de frame. On laisse le repli capturer l'écran.
    if rect.left <= -30_000
        || rect.top <= -30_000
        || rect.right - rect.left < 8
        || rect.bottom - rect.top < 8
    {
        return Err(GrabError::Failed);
    }
    let info = WindowInfo {
        title: window.title().unwrap_or_default(),
        app_name: window.process_name().unwrap_or_default(),
        pid,
        x: rect.left,
        y: rect.top,
        width: (rect.right - rect.left).max(0) as u32,
        height: (rect.bottom - rect.top).max(0) as u32,
    };

    let slot: Slot = Arc::new(Mutex::new(None));
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        slot.clone(),
    );

    // Bloque jusqu'à ce que `on_frame_arrived` appelle `stop()` (une frame).
    OneShot::start(settings).map_err(|_| GrabError::Failed)?;

    let mut image = slot
        .lock()
        .ok()
        .and_then(|mut s| s.take())
        .ok_or(GrabError::Failed)?;
    if let Some((px, py)) = pos {
        draw_click_marker(&mut image, px - info.x, py - info.y);
    }
    Ok((image, Some(info)))
}
