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

use crate::capture::draw_click_marker;
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

/// Capture la fenêtre au premier plan pour un clic. `None` si c'est notre propre
/// fenêtre (l'IHM du recorder) ou si la capture échoue.
pub fn capture_for_click(pos: Option<(i32, i32)>) -> Option<(RgbaImage, Option<WindowInfo>)> {
    let window = Window::foreground().ok()?;

    // Ne jamais capturer notre propre fenêtre (Stop, panneau de revue…).
    let pid = window.process_id().ok()?;
    if pid == std::process::id() {
        return None;
    }

    let rect = window.rect().ok()?;
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
    OneShot::start(settings).ok()?;

    let mut image = slot.lock().ok()?.take()?;
    if let Some((px, py)) = pos {
        draw_click_marker(&mut image, px - info.x, py - info.y);
    }
    Some((image, Some(info)))
}
