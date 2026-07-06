//! Écriture asynchrone des captures.
//!
//! Au clic, on ne fait que le strict nécessaire (snapshot de l'image brute +
//! métadonnées), poussé dans une file. Un thread dédié se charge du travail
//! coûteux — conversion en RGBA, encodage WebP, écriture disque — sans jamais
//! bloquer la capture d'entrées ni l'IHM.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

use image::RgbaImage;

use crate::capture::encode_webp;
use crate::progress::Progress;

/// Pixels d'une capture, prêts (ou presque) à encoder.
pub enum Pixels {
    /// Déjà en RGBA (chemin `xcap`).
    Rgba(RgbaImage),
    /// Frame brute du portail (BGRx…), convertie côté writer (chemin Wayland).
    #[cfg(target_os = "linux")]
    Portal(crate::screencast::Frame),
}

/// Un travail d'écriture : une capture à encoder et déposer sur disque.
pub struct Job {
    pub pixels: Pixels,
    pub path: PathBuf,
    pub quality: f32,
}

/// File d'écriture asynchrone. Tant qu'elle vit, un thread consomme les travaux ;
/// à la destruction, la file est fermée puis vidée avant de rendre la main.
pub struct Writer {
    tx: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Démarre le thread d'écriture. `progress` affiche l'activité en cours.
    pub fn new(progress: Progress, lang: crate::i18n::Lang) -> Self {
        let (tx, rx) = channel::<Job>();
        let thread = std::thread::Builder::new()
            .name("ssr-writer".into())
            .spawn(move || run(rx, progress, lang))
            .ok();
        Self {
            tx: Some(tx),
            thread,
        }
    }

    /// Émetteur pour déposer des travaux depuis le thread de capture.
    pub fn sender(&self) -> Sender<Job> {
        self.tx.clone().expect("writer actif")
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // Fermer le canal fait sortir le thread une fois la file vidée.
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Boucle du thread : encode et écrit chaque capture, dans l'ordre d'arrivée.
fn run(rx: Receiver<Job>, progress: Progress, lang: crate::i18n::Lang) {
    while let Ok(job) = rx.recv() {
        progress.set(lang.msg_saving_image());
        if let Err(err) = write_job(job) {
            eprintln!("ssr: écriture d'une capture échouée : {err}");
        }
        // On efface seulement si plus rien n'attend, pour ne pas clignoter.
        // (best-effort : une nouvelle capture réaffichera le message).
        progress.clear();
    }
}

fn write_job(job: Job) -> std::io::Result<()> {
    let image = match job.pixels {
        Pixels::Rgba(img) => img,
        #[cfg(target_os = "linux")]
        Pixels::Portal(frame) => {
            // Recadre sur le contenu (retire le noir dont Mutter entoure les
            // captures de fenêtre).
            let (rgba, w, h) = frame
                .to_rgba_cropped()
                .ok_or_else(|| std::io::Error::other("format de frame non géré"))?;
            RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| std::io::Error::other("dimensions de frame incohérentes"))?
        }
    };
    let bytes = encode_webp(&image, job.quality);
    std::fs::write(&job.path, bytes)
}
