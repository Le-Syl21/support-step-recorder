//! Sonde xcap : teste la détection de fenêtres et la capture d'écran sur
//! l'environnement courant (notamment Wayland).

use xcap::{Monitor, Window};

fn main() {
    match Window::all() {
        Ok(wins) => {
            let focused = wins
                .iter()
                .filter(|w| w.is_focused().unwrap_or(false))
                .count();
            println!(
                "Window::all = {} fenêtre(s), dont {focused} focus",
                wins.len()
            );
            for w in wins.iter().take(5) {
                println!(
                    "  - «{}» app={} focus={:?} {}x{}",
                    w.title().unwrap_or_default(),
                    w.app_name().unwrap_or_default(),
                    w.is_focused(),
                    w.width().unwrap_or(0),
                    w.height().unwrap_or(0),
                );
            }
        }
        Err(e) => println!("Window::all ERREUR: {e}"),
    }

    match Monitor::all() {
        Ok(mons) => {
            println!("Monitor::all = {} moniteur(s)", mons.len());
            if let Some(m) = mons.first() {
                match m.capture_image() {
                    Ok(img) => {
                        let bytes =
                            webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height())
                                .encode(80.0)
                                .to_vec();
                        std::fs::write("/tmp/ssr-shot.webp", &bytes).unwrap();
                        println!(
                            "Capture moniteur OK: {}x{} -> /tmp/ssr-shot.webp ({} octets)",
                            img.width(),
                            img.height(),
                            bytes.len()
                        );
                    }
                    Err(e) => println!("capture_image ERREUR: {e}"),
                }
            }
        }
        Err(e) => println!("Monitor::all ERREUR: {e}"),
    }
}
