//! Editing invariants: numbering, stable ids, undo/redo, dirty state.

mod common;

use common::*;
use ssr_core::editor::{EditError, EditKind, EditorDocument, REDACTED};
use ssr_core::model::WindowInfo;
use ssr_core::session::Session;
use ssr_core::text::KeyToken;
use ssr_core::{Action, Button, Lang, Step};

fn doc_with(scratch: &Scratch, n: usize) -> EditorDocument {
    let mut doc = new_doc(scratch);
    let steps = (0..n)
        .map(|i| Step::manual(format!("Step {}", i + 1), None))
        .collect();
    doc.insert_steps(0, steps).unwrap();
    doc
}

fn titles(doc: &EditorDocument) -> Vec<String> {
    doc.steps()
        .iter()
        .map(|s| s.display_title(Lang::En))
        .collect()
}

fn assert_numbered(doc: &EditorDocument) {
    for (i, s) in doc.steps().iter().enumerate() {
        assert_eq!(s.index, i + 1, "index must equal position + 1");
    }
}

/// A01: a stopped recording becomes an editable guide with every click,
/// its screenshots moved into the workspace.
#[test]
fn a01_stopped_session_becomes_a_guide() {
    let scratch = Scratch::new("a01");
    let ws = workspace(&scratch);
    let capture = ws.subdir("capture").unwrap();
    let mut session = Session::new(&capture, Lang::En).unwrap();
    for i in 0..5 {
        let name = session.next_screenshot_name();
        std::fs::write(capture.join(&name), webp(8, 8, [i * 40, 0, 0])).unwrap();
        session.record_click(Button::Left, Some((i as i32, 0)), None, Some(name));
    }
    session.stop();
    let doc = EditorDocument::from_session(&session, ws, "Recording", Lang::En).unwrap();
    assert_eq!(doc.steps().len(), 5);
    assert!(doc.is_dirty(), "a new recording is unsaved");
    assert_numbered(&doc);
    for s in doc.steps() {
        let r = s.screenshot.as_deref().expect("screenshot kept");
        assert!(ssr_core::assets::is_asset_ref(r), "{r}");
        assert!(doc.assets().contains(r));
    }
    assert!(!capture.exists(), "the capture folder is cleaned up");
    doc.close();
}

/// Stop with zero steps: an empty, usable guide.
#[test]
fn empty_session_gives_an_empty_guide() {
    let scratch = Scratch::new("empty");
    let ws = workspace(&scratch);
    let capture = ws.subdir("capture").unwrap();
    let mut session = Session::new(&capture, Lang::En).unwrap();
    session.stop();
    let mut doc = EditorDocument::from_session(&session, ws, "Recording", Lang::En).unwrap();
    assert!(doc.steps().is_empty());
    assert!(doc.is_dirty());
    let id = doc.add_manual_step(0, "First", None).unwrap();
    assert_eq!(doc.position(id), Some(0));
    doc.close();
}

/// A02: rename and notes touch only that step; blank title means automatic.
#[test]
fn a02_rename_and_notes() {
    let scratch = Scratch::new("a02");
    let mut doc = doc_with(&scratch, 3);
    let id = doc.steps()[1].id.unwrap();
    doc.set_title(id, Some("  Open the Settings panel ".into()))
        .unwrap();
    doc.set_notes(id, "Line one\nLine two\n".into()).unwrap();
    let s = doc.step(id).unwrap();
    assert_eq!(s.title_override.as_deref(), Some("Open the Settings panel"));
    assert_eq!(s.notes, "Line one\nLine two");
    assert_eq!(titles(&doc)[0], "Step 1");
    assert_eq!(titles(&doc)[2], "Step 3");
    assert_eq!(
        doc.set_title(id, Some("Open the Settings panel".into())),
        Err(EditError::NoChange)
    );
    doc.set_title(id, Some("   ".into())).unwrap();
    assert_eq!(doc.step(id).unwrap().title_override, None);
    assert_eq!(
        doc.step(id).unwrap().display_title(Lang::En),
        "Untitled step"
    );
    doc.close();
}

