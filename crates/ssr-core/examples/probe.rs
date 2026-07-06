//! Sonde de diagnostic : affiche les événements clavier/souris captés par
//! `device_query` pendant ~10 s. Sert à vérifier si la capture globale
//! fonctionne dans l'environnement courant (X11 vs Wayland).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use device_query::{DeviceEvents, DeviceEventsHandler};

fn main() {
    let handler = DeviceEventsHandler::new(Duration::from_millis(10))
        .expect("boucle d'événements indisponible");

    let count = Arc::new(AtomicUsize::new(0));

    let c = count.clone();
    let _g1 = handler.on_mouse_down(move |b| {
        c.fetch_add(1, Ordering::SeqCst);
        println!("MOUSE DOWN {b:?}");
    });
    let c = count.clone();
    let _g2 = handler.on_key_down(move |k| {
        c.fetch_add(1, Ordering::SeqCst);
        println!("KEY DOWN  {k:?}");
    });

    println!(">> Cliquez et tapez n'importe où pendant 10 secondes...");
    std::thread::sleep(Duration::from_secs(10));
    println!(
        ">> Terminé. {} événement(s) capté(s).",
        count.load(Ordering::SeqCst)
    );
}
