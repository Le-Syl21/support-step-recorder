//! Capture d'écran sous Wayland via **xdg-desktop-portal** (`ScreenCast`) +
//! **PipeWire**.
//!
//! Au démarrage, le portail ouvre un **dialogue de consentement** géré par le
//! compositeur : l'utilisateur y choisit ce qu'il partage (un écran ou **une
//! fenêtre précise**). On récupère alors un nœud PipeWire dont on tire les
//! frames à la demande. Avec `PersistMode::Application` + un `restore_token`
//! sauvegardé, les lancements suivants ne redemandent plus le consentement.
//!
//! C'est le chemin de capture Wayland ; X11/Windows/macOS passent par `xcap`
//! (cf. [`crate::capture`]).

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
use ashpd::desktop::{PersistMode, Session};

use pipewire as pw;
use pw::{properties::properties, spa};
use spa::pod::Pod;

/// Une frame **brute** (telle que livrée par PipeWire) + les infos nécessaires
/// pour la convertir en RGBA. La conversion (coûteuse) est différée au moment du
/// clic (cf. [`Frame::to_rgba`]) plutôt que faite sur chaque frame du flux, pour
/// garder du CPU disponible pendant l'enregistrement.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    stride: usize,
    format: spa::param::video::VideoFormat,
    bytes: Vec<u8>,
}

impl Frame {
    /// Convertit la frame en RGBA contigu (`None` si format non géré).
    pub fn to_rgba(&self) -> Option<Vec<u8>> {
        to_rgba(
            &self.bytes,
            self.width,
            self.height,
            self.stride,
            self.format,
        )
    }

