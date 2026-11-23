//! Capture d'écran de la fenêtre active et encodage WebP.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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

/// Vrai si `window` appartient à notre propre process (l'IHM du recorder).
pub fn is_own_window(window: &Window) -> bool {
    window.pid().ok() == Some(std::process::id())
}

/// Whether a click at `pos` lands on one of our own windows (the recorder's
/// GUI), which is never a step. Decided by the window under the pointer, not
/// the focused one: Windows only hands the focus over after the click, so a
/// click into the recorder from another application still found that
/// application focused and was recorded.
pub fn click_on_own_window(pos: Option<(i32, i32)>) -> bool {
    pos.is_some_and(|(x, y)| own_window_at(x, y))
}

#[cfg(target_os = "windows")]
fn own_window_at(x: i32, y: i32) -> bool {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetWindowThreadProcessId, WindowFromPoint, GA_ROOT,
    };
    // SAFETY: plain Win32 queries on a window handle the system just gave us;
    // a stale or null handle makes them fail, not misbehave.
    unsafe {
        let hwnd = WindowFromPoint(POINT { x, y });
        if hwnd.is_null() {
            return false;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(if root.is_null() { hwnd } else { root }, &mut pid);
        pid == std::process::id()
    }
}

/// X11 and macOS: xcap lists the windows from the top of the stack down, so
/// the first one containing the point is the one under the pointer. Wayland
/// gives no window positions: nothing matches, the click is kept.
#[cfg(not(target_os = "windows"))]
fn own_window_at(x: i32, y: i32) -> bool {
    let Ok(windows) = Window::all() else {
        return false;
    };
    windows
        .iter()
        .filter(|w| !w.is_minimized().unwrap_or(false))
        .find(|w| {
            let (Ok(wx), Ok(wy), Ok(ww), Ok(wh)) = (w.x(), w.y(), w.width(), w.height()) else {
                return false;
            };
            x >= wx && y >= wy && x < wx + ww as i32 && y < wy + wh as i32
        })
        .is_some_and(is_own_window)
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
    /// est délégué au [`crate::writer::Writer`]. `None` uniquement si le clic vise
    /// notre propre fenêtre : en cas d'échec, une image de remplacement est fournie.
    ///
    /// Seule la partie réellement synchrone au clic est faite ici : le grab de
    /// l'écran (xcap) ou le snapshot de la dernière frame (portail).
    pub fn capture_for_click(
        &self,
        pos: Option<(i32, i32)>,
    ) -> Option<(Pixels, Option<WindowInfo>)> {
        match self {
            Screen::Xcap => grab_robust(pos),
            #[cfg(target_os = "linux")]
            Screen::Portal(p) => {
                // Sous Wayland, `pos` est inconnu (evdev) → pas de marqueur ;
                // l'utilisateur a déjà ciblé la bonne fenêtre au portail. On ne
                // fait que snapshotter la frame brute ; conversion + encodage
                // auront lieu dans le writer. Pas encore de frame → placeholder.
                match p.latest_frame() {
                    Some(frame) => Some((Pixels::Portal(frame), None)),
                    None => Some((Pixels::Rgba(placeholder_image(None, pos)), None)),
                }
            }
        }
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Capture robuste : une capture ne doit JAMAIS bloquer le thread consommateur
// ni faire perdre une étape.
// ---------------------------------------------------------------------------

/// Pourquoi une capture n'a pas produit d'image.
pub enum GrabError {
    /// Le clic/la fenêtre active est le recorder lui-même : pas d'étape.
    OwnWindow,
    /// Échec ou fenêtre inexploitable (minimisée, fermée, taille nulle…).
    Failed,
}

/// Résultat d'une tentative de capture.
pub type Grab = Result<(RgbaImage, Option<WindowInfo>), GrabError>;

/// Délai max du backend natif (WGC / ScreenCaptureKit) : une capture saine
/// prend quelques centaines de ms.
const NATIVE_TIMEOUT: Duration = Duration::from_millis(1200);
/// Délai max du repli `xcap`.
const XCAP_TIMEOUT: Duration = Duration::from_millis(2500);
/// Nombre max de captures abandonnées (bloquées) tolérées en parallèle : borne
/// les threads « fantômes » si un backend ne rend jamais la main.
const MAX_STUCK: usize = 2;

/// Compte les captures encore en cours pour un backend donné.
pub(crate) struct Gate {
    inflight: AtomicUsize,
}

impl Gate {
    pub(crate) const fn new() -> Self {
        Self {
            inflight: AtomicUsize::new(0),
        }
    }
}

static NATIVE_GATE: Gate = Gate::new();
static XCAP_GATE: Gate = Gate::new();

/// Exécute `f` dans un thread dédié, avec délai maximal et capture des paniques.
///
/// Renvoie `None` si `f` a paniqué, dépassé `timeout`, ou si `gate` signale déjà
/// `MAX_STUCK` appels restés bloqués. Un appel qui dépasse le délai est
/// abandonné (le thread finira, ou pas, sans gêner l'appelant).
pub(crate) fn guarded<T: Send + 'static>(
    gate: &'static Gate,
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    if gate.inflight.fetch_add(1, Ordering::SeqCst) >= MAX_STUCK {
        gate.inflight.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("ssr-grab".into())
        .spawn(move || {
            struct Done(&'static Gate);
            impl Drop for Done {
                fn drop(&mut self) {
                    self.0.inflight.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _done = Done(gate);
            let _ = tx.send(catch_unwind(AssertUnwindSafe(f)).ok());
        });
    if spawned.is_err() {
        gate.inflight.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    rx.recv_timeout(timeout).ok().flatten()
}

/// Capture pour un clic via la chaîne de repli complète. `None` seulement si le
/// clic vise notre propre fenêtre ; sinon renvoie toujours quelque chose (au pire
/// un placeholder), pour ne perdre aucune étape.
fn grab_robust(pos: Option<(i32, i32)>) -> Option<(Pixels, Option<WindowInfo>)> {
    // 1) Backend natif (Windows / macOS), isolé : un blocage ne gèle plus le consumer.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    match guarded(&NATIVE_GATE, NATIVE_TIMEOUT, move || {
        capture_for_click_native(pos)
    }) {
        Some(Ok((image, info))) => return Some((Pixels::Rgba(image), info)),
        Some(Err(GrabError::OwnWindow)) => return None,
        Some(Err(GrabError::Failed)) | None => {
            eprintln!("ssr: capture native indisponible — repli xcap");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = &NATIVE_GATE;
    let _ = NATIVE_TIMEOUT;

    // 2) xcap : fenêtre active, sinon écran (recadré sur la fenêtre si connue).
    match guarded(&XCAP_GATE, XCAP_TIMEOUT, move || {
        capture_for_click_xcap(pos)
    }) {
        Some(Ok((image, info))) => Some((Pixels::Rgba(image), info)),
        Some(Err(GrabError::OwnWindow)) => None,
        Some(Err(GrabError::Failed)) | None => {
            // 3) Tout a échoué : l'étape est gardée avec une image de remplacement.
            eprintln!("ssr: capture impossible — image de remplacement");
            Some((Pixels::Rgba(placeholder_image(None, pos)), None))
        }
    }
}

/// Backend natif de la plateforme (Windows / macOS).
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn capture_for_click_native(pos: Option<(i32, i32)>) -> Grab {
    #[cfg(target_os = "windows")]
    return crate::capture_windows::capture_for_click(pos);
    #[cfg(target_os = "macos")]
    return crate::capture_macos::capture_for_click(pos);
}

/// Une fenêtre exploitable : ni minimisée, ni hors écran (Windows range les
/// fenêtres réduites à `-32000`), ni de taille ridicule.
pub fn usable_window(window: &Window) -> bool {
    if window.is_minimized().unwrap_or(false) {
        return false;
    }
    let (x, y) = (window.x().unwrap_or(0), window.y().unwrap_or(0));
    let (w, h) = (window.width().unwrap_or(0), window.height().unwrap_or(0));
    x > -30_000 && y > -30_000 && w >= 8 && h >= 8
}

/// Capture `xcap` : fenêtre active si elle est exploitable ; si la capture de la
/// fenêtre échoue (minimisée entre-temps, fermée…) ou qu'il n'y a pas de fenêtre,
/// on retombe sur l'écran du curseur, recadré sur la fenêtre quand on la connaît.
///
/// Dessine le marqueur de clic si la position du curseur `pos` est connue.
pub fn capture_for_click_xcap(pos: Option<(i32, i32)>) -> Grab {
    let window = focused_window().filter(usable_window);
    if let Some(window) = &window {
        // Ne jamais capturer notre propre fenêtre : un clic sur l'IHM du
        // recorder (Arrêter, panneau de revue…) ne doit pas devenir une étape.
        if is_own_window(window) {
            return Err(GrabError::OwnWindow);
        }
    }
    let info = window.as_ref().map(window_info);
    if let (Some(window), Some(info)) = (&window, &info) {
        if let Some(mut image) = capture_window(window).filter(|i| i.width() > 0 && i.height() > 0)
        {
            if let Some((px, py)) = pos {
                draw_click_marker(&mut image, px - info.x, py - info.y);
            }
            return Ok((image, Some(info.clone())));
        }
    }
    capture_monitor(pos, info)
}

/// Capture l'écran sous le curseur (ou le premier), recadré sur `win` si connue.
fn capture_monitor(pos: Option<(i32, i32)>, win: Option<WindowInfo>) -> Grab {
    let monitor = match pos {
        Some((x, y)) => Monitor::from_point(x, y).ok().or_else(first_monitor),
        None => first_monitor(),
    }
    .ok_or(GrabError::Failed)?;
    let image = monitor.capture_image().map_err(|_| GrabError::Failed)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(GrabError::Failed);
    }
    let origin = (monitor.x().unwrap_or(0), monitor.y().unwrap_or(0));
    // Écrans HiDPI : l'image est en pixels physiques, les coordonnées en logiques.
    let scale = match monitor.width() {
        Ok(w) if w > 0 => image.width() as f32 / w as f32,
        _ => 1.0,
    };
    Ok(compose_fallback(image, origin, scale, win, pos))
}

/// Recadre une capture d'écran sur la fenêtre `win` (si elle y tient) et dessine
/// le marqueur. `origin` = coin de l'écran, `scale` = pixels image / coordonnées.
pub(crate) fn compose_fallback(
    image: RgbaImage,
    origin: (i32, i32),
    scale: f32,
    win: Option<WindowInfo>,
    pos: Option<(i32, i32)>,
) -> (RgbaImage, Option<WindowInfo>) {
    let to_img = |v: i32, o: i32| ((v - o) as f32 * scale).round() as i32;
    let click = pos.map(|(x, y)| (to_img(x, origin.0), to_img(y, origin.1)));
    let (iw, ih) = (image.width() as i32, image.height() as i32);

    if let Some(w) = &win {
        let x0 = to_img(w.x, origin.0).max(0);
        let y0 = to_img(w.y, origin.1).max(0);
        let x1 = (to_img(w.x, origin.0) + (w.width as f32 * scale).round() as i32).min(iw);
        let y1 = (to_img(w.y, origin.1) + (w.height as f32 * scale).round() as i32).min(ih);
        if x1 - x0 >= 8 && y1 - y0 >= 8 {
            let mut cropped = image::imageops::crop_imm(
                &image,
                x0 as u32,
                y0 as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
            )
            .to_image();
            if let Some((cx, cy)) = click {
                draw_click_marker(&mut cropped, cx - x0, cy - y0);
            }
            return (cropped, win);
        }
    }
    let mut image = image;
    if let Some((cx, cy)) = click {
        draw_click_marker(&mut image, cx, cy);
    }
    (image, None)
}

/// Image de remplacement quand aucune capture n'a pu être obtenue : fond gris
/// barré d'une croix, avec le marqueur de clic si la position est connue.
pub fn placeholder_image(win: Option<&WindowInfo>, pos: Option<(i32, i32)>) -> RgbaImage {
    let (w, h) = match win {
        Some(w) if w.width >= 160 && w.height >= 90 => (w.width.min(1920), w.height.min(1080)),
        _ => (960, 540),
    };
    let mut img = RgbaImage::from_pixel(w, h, image::Rgba([58, 62, 70, 255]));
    let line = image::Rgba([120, 126, 138, 255]);
    let (fw, fh) = (w as f32, h as f32);
    for i in 0..w {
        let t = i as f32 / fw;
        for (x, y) in [(i, (t * fh) as u32), (i, ((1.0 - t) * fh) as u32)] {
            for d in 0..3u32 {
                if x < w && y + d < h {
                    img.put_pixel(x, y + d, line);
                }
            }
        }
    }
    for x in 0..w {
        for d in 0..3u32 {
            img.put_pixel(x, d.min(h - 1), line);
            img.put_pixel(x, (h - 1).saturating_sub(d), line);
        }
    }
    if let Some((px, py)) = pos {
        let (ox, oy) = win.map(|w| (w.x, w.y)).unwrap_or((0, 0));
        draw_click_marker(&mut img, px - ox, py - oy);
    }
    img
}

fn first_monitor() -> Option<Monitor> {
    Monitor::all().ok()?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    static T_GATE: Gate = Gate::new();

    #[test]
    fn guarded_returns_value() {
        assert_eq!(guarded(&T_GATE, Duration::from_secs(2), || 7), Some(7));
    }

    #[test]
    fn guarded_survives_panic() {
        static G: Gate = Gate::new();
        assert_eq!(
            guarded(&G, Duration::from_secs(2), || -> u8 { panic!("boom") }),
            None
        );
        // la porte est libérée : un appel suivant réussit.
        assert_eq!(guarded(&G, Duration::from_secs(2), || 1u8), Some(1));
    }

    #[test]
    fn guarded_times_out_and_bounds_stuck_threads() {
        static G: Gate = Gate::new();
        let start = Instant::now();
        for _ in 0..MAX_STUCK {
            let r = guarded(&G, Duration::from_millis(50), || {
                std::thread::sleep(Duration::from_secs(3));
            });
            assert!(r.is_none());
        }
        // Plus de place : refus immédiat, sans lancer de nouveau thread.
        assert!(guarded(&G, Duration::from_secs(2), || 1).is_none());
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "ne doit pas bloquer"
        );
    }

    #[test]
    fn fallback_crops_to_window_with_scale() {
        let screen = RgbaImage::from_pixel(400, 200, image::Rgba([10, 10, 10, 255]));
        let win = WindowInfo {
            x: 50,
            y: 20,
            width: 100,
            height: 60,
            ..Default::default()
        };
        // Écran HiDPI ×2 : l'image fait 400 px pour 200 unités logiques.
        let (img, info) = compose_fallback(screen, (0, 0), 2.0, Some(win), Some((60, 30)));
        assert_eq!((img.width(), img.height()), (200, 120));
        assert!(info.is_some());
    }

    #[test]
    fn fallback_without_usable_window_keeps_whole_screen() {
        let screen = RgbaImage::from_pixel(100, 80, image::Rgba([10, 10, 10, 255]));
        let off = WindowInfo {
            x: -32000,
            y: -32000,
            width: 100,
            height: 50,
            ..Default::default()
        };
        let (img, info) = compose_fallback(screen, (0, 0), 1.0, Some(off), None);
        assert_eq!((img.width(), img.height()), (100, 80));
        assert!(info.is_none());
    }

    #[test]
    fn placeholder_is_never_empty() {
        let img = placeholder_image(None, Some((10, 10)));
        assert_eq!((img.width(), img.height()), (960, 540));
    }
}
