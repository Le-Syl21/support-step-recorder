//! Backend de capture par-fenêtre natif macOS via ScreenCaptureKit.
//!
//! `SCScreenshotManager::capture_image` est l'API one-shot de SCK (pas de flux à
//! monter/démonter), idéale pour un snapshot au clic. On cible la fenêtre au
//! premier plan et on exclut la nôtre.
//!
//! NOTE : écrit sans machine macOS pour tester — validé à la compilation en CI,
//! la correction runtime reste à vérifier sur un Mac.

use image::RgbaImage;
use screencapturekit::screenshot_manager::{CGImageExt, SCScreenshotManager};
use screencapturekit::shareable_content::SCShareableContent;
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;

use crate::capture::draw_click_marker;
use crate::model::WindowInfo;

/// Capture la fenêtre au premier plan pour un clic. `None` si c'est notre propre
/// fenêtre ou si la capture échoue.
pub fn capture_for_click(pos: Option<(i32, i32)>) -> Option<(RgbaImage, Option<WindowInfo>)> {
    let content = SCShareableContent::get().ok()?;
    let own_pid = std::process::id() as i32;

    // `windows()` est ordonné de l'avant vers l'arrière : la première fenêtre
    // normale (layer 0) visible, qui n'est pas la nôtre, approxime le focus.
    let window = content.windows().into_iter().find(|w| {
        w.is_on_screen()
            && w.window_layer() == 0
            && w.owning_application().map(|a| a.process_id()).unwrap_or(0) != own_pid
    })?;

    let frame = window.frame();
    let info = WindowInfo {
        title: window.title().unwrap_or_default(),
        app_name: window
            .owning_application()
            .map(|a| a.application_name())
            .unwrap_or_default(),
        pid: window
            .owning_application()
            .map(|a| a.process_id() as u32)
            .unwrap_or_default(),
        x: frame.origin.x as i32,
        y: frame.origin.y as i32,
        width: frame.size.width as u32,
        height: frame.size.height as u32,
    };

    let filter = SCContentFilter::create().with_window(&window).build();
    let config = SCStreamConfiguration::new()
        .with_width(frame.size.width as u32)
        .with_height(frame.size.height as u32);

    let cg = SCScreenshotManager::capture_image(&filter, &config).ok()?;
    let (w, h) = (cg.width() as u32, cg.height() as u32);
    let rgba = cg.rgba_data().ok()?;

    let mut image = RgbaImage::from_raw(w, h, rgba)?;
    if let Some((px, py)) = pos {
        draw_click_marker(&mut image, px - info.x, py - info.y);
    }
    Some((image, Some(info)))
}
