//! Backend d'entrées via evdev (`/dev/input`) — fonctionne sous Wayland comme
//! sous X11, indépendamment du compositeur. Nécessite l'accès en lecture aux
//! périphériques (règle udev posée à l'installation, cf. `packaging/`).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use evdev::{Device, EventType};

use super::{Button, InputBackend, InputEvent, InputHandle, Keycode};

/// Capture en lisant directement les périphériques `/dev/input`.
pub struct EvdevBackend;

impl InputBackend for EvdevBackend {
    fn name(&self) -> &'static str {
        "evdev (/dev/input)"
    }

    fn start(&self, tx: Sender<InputEvent>) -> io::Result<InputHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();

        for (path, device) in evdev::enumerate() {
            // Seuls les périphériques émettant des EV_KEY nous intéressent
            // (claviers + souris : les boutons souris sont aussi des EV_KEY).
            if !device.supported_events().contains(EventType::KEY) {
                continue;
            }
            if device.set_nonblocking(true).is_err() {
                continue;
            }
            let tx = tx.clone();
            let stop = stop.clone();
            let handle = std::thread::Builder::new()
                .name(format!("evdev:{}", path.display()))
                .spawn(move || read_loop(device, tx, stop))
                .map_err(io::Error::other)?;
            threads.push(handle);
        }

        if threads.is_empty() {
            return Err(io::Error::other(
                "aucun périphérique /dev/input lisible — règle udev manquante \
                 (ou utilisateur hors du groupe input) ?",
            ));
        }

        Ok(InputHandle::new(EvdevGuard {
            stop,
            threads: Some(threads),
        }))
    }
}

fn read_loop(mut device: Device, tx: Sender<InputEvent>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match device.fetch_events() {
            Ok(events) => {
                for ev in events {
                    if ev.event_type() != EventType::KEY {
                        continue;
                    }
                    if let Some(out) = translate(ev.code(), ev.value()) {
                        if tx.send(out).is_err() {
                            return;
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(8));
            }
            Err(_) => return, // périphérique débranché
        }
    }
}

/// Traduit un EV_KEY brut (code Linux, valeur) en événement normalisé.
///
/// `value` : 1 = appui, 0 = relâchement, 2 = répétition automatique.
fn translate(code: u16, value: i32) -> Option<InputEvent> {
    if let Some(button) = mouse_button(code) {
        // Capture au relâchement, comme le backend device_query.
        return (value == 0).then_some(InputEvent::MouseUp { button, pos: None });
    }
    let key = keycode(code)?;
    match value {
        1 => Some(InputEvent::KeyDown(key)),
        0 => Some(InputEvent::KeyUp(key)),
        _ => None, // répétition ignorée
    }
}

fn mouse_button(code: u16) -> Option<Button> {
    match code {
        0x110 => Some(Button::Left),
        0x111 => Some(Button::Right),
        0x112 => Some(Button::Middle),
        _ => None,
    }
}

/// Mappe un code clavier Linux (`input-event-codes.h`) vers [`Keycode`].
fn keycode(code: u16) -> Option<Keycode> {
    use Keycode::*;
    Some(match code {
        2 => Key1,
        3 => Key2,
        4 => Key3,
        5 => Key4,
        6 => Key5,
        7 => Key6,
        8 => Key7,
        9 => Key8,
        10 => Key9,
        11 => Key0,
        12 => Minus,
        13 => Equal,
        14 => Backspace,
        15 => Tab,
        16 => Q,
        17 => W,
        18 => E,
        19 => R,
        20 => T,
        21 => Y,
        22 => U,
        23 => I,
        24 => O,
        25 => P,
        26 => LeftBracket,
        27 => RightBracket,
        28 => Enter,
        29 => LControl,
        30 => A,
        31 => S,
        32 => D,
        33 => F,
        34 => G,
        35 => H,
        36 => J,
        37 => K,
        38 => L,
        39 => Semicolon,
        40 => Apostrophe,
        41 => Grave,
        42 => LShift,
        43 => BackSlash,
        44 => Z,
        45 => X,
        46 => C,
        47 => V,
        48 => B,
        49 => N,
        50 => M,
        51 => Comma,
        52 => Dot,
        53 => Slash,
        54 => RShift,
        56 => LAlt,
        57 => Space,
        96 => NumpadEnter,
        97 => RControl,
        100 => RAlt,
        _ => return None,
    })
}

/// Arrête les threads de lecture quand la poignée est détruite.
struct EvdevGuard {
    stop: Arc<AtomicBool>,
    threads: Option<Vec<JoinHandle<()>>>,
}

impl Drop for EvdevGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(threads) = self.threads.take() {
            for t in threads {
                let _ = t.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Codes issus de `input-event-codes.h`.
    const KEY_A: u16 = 30;
    const KEY_LEFTSHIFT: u16 = 42;
    const BTN_LEFT: u16 = 0x110;
    const BTN_RIGHT: u16 = 0x111;
    const BTN_MIDDLE: u16 = 0x112;

    #[test]
    fn keycode_maps_known_codes() {
        assert_eq!(keycode(KEY_A), Some(Keycode::A));
        assert_eq!(keycode(KEY_LEFTSHIFT), Some(Keycode::LShift));
        assert_eq!(keycode(28), Some(Keycode::Enter));
        assert_eq!(keycode(57), Some(Keycode::Space));
    }

    #[test]
    fn keycode_rejects_unknown_code() {
        // Un bouton souris n'est pas une touche clavier.
        assert_eq!(keycode(BTN_LEFT), None);
        // Code non mappé (KEY_F1 = 59, hors table).
        assert_eq!(keycode(59), None);
    }

    #[test]
    fn mouse_button_maps_the_three_buttons() {
        assert_eq!(mouse_button(BTN_LEFT), Some(Button::Left));
        assert_eq!(mouse_button(BTN_RIGHT), Some(Button::Right));
        assert_eq!(mouse_button(BTN_MIDDLE), Some(Button::Middle));
        assert_eq!(mouse_button(KEY_A), None);
    }

    #[test]
    fn translate_key_press_and_release() {
        assert!(matches!(
            translate(KEY_A, 1),
            Some(InputEvent::KeyDown(Keycode::A))
        ));
        assert!(matches!(
            translate(KEY_A, 0),
            Some(InputEvent::KeyUp(Keycode::A))
        ));
    }

    #[test]
    fn translate_ignores_key_autorepeat() {
        // value == 2 : répétition automatique, ignorée.
        assert!(translate(KEY_A, 2).is_none());
    }

    #[test]
    fn translate_mouse_only_on_release() {
        // Appui (value == 1) : rien, on capture au relâchement.
        assert!(translate(BTN_LEFT, 1).is_none());
        // Relâchement (value == 0) : MouseUp sans position (evdev/Wayland).
        assert!(matches!(
            translate(BTN_LEFT, 0),
            Some(InputEvent::MouseUp {
                button: Button::Left,
                pos: None,
            })
        ));
    }

    #[test]
    fn translate_unknown_key_is_none() {
        assert!(translate(59, 1).is_none());
    }
}
