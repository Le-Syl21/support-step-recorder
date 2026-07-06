//! Capture d'écran de la fenêtre active et encodage WebP.

use image::RgbaImage;
use webp::{Encoder, WebPConfig};
use xcap::{Monitor, Window};

use crate::model::WindowInfo;
use crate::writer::Pixels;

/// Renvoie la fenêtre ayant actuellement le focus, si elle est identifiable.
pub fn focused_window() -> Option<Window> {
    Window::all()
        .ok()?
        .into_iter()
        .find(|w| w.is_focused().unwrap_or(false))
}

/// Extrait les métadonnées d'une fenêtre (au mieux ; champs vides sinon).
pub fn window_info(window: &Window) -> WindowInfo {
    WindowInfo {
        title: window.title().unwrap_or_default(),
        app_name: window.app_name().unwrap_or_default(),
        pid: window.pid().unwrap_or_default(),
        x: window.x().unwrap_or_default(),
        y: window.y().unwrap_or_default(),
        width: window.width().unwrap_or_default(),
        height: window.height().unwrap_or_default(),
    }
}

/// Capture l'image de la fenêtre.
pub fn capture_window(window: &Window) -> Option<RgbaImage> {
    window.capture_image().ok()
}

/// Encode une image RGBA en WebP avec perte à la qualité donnée (0..=100).
///
/// On force un `method` bas (encodeur rapide) : pour un enregistreur d'actions,
/// garder du CPU disponible prime sur la compacité du fichier.
pub fn encode_webp(image: &RgbaImage, quality: f32) -> Vec<u8> {
    let encoder = Encoder::from_rgba(image.as_raw(), image.width(), image.height());
    let Ok(mut config) = WebPConfig::new() else {
        return encoder.encode(quality).to_vec(); // repli si config indisponible
    };
    config.lossless = 0;
    config.alpha_compression = 1;
    config.quality = quality;
    // 0 = le plus rapide (CPU mini) … 6 = le plus lent (fichier plus petit).
    config.method = 1;
    match encoder.encode_advanced(&config) {
        Ok(mem) => mem.to_vec(),
        Err(_) => encoder.encode(quality).to_vec(),
    }
}

/// Dessine un marqueur de clic (anneau rouge) centré sur `(cx, cy)`.
///
/// `cx`/`cy` sont exprimés en pixels relatifs à l'image (coin haut-gauche).
pub fn draw_click_marker(image: &mut RgbaImage, cx: i32, cy: i32) {
    const RADIUS: i32 = 16;
    const THICKNESS: i32 = 3;
    let red = image::Rgba([235, 30, 30, 255]);

    let (w, h) = (image.width() as i32, image.height() as i32);
    let inner = (RADIUS - THICKNESS).pow(2);
    let outer = RADIUS.pow(2);

    for dy in -RADIUS..=RADIUS {
        for dx in -RADIUS..=RADIUS {
            let dist = dx * dx + dy * dy;
            if dist <= outer && dist >= inner {
                let (px, py) = (cx + dx, cy + dy);
                if px >= 0 && py >= 0 && px < w && py < h {
                    image.put_pixel(px as u32, py as u32, red);
                }
            }
        }
    }
}

/// Source de capture d'écran, choisie selon l'environnement et gardée vivante
/// pour toute la durée d'un enregistrement.
///
/// - Wayland → [`screencast::PortalCapture`] (portail + PipeWire, capture d'une
///   fenêtre au choix de l'utilisateur).
/// - X11/Windows/macOS → `xcap` (fenêtre active, sans état).
pub enum Screen {
    Xcap,
    #[cfg(target_os = "linux")]
    Portal(crate::screencast::PortalCapture),
}

impl Screen {
    /// Prépare la source adaptée. Sous Wayland, ouvre le dialogue de
    /// consentement du portail ; en cas d'échec, repli sur `xcap`.
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            if crate::input::is_wayland() {
                match crate::screencast::PortalCapture::start() {
                    Ok(p) => return Screen::Portal(p),
                    Err(e) => eprintln!("ssr: portail indisponible ({e}) — repli xcap"),
                }
            }
        }
        Screen::Xcap
    }

    /// Capture adaptée à un clic. Renvoie **les pixels bruts** (non encodés) et
    /// l'info fenêtre éventuelle — le travail lourd (conversion/encodage/écriture)
    /// est délégué au [`crate::writer::Writer`]. `None` si rien à capturer.
    ///
    /// Seule la partie réellement synchrone au clic est faite ici : le grab de
    /// l'écran (xcap) ou le snapshot de la dernière frame (portail).
    pub fn capture_for_click(
        &self,
        pos: Option<(i32, i32)>,
    ) -> Option<(Pixels, Option<WindowInfo>)> {
        match self {
            Screen::Xcap => {
                let (image, info) = capture_for_click_xcap(pos)?;
                Some((Pixels::Rgba(image), info))
            }
            #[cfg(target_os = "linux")]
            Screen::Portal(p) => {
                // Sous Wayland, `pos` est inconnu (evdev) → pas de marqueur ;
                // l'utilisateur a déjà ciblé la bonne fenêtre au portail. On ne
                // fait que snapshotter la frame brute ; conversion + encodage
                // auront lieu dans le writer.
                let frame = p.latest_frame()?;
                Some((Pixels::Portal(frame), None))
            }
        }
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

/// Capture `xcap` : fenêtre active si on peut l'identifier (X11/Windows/macOS),
/// sinon plein écran en repli. Le grab de l'écran est la seule partie qui doit
/// être synchrone au clic ; l'encodage est délégué au writer.
///
/// Dessine le marqueur de clic si la position du curseur `pos` est connue.
/// Renvoie l'image RGBA brute et l'info fenêtre éventuelle.
pub fn capture_for_click_xcap(pos: Option<(i32, i32)>) -> Option<(RgbaImage, Option<WindowInfo>)> {
    // Cas nominal : capture de la fenêtre active.
    if let Some(window) = focused_window() {
        let info = window_info(&window);
        let mut image = capture_window(&window)?;
        if let Some((px, py)) = pos {
            draw_click_marker(&mut image, px - info.x, py - info.y);
        }
        return Some((image, Some(info)));
    }

    // Repli plein écran (Wayland, ou aucune fenêtre focus détectée).
    let monitor = match pos {
        Some((x, y)) => Monitor::from_point(x, y).ok().or_else(first_monitor)?,
        None => first_monitor()?,
    };
    let mut image = monitor.capture_image().ok()?;
    if let (Some((px, py)), Ok(mx), Ok(my)) = (pos, monitor.x(), monitor.y()) {
        draw_click_marker(&mut image, px - mx, py - my);
    }
    Some((image, None))
}

fn first_monitor() -> Option<Monitor> {
    Monitor::all().ok()?.into_iter().next()
}