/// A04 / A05: delete renumbers, reorder keeps ids.
#[test]
fn a04_a05_delete_and_reorder() {
    let scratch = Scratch::new("a04");
    let mut doc = doc_with(&scratch, 5);
    let ids: Vec<_> = doc.steps().iter().map(|s| s.id.unwrap()).collect();
    doc.delete_step(ids[3]).unwrap();
    assert_eq!(doc.steps().len(), 4);
    assert_numbered(&doc);
    assert_eq!(doc.delete_step(ids[3]), Err(EditError::UnknownStep(ids[3])));

    doc.move_step(ids[4], 0).unwrap();
    assert_eq!(titles(&doc), ["Step 5", "Step 1", "Step 2", "Step 3"]);
    assert_eq!(doc.steps()[0].id, Some(ids[4]), "id unchanged by a move");
    assert_numbered(&doc);
    assert_eq!(
        doc.move_step(ids[0], 9),
        Err(EditError::OutOfRange { index: 9, len: 4 })
    );
    assert_eq!(doc.move_step(ids[4], 0), Err(EditError::NoChange));
    doc.close();
}

/// A16: undo/redo restore the whole state, including after a delete,
/// a detach and an import-like batch; the dirty flag follows.
#[test]
fn a16_undo_redo_and_dirty_state() {
    let scratch = Scratch::new("a16");
    let mut doc = new_doc(&scratch);
    let (img, _) = doc
        .assets()
        .put_checked(&png(4, 4, [1, 2, 3]), 1_000_000)
        .unwrap();
    let id = doc
        .add_manual_step(0, "With image", Some(img.clone()))
        .unwrap();
    doc.add_manual_step(1, "Second", None).unwrap();
    let path = scratch.path("guide.ssrproj");
    doc.save_project(&path).unwrap();
    assert!(!doc.is_dirty());
    let before: Vec<_> = doc.steps().to_vec();

    doc.detach_screenshot(id).unwrap();
    assert!(doc.is_dirty());
    assert_eq!(doc.undo(), Ok(EditKind::DetachImage));
    assert_eq!(
        doc.step(id).unwrap().screenshot.as_deref(),
        Some(img.as_str())
    );
    assert!(!doc.is_dirty(), "back to the saved state");

    doc.delete_step(id).unwrap();
    assert_eq!(doc.steps().len(), 1);
    doc.undo().unwrap();
    assert_eq!(doc.steps().len(), 2);
    assert_eq!(
        doc.steps()
            .iter()
            .map(|s| (s.id, s.index))
            .collect::<Vec<_>>(),
        before.iter().map(|s| (s.id, s.index)).collect::<Vec<_>>()
    );
    assert_eq!(doc.redo(), Ok(EditKind::Delete));
    assert_eq!(doc.steps().len(), 1);
    assert!(doc.is_dirty());
    doc.undo().unwrap();

    // A batch is one undo step.
    let batch = vec![
        Step::manual("a", None),
        Step::manual("b", None),
        Step::manual("c", None),
    ];
    doc.insert_steps(1, batch).unwrap();
    assert_eq!(doc.steps().len(), 5);
    doc.undo().unwrap();
    assert_eq!(doc.steps().len(), 2);
    assert!(!doc.is_dirty());

    // A new edit clears the redo stack.
    doc.redo().unwrap();
    doc.undo().unwrap();
    doc.set_notes(id, "x".into()).unwrap();
    assert_eq!(doc.redo(), Err(EditError::EmptyHistory));
    doc.close();
}

/// An export does not make the project clean.
#[test]
fn export_does_not_clear_dirty() {
    let scratch = Scratch::new("exp-dirty");
    let mut doc = doc_with(&scratch, 1);
    let zip = scratch.path("out.zip");
    let snap = doc.snapshot();
    ssr_core::export::export_guide(&snap, &zip, &ssr_core::Progress::new(), Lang::En).unwrap();
    doc.mark_exported(&zip, snap.revision);
    assert!(doc.is_exported());
    assert!(doc.is_dirty());
    doc.close();
}

/// Attaching needs an image of the workspace; inserting needs a valid spot.
#[test]
fn rejects_foreign_assets_and_bad_positions() {
    let scratch = Scratch::new("reject");
    let mut doc = doc_with(&scratch, 2);
    let id = doc.steps()[0].id.unwrap();
    let fake = format!("assets/{}.png", "0".repeat(64));
    assert_eq!(
        doc.attach_screenshot(id, fake.clone()),
        Err(EditError::InvalidAsset(fake))
    );
    assert!(matches!(
        doc.insert_steps(5, vec![Step::manual("x", None)]),
        Err(EditError::OutOfRange { .. })
    ));
    doc.close();
}

