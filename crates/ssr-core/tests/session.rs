use std::fs;

use ssr_core::export;
use ssr_core::model::WindowInfo;
use ssr_core::session::Session;
use ssr_core::text::{key_to_token, KeyToken, Keycode};
use ssr_core::{Button, Lang};

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("ssr-test-{tag}-{nanos}"))
}

#[test]
fn keys_translate_with_shift() {
    assert!(matches!(key_to_token(&Keycode::A, false), Some(KeyToken::Insert(s)) if s == "a"));
    assert!(matches!(key_to_token(&Keycode::A, true), Some(KeyToken::Insert(s)) if s == "A"));
    assert!(matches!(key_to_token(&Keycode::Key1, true), Some(KeyToken::Insert(s)) if s == "!"));
    assert!(matches!(
        key_to_token(&Keycode::Backspace, false),
        Some(KeyToken::Backspace)
    ));
    assert!(key_to_token(&Keycode::F5, false).is_none());
}

#[test]
fn records_text_then_click_and_exports() {
    let dir = unique_dir("flow");
    let mut session = Session::new(&dir, Lang::Fr).unwrap();

    // Saisie « Bonjour » : Maj+b puis « onjour ».
    session.apply_key(KeyToken::Insert("B".into()));
    for c in "onjour".chars() {
        session.apply_key(KeyToken::Insert(c.to_string()));
    }

    let window = Some(WindowInfo {
        title: "Éditeur".into(),
        app_name: "demo".into(),
        pid: 42,
        x: 100,
        y: 50,
        width: 800,
        height: 600,
    });

    // Un clic vide d'abord le texte (étape Text), puis crée l'étape Click.
    session.record_click(Button::Left, Some((260, 130)), window, None);
    session.stop();

    assert_eq!(session.steps.len(), 2, "une étape Text + une étape Click");
    assert!(matches!(
        session.steps[0].action,
        ssr_core::Action::Text { .. }
    ));
    if let ssr_core::Action::Click { rel_x, rel_y, .. } = session.steps[1].action {
        assert_eq!(
            (rel_x, rel_y),
            (Some(160), Some(80)),
            "coords relatives à la fenêtre"
        );
    } else {
        panic!("la 2e étape doit être un clic");
    }

    // Export complet.
    let zip = dir.with_extension("zip");
    export::export_zip(&session, &zip, &ssr_core::Progress::new(), Lang::Fr).unwrap();
    assert!(zip.exists(), "l'archive ZIP doit être créée");
    assert!(dir.join("report.html").exists());
    assert!(dir.join("steps.json").exists());

    let html = fs::read_to_string(dir.join("report.html")).unwrap();
    assert!(html.contains("Bonjour"));
    assert!(html.contains("Support Step Recorder"));

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&zip);
}