    /// Convertit en RGBA **recadré sur la zone non-noire**.
    ///
    /// Mutter livre les captures de fenêtre dans un buffer à la taille de l'écran,
    /// avec du noir autour ; on rogne ce noir pour ne garder que le contenu.
    /// Renvoie `(rgba, largeur, hauteur)` de la zone recadrée. Si la frame est
    /// entièrement noire, on renvoie l'image entière (recadrer donnerait du vide).
    pub fn to_rgba_cropped(&self) -> Option<(Vec<u8>, u32, u32)> {
        let (swap_rb, opaque) = rb_opaque(self.format)?;
        let (w, h) = (self.width as usize, self.height as usize);
        if self.stride < w * 4 || self.bytes.len() < self.stride * h {
            return None;
        }

        // Boîte englobante du contenu non-noir (seuil pour absorber le bruit).
        const THRESHOLD: u8 = 8;
        let is_content = |p: usize| {
            self.bytes[p] >= THRESHOLD
                || self.bytes[p + 1] >= THRESHOLD
                || self.bytes[p + 2] >= THRESHOLD
        };
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        let mut found = false;
        for y in 0..h {
            let row = y * self.stride;
            for x in 0..w {
                if is_content(row + x * 4) {
                    found = true;
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        if !found {
            let rgba = self.to_rgba()?;
            return Some((rgba, self.width, self.height));
        }

        let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut out = vec![0u8; cw * ch * 4];
        for y in 0..ch {
            let srow = (y0 + y) * self.stride + x0 * 4;
            let drow = y * cw * 4;
            for x in 0..cw {
                let s = &self.bytes[srow + x * 4..srow + x * 4 + 4];
                let d = &mut out[drow + x * 4..drow + x * 4 + 4];
                if swap_rb {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                } else {
                    d[0] = s[0];
                    d[1] = s[1];
                    d[2] = s[2];
                }
                d[3] = if opaque { 255 } else { s[3] };
            }
        }
        Some((out, cw as u32, ch as u32))
    }
}

/// `(swap_rb, alpha_opaque)` selon le format PipeWire (tous en 4 octets/pixel).
fn rb_opaque(format: spa::param::video::VideoFormat) -> Option<(bool, bool)> {
    use spa::param::video::VideoFormat as F;
    match format {
        F::RGBA => Some((false, false)),
        F::RGBx => Some((false, true)),
        F::BGRA => Some((true, false)),
        F::BGRx => Some((true, true)),
        _ => None,
    }
}

/// Capture d'écran active via le portail : détient le thread PipeWire et
/// expose la dernière frame reçue. À la destruction, arrête le flux et ferme
/// la session du portail.
pub struct PortalCapture {
    latest: Arc<Mutex<Option<Frame>>>,
    stop: Option<pw::channel::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl PortalCapture {
    /// Négocie le portail (dialogue de consentement) et démarre le flux
    /// PipeWire dans un thread dédié.
    pub fn start() -> io::Result<Self> {
        let latest = Arc::new(Mutex::new(None));
        let (tx, rx) = pw::channel::channel::<()>();

        let latest_thread = latest.clone();
        let thread = std::thread::Builder::new()
            .name("ssr-screencast".into())
            .spawn(move || {
                if let Err(e) = run(latest_thread, rx) {
                    eprintln!("ssr: capture portail arrêtée : {e}");
                }
            })
            .map_err(io::Error::other)?;

        Ok(Self {
            latest,
            stop: Some(tx),
            thread: Some(thread),
        })
    }

    /// Dernière frame livrée par le flux, s'il y en a une.
    pub fn latest_frame(&self) -> Option<Frame> {
        self.latest.lock().ok().and_then(|g| g.clone())
    }
}

impl Drop for PortalCapture {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(()); // demande au mainloop PipeWire de quitter
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Données passées aux callbacks PipeWire.
struct UserData {
    format: spa::param::video::VideoInfoRaw,
    latest: Arc<Mutex<Option<Frame>>>,
    /// Dernière frame stockée (throttle : on ne garde qu'une frame ~toutes les
    /// 100 ms, largement assez pour une capture au clic).
    last_store: Option<Instant>,
}

/// Corps du thread de capture : portail (async) puis boucle PipeWire (bloquante).
fn run(latest: Arc<Mutex<Option<Frame>>>, stop_rx: pw::channel::Receiver<()>) -> io::Result<()> {
    // 1) Portail ScreenCast. ashpd (feature async-io) pilote l'IO zbus depuis un
    //    réacteur global : on peut donc bloquer ici avec pollster, et la session
    //    reste vivante pendant tout le flux sans runtime à garder actif.
    let portal = pollster::block_on(negotiate())
        .map_err(|e| io::Error::other(format!("portail ScreenCast : {e}")))?;

    // 2) Flux PipeWire sur le nœud fourni par le portail.
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(io::Error::other)?;
    let context = pw::context::ContextRc::new(&mainloop, None).map_err(io::Error::other)?;
    let core = context
        .connect_fd_rc(portal.fd, None)
        .map_err(io::Error::other)?;

    let stream = pw::stream::StreamBox::new(
        &core,
        "ssr-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(io::Error::other)?;

    let user_data = UserData {
        format: Default::default(),
        latest: latest.clone(),
        last_store: None,
    };

    let _listener = stream
        .add_local_listener_with_user_data(user_data)
        .param_changed(|_, ud, id, param| {
            let Some(param) = param else { return };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Ok((mt, ms)) = spa::param::format_utils::parse_format(param) else {
                return;
            };
            if mt != spa::param::format::MediaType::Video
                || ms != spa::param::format::MediaSubtype::Raw
            {
                return;
            }
            let _ = ud.format.parse(param);
        })
        .process(|stream, ud| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };
            // Throttle : on ne conserve qu'une frame toutes les ~200 ms — la
            // dernière frame reflète l'état juste avant le clic ; ça suffit et
            // épargne le memcpy.
            if let Some(t) = ud.last_store {
                if t.elapsed() < Duration::from_millis(200) {
                    return;
                }
            }
            let width = ud.format.size().width;
            let height = ud.format.size().height;
            let stride = data.chunk().stride().max(0) as usize;
            let format = ud.format.format();
            let Some(bytes) = data.data() else { return };
            if width == 0 || height == 0 || stride == 0 {
                return;
            }
            let needed = stride * height as usize;
            if bytes.len() < needed {
                return;
            }
            // On copie la frame brute ; la conversion RGBA est faite plus tard,
            // seulement quand un clic déclenche une capture.
            if let Ok(mut slot) = ud.latest.lock() {
                *slot = Some(Frame {
                    width,
                    height,
                    stride,
                    format,
                    bytes: bytes[..needed].to_vec(),
                });
            }
            ud.last_store = Some(Instant::now());
        })
        .register()
        .map_err(io::Error::other)?;

    // Format demandé : RGBA/RGBx/BGRA/BGRx (GNOME livre en général du BGRx).
    let obj = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBx,
            spa::param::video::VideoFormat::BGRA,
            spa::param::video::VideoFormat::BGRx,
        ),
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle {
                width: 1920,
                height: 1080
            },
            spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            spa::utils::Rectangle {
                width: 8192,
                height: 8192
            }
        ),
        // Framerate volontairement bas : une capture au clic n'a pas besoin de
        // fluidité, et un flux lent réduit le CPU (côté compositeur et ici).
        spa::pod::property!(
            spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            spa::utils::Fraction { num: 5, denom: 1 },
            spa::utils::Fraction { num: 0, denom: 1 },
            spa::utils::Fraction { num: 10, denom: 1 }
        ),
    );
    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .map_err(io::Error::other)?
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).ok_or_else(|| io::Error::other("POD invalide"))?];

    stream
        .connect(
            spa::utils::Direction::Input,
            Some(portal.node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(io::Error::other)?;

    // Un message sur `stop_rx` fait quitter la boucle (déclenché par Drop).
    let ml = mainloop.clone();
    let _stop = stop_rx.attach(mainloop.loop_(), move |_| ml.quit());

    mainloop.run();

    // Sortie de boucle : referme proprement la session du portail.
    pollster::block_on(async {
        let _ = portal.session.close().await;
    });
    Ok(())
}

/// Résultat de la négociation du portail.
struct Portal {
    fd: std::os::fd::OwnedFd,
    node_id: u32,
    /// Session gardée vivante : sa fermeture couperait le flux PipeWire.
    session: Session<Screencast>,
}

/// Dialogue de consentement + ouverture du flux PipeWire.
async fn negotiate() -> ashpd::Result<Portal> {
    let proxy = Screencast::new().await?;
    let session = proxy.create_session(Default::default()).await?;

    let token = load_restore_token();
    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Embedded)
                .set_sources(SourceType::Monitor | SourceType::Window)
                .set_multiple(false)
                .set_persist_mode(PersistMode::Application)
                .set_restore_token(token.as_deref()),
        )
        .await?;

    let response = proxy
        .start(&session, None, Default::default())
        .await?
        .response()?;

    if let Some(tok) = response.restore_token() {
        save_restore_token(tok);
    }

    let stream = response
        .streams()
        .first()
        .ok_or_else(|| ashpd::Error::NoResponse)?;
    let node_id = stream.pipe_wire_node_id();
    let fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await?;

    Ok(Portal {
        fd,
        node_id,
        session,
    })
}

