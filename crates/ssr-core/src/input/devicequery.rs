//! Backend d'entrées via `device_query` (X11, Windows, macOS).

use std::io;
use std::sync::mpsc::Sender;

use device_query::{DeviceEvents, DeviceEventsHandler, DeviceQuery, DeviceState, MouseButton};
use std::time::Duration;

use super::{Button, InputBackend, InputEvent, InputHandle};

/// Capture via la boucle d'événements de `device_query`.
pub struct DeviceQueryBackend;

impl InputBackend for DeviceQueryBackend {
    fn name(&self) -> &'static str {
        "device_query (X11/Windows/macOS)"
    }

    fn start(&self, tx: Sender<InputEvent>) -> io::Result<InputHandle> {
        let handler = DeviceEventsHandler::new(Duration::from_millis(10))
            .ok_or_else(|| io::Error::other("boucle d'événements indisponible"))?;

        // Chaque garde maintient son callback abonné ; on les conserve toutes.
        let mut guards: Vec<Box<dyn std::any::Any + Send + Sync>> = Vec::new();

        let t = tx.clone();
        guards.push(Box::new(handler.on_mouse_up(move |button| {
            let pos = Some(DeviceState::new().get_mouse().coords);
            let _ = t.send(InputEvent::MouseUp {
                button: from_dq_button(*button),
                pos,
            });
        })));

        let t = tx.clone();
        guards.push(Box::new(handler.on_key_down(move |key| {
            let _ = t.send(InputEvent::KeyDown(*key));
        })));

        let t = tx;
        guards.push(Box::new(handler.on_key_up(move |key| {
            let _ = t.send(InputEvent::KeyUp(*key));
        })));

        // On garde handler + gardes vivants dans la poignée.
        Ok(InputHandle::new((handler, guards)))
    }
}

fn from_dq_button(b: MouseButton) -> Button {
    match b {
        1 => Button::Left,
        2 => Button::Middle,
        3 => Button::Right,
        other => Button::Other(other as u16),
    }
}