/// Typed text can be redacted (a password typed by mistake).
#[test]
fn redacts_typed_text() {
    let scratch = Scratch::new("redact");
    let ws = workspace(&scratch);
    let capture = ws.subdir("capture").unwrap();
    let mut session = Session::new(&capture, Lang::En).unwrap();
    for c in "s3cret".chars() {
        session.apply_key(KeyToken::Insert(c.to_string()));
    }
    let w = WindowInfo {
        title: "Login".into(),
        ..Default::default()
    };
    session.record_click(Button::Left, None, Some(w), None);
    session.stop();
    let mut doc = EditorDocument::from_session(&session, ws, "R", Lang::En).unwrap();
    let id = doc.steps()[0].id.unwrap();
    doc.redact_text(id).unwrap();
    let s = doc.step(id).unwrap();
    assert!(matches!(&s.action, Action::Text { content } if content == REDACTED));
    assert!(!s.description.contains("s3cret"));
    assert!(!serde_json::to_string(doc.steps())
        .unwrap()
        .contains("s3cret"));
    doc.close();
}

/// A18: a custom title is kept literally; the automatic one follows the
/// requested language.
#[test]
fn a18_titles_and_languages() {
    let scratch = Scratch::new("a18");
    let ws = workspace(&scratch);
    let capture = ws.subdir("capture").unwrap();
    let mut session = Session::new(&capture, Lang::Fr).unwrap();
    let w = WindowInfo {
        title: "Paramètres".into(),
        app_name: "gnome-control-center".into(),
        ..Default::default()
    };
    session.record_click(Button::Left, None, Some(w.clone()), None);
    session.record_click(Button::Right, None, Some(w), None);
    session.stop();
    let mut doc = EditorDocument::from_session(&session, ws, "R", Lang::Fr).unwrap();
    let first = doc.steps()[0].id.unwrap();
    doc.set_title(first, Some("Ouvrir les paramètres".into()))
        .unwrap();
    assert_eq!(
        doc.steps()[0].display_title(Lang::En),
        "Ouvrir les paramètres"
    );
    assert!(doc.steps()[1]
        .display_title(Lang::En)
        .starts_with("Right click"));
    assert!(doc.steps()[1]
        .display_title(Lang::Fr)
        .starts_with("Clic droit"));
    doc.close();
}

/// The history is bounded.
#[test]
fn history_is_bounded() {
    let scratch = Scratch::new("hist");
    let mut doc = doc_with(&scratch, 1);
    let id = doc.steps()[0].id.unwrap();
    for i in 0..(ssr_core::editor::HISTORY_LIMIT + 10) {
        doc.set_notes(id, format!("n{i}")).unwrap();
    }
    let mut undone = 0;
    while doc.undo().is_ok() {
        undone += 1;
    }
    assert_eq!(undone, ssr_core::editor::HISTORY_LIMIT);
    doc.close();
}

/// Crash recovery: a guide dropped without `close` (what a crash leaves)
/// is found at the next start and rebuilt from its recovery copy.
#[test]
fn recovers_after_a_crash() {
    let scratch = Scratch::new("recover");
    let base = scratch.path("workspaces");
    let mut doc = doc_with(&scratch, 2);
    let id = doc.steps()[1].id.unwrap();
    doc.set_title(id, Some("Edited before the crash".into()))
        .unwrap();
    assert!(doc.autosave(std::time::Duration::ZERO));
    let dir = doc.workspace_dir().to_path_buf();
    drop(doc); // no `close`: the workspace stays, its lock is released

    let found = ssr_core::workspace::scan_abandoned(&base);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].dir, dir);
    assert_eq!(found[0].step_count, 2);

    let ws = ssr_core::workspace::Workspace::adopt(&dir).unwrap();
    let doc = EditorDocument::recover(ws).unwrap();
    assert_eq!(
        doc.step(id).unwrap().title_override.as_deref(),
        Some("Edited before the crash")
    );
    assert!(doc.is_dirty(), "recovered work is unsaved");
    doc.close();
    assert!(!dir.exists());
}
