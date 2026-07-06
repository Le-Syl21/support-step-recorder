//! Boucle de capture : sélectionne un backend d'entrées, consomme ses
//! événements dans un thread dédié et alimente la session.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::capture::Screen;
use crate::i18n::Lang;
use crate::input::{self, Button, InputEvent, InputHandle, Keycode};
use crate::progress::Progress;
use crate::session::Session;
use crate::text;
use crate::writer::{Job, Writer};

/// Poignée d'enregistrement : tant qu'elle vit, les événements sont capturés.
pub struct Recorder {
    /// Session partagée avec l'IHM (lecture seule côté IHM).
    pub session: Arc<Mutex<Session>>,
    backend_name: &'static str,
    // `None` après stop()/drop : sa destruction arrête le backend et ferme le canal.
    input: Option<InputHandle>,
    consumer: Option<JoinHandle<()>>,
    // File d'écriture asynchrone. Détruite APRÈS le consumer (cf. `shutdown`)
    // pour drainer les captures encore en attente d'encodage.
    writer: Option<Writer>,
}

impl Recorder {
    /// Démarre la capture dans le dossier `dir` (backend choisi automatiquement).
    ///
    /// `progress` reçoit l'action longue en cours (attente de capture, sauvegarde
    /// de l'image) pour affichage par l'IHM.
    pub fn start(dir: PathBuf, progress: Progress, lang: Lang) -> std::io::Result<Self> {
        let session = Arc::new(Mutex::new(Session::new(dir, lang)?));
        let backend = input::select();
        let backend_name = backend.name();

        // Source de capture d'écran (sous Wayland : dialogue de consentement du
        // portail à cet instant).
        let screen = Screen::new();

        // File d'écriture asynchrone : encodage WebP + sauvegarde hors du chemin
        // du clic.
        let writer = Writer::new(progress.clone(), lang);
        let jobs = writer.sender();

        let (tx, rx) = channel::<InputEvent>();
        let input = backend.start(tx)?;

        let consumer = {
            let session = session.clone();
            std::thread::Builder::new()
                .name("ssr-consumer".into())
                .spawn(move || consume(rx, session, progress, screen, jobs, lang))
                .map_err(std::io::Error::other)?
        };

        Ok(Self {
            session,
            backend_name,
            input: Some(input),
            consumer: Some(consumer),
            writer: Some(writer),
        })
    }

    /// Nom du backend d'entrées actif (diagnostic).
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    /// Arrête la capture et renvoie la session figée.
    pub fn stop(mut self) -> Arc<Mutex<Session>> {
        // Marque l'arrêt AVANT de drainer les événements restants : ainsi le clic
        // sur « Arrêter » (encore en vol dans le canal) n'est pas capturé.
        if let Ok(mut s) = self.session.lock() {
            s.recording = false;
        }
        self.shutdown();
        if let Ok(mut s) = self.session.lock() {
            s.stop();
        }
        self.session.clone()
    }

    fn shutdown(&mut self) {
        // Détruire la poignée ferme le canal -> le consumer sort de sa boucle.
        self.input = None;
        if let Some(c) = self.consumer.take() {
            let _ = c.join();
        }
        // Le consumer est arrêté : plus aucun job ne sera émis. On détruit le
        // writer, ce qui draine les captures restantes puis joint son thread.
        self.writer = None;
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Boucle du thread consommateur : applique chaque événement à la session.
fn consume(
    rx: Receiver<InputEvent>,
    session: Arc<Mutex<Session>>,
    progress: Progress,
    screen: Screen,
    jobs: Sender<Job>,
    lang: Lang,
) {
    while let Ok(event) = rx.recv() {
        match event {
            InputEvent::MouseUp { button, pos } => {
                on_click(&session, button, pos, &progress, &screen, &jobs, lang)
            }
            InputEvent::KeyDown(key) => on_key_down(&session, key),
            InputEvent::KeyUp(key) => on_key_up(&session, key),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_click(
    session: &Arc<Mutex<Session>>,
    button: Button,
    pos: Option<(i32, i32)>,
    progress: &Progress,
    screen: &Screen,
    jobs: &Sender<Job>,
    lang: Lang,
) {
    // Court-circuit si l'enregistrement est déjà terminé : évite une capture
    // inutile (notamment le clic sur « Arrêter »).
    if !session.lock().map(|s| s.recording).unwrap_or(false) {
        return;
    }

    // Seule partie synchrone au clic : le snapshot de l'écran (grab xcap ou
    // dernière frame du portail). On capture l'état au moment du clic (avant que
    // l'action ne prenne effet), comme un PSR. L'encodage est délégué au writer.
    progress.set(lang.msg_capturing());
    let capture = screen.capture_for_click(pos);
    progress.clear();

    let Some((pixels, info)) = capture else {
        return;
    };

    // Enregistre l'étape (avec le nom de fichier réservé) et met la capture en
    // file d'écriture. La session reste à jour dans l'ordre ; le fichier .webp
    // apparaîtra un court instant plus tard.
    let mut s = match session.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if !s.recording {
        return;
    }
    let name = s.next_screenshot_name();
    let path = s.dir.join(&name);
    s.record_click(button, pos, info, Some(name));
    drop(s);

    let _ = jobs.send(Job {
        pixels,
        path,
        quality: 55.0,
    });
}

fn on_key_down(session: &Arc<Mutex<Session>>, key: Keycode) {
    let mut s = match session.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if !s.recording {
        return;
    }
    if text::is_shift(&key) {
        s.shift_down = true;
        return;
    }
    if let Some(token) = text::key_to_token(&key, s.shift_down) {
        s.apply_key(token);
    }
}

fn on_key_up(session: &Arc<Mutex<Session>>, key: Keycode) {
    if !text::is_shift(&key) {
        return;
    }
    if let Ok(mut s) = session.lock() {
        s.shift_down = false;
    }
}