/// Convertit un tampon brut (une frame) vers du RGBA contigu, en respectant le
/// `stride` et l'ordre des composantes du format PipeWire.
fn to_rgba(
    src: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    format: spa::param::video::VideoFormat,
) -> Option<Vec<u8>> {
    let (swap_rb, opaque) = rb_opaque(format)?;
    let (w, h) = (width as usize, height as usize);
    if stride < w * 4 || src.len() < stride * h {
        return None;
    }
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let row = &src[y * stride..y * stride + w * 4];
        let dst = &mut out[y * w * 4..(y + 1) * w * 4];
        for x in 0..w {
            let s = &row[x * 4..x * 4 + 4];
            let d = &mut dst[x * 4..x * 4 + 4];
            if swap_rb {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
            } else {
                d[0] = s[0];
                d[1] = s[1];
                d[2] = s[2];
            }
            d[3] = if opaque { 255 } else { s[3] };
        }
    }
    Some(out)
}

/// Emplacement du jeton de restauration (évite de redemander le consentement).
fn restore_token_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("ssr").join("screencast.token"))
}

fn load_restore_token() -> Option<String> {
    let p = restore_token_path()?;
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
}

fn save_restore_token(token: &str) {
    if let Some(p) = restore_token_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spa::param::video::VideoFormat;

    /// Construit une frame BGRx `w×h` avec un bloc de contenu (blanc) dans le
    /// rectangle `[x0,x1)×[y0,y1)`, le reste noir. `stride` = `w*4` + padding.
    fn frame_with_block(
        w: usize,
        h: usize,
        pad: usize,
        (x0, y0, x1, y1): (usize, usize, usize, usize),
    ) -> Frame {
        let stride = w * 4 + pad;
        let mut bytes = vec![0u8; stride * h];
        for y in y0..y1 {
            for x in x0..x1 {
                let p = y * stride + x * 4;
                bytes[p] = 200; // B
                bytes[p + 1] = 200; // G
                bytes[p + 2] = 200; // R
                bytes[p + 3] = 255; // x
            }
        }
        Frame {
            width: w as u32,
            height: h as u32,
            stride,
            format: VideoFormat::BGRx,
            bytes,
        }
    }

    #[test]
    fn crop_reduces_to_content_bbox() {
        // Contenu 2×2 à partir de (1,1) dans une frame 4×4 (le reste noir).
        let frame = frame_with_block(4, 4, 0, (1, 1, 3, 3));
        let (rgba, cw, ch) = frame.to_rgba_cropped().expect("recadrage");
        assert_eq!((cw, ch), (2, 2), "doit se réduire au bloc de contenu");
        assert_eq!(rgba.len(), 2 * 2 * 4);
        // BGRx(200,200,200) -> RGBA(200,200,200,255).
        assert_eq!(&rgba[0..4], &[200, 200, 200, 255]);
    }

    #[test]
    fn crop_handles_row_padding() {
        // Même bloc, mais avec du padding de stride : le recadrage doit rester
        // correct malgré des lignes plus larges que w*4.
        let frame = frame_with_block(4, 4, 12, (2, 0, 4, 1));
        let (_, cw, ch) = frame.to_rgba_cropped().expect("recadrage");
        assert_eq!((cw, ch), (2, 1));
    }

    #[test]
    fn all_black_frame_is_kept_whole() {
        let frame = frame_with_block(3, 2, 0, (0, 0, 0, 0)); // aucun contenu
        let (_, cw, ch) = frame.to_rgba_cropped().expect("frame entière");
        assert_eq!((cw, ch), (3, 2), "frame noire → conservée entière");
    }
}
