//! `.ssrproj` save/reopen and the exported guide ZIP.

mod common;

use common::*;
use ssr_core::editor::EditorDocument;
use ssr_core::import::{import_recording, ImportError, ImportLimits};
use ssr_core::{export, Lang, Progress, Step};

fn merged_guide(scratch: &Scratch) -> EditorDocument {
    let (alpha, beta) = (alpha(&scratch.0), beta(&scratch.0));
    let mut doc = new_doc(scratch);
    let limits = ImportLimits::default();
    let a = import_recording(&alpha, doc.assets(), &limits).unwrap();
    doc.import_steps(0, &a, &[0, 1]).unwrap();
    let b = import_recording(&beta, doc.assets(), &limits).unwrap();
    doc.import_steps(2, &b, &[0, 1]).unwrap();
    doc
}

/// A11 + A21: everything edited survives a save and a reopen in another
/// workspace, which is private.
#[test]
fn a11_project_round_trip() {
    let scratch = Scratch::new("a11");
    let mut doc = merged_guide(&scratch);
    doc.set_guide_info("Network setup".into(), "Follow these steps.".into())
        .unwrap();
    let ids: Vec<_> = doc.steps().iter().map(|s| s.id.unwrap()).collect();
    doc.set_title(ids[0], Some("Open Settings".into())).unwrap();
    doc.set_notes(ids[0], "Use the gear icon.\nTop right.".into())
        .unwrap();
    doc.detach_screenshot(ids[2]).unwrap();
    doc.move_step(ids[3], 0).unwrap();
    let expected: Vec<Step> = doc.steps().to_vec();

    let path = scratch.path("guide.ssrproj");
    doc.save_project(&path).unwrap();
    assert!(!doc.is_dirty());
    doc.close();

    let (reopened, warnings) =
        EditorDocument::open_project(&path, workspace(&scratch), &ImportLimits::default())
            .map_err(|(e, _)| e)
            .unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(!reopened.is_dirty(), "a freshly opened project is clean");
    assert_eq!(reopened.project_path(), Some(path.as_path()));
    assert_eq!(reopened.metadata().title, "Network setup");
    assert_eq!(reopened.metadata().description, "Follow these steps.");
    assert_eq!(reopened.metadata().sources.len(), 2);
    assert_eq!(reopened.steps().len(), expected.len());
    for (a, b) in reopened.steps().iter().zip(&expected) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.index, b.index);
        assert_eq!(a.title_override, b.title_override);
        assert_eq!(a.notes, b.notes);
        assert_eq!(a.screenshot, b.screenshot);
        assert_eq!(a.source, b.source);
        assert_eq!(
            a.timestamp_ms, b.timestamp_ms,
            "A22: capture time preserved"
        );
        if let Some(r) = &a.screenshot {
            assert!(reopened.assets().contains(r));
        }
    }
    // Only referenced images are in the project.
    let names = zip_names(&path);
    let assets_in_zip = names.iter().filter(|n| n.starts_with("assets/")).count();
    let referenced: std::collections::HashSet<_> = expected
        .iter()
        .filter_map(|s| s.screenshot.clone())
        .collect();
    assert_eq!(assets_in_zip, referenced.len());
    assert!(
        !names.iter().any(|n| n.ends_with(".html")),
        "no HTML in a project"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(reopened.workspace_dir())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(m & 0o777, 0o700);
    }
    reopened.close();
}

/// A12 + A03 + A19: the exported guide has the report, numbered steps with
/// the edits, only the images still referenced, escaped text.
#[test]
fn a12_export_contains_only_what_is_referenced() {
    let scratch = Scratch::new("a12");
    let mut doc = merged_guide(&scratch);
    let ids: Vec<_> = doc.steps().iter().map(|s| s.id.unwrap()).collect();
    let detached = doc.step(ids[2]).unwrap().screenshot.clone().unwrap();
    doc.detach_screenshot(ids[2]).unwrap();
    doc.set_title(
        ids[0],
        Some("<script>alert(1)</script> & \"quotes\" 'x'".into()),
    )
    .unwrap();
    doc.set_notes(ids[0], "<img src=x onerror=alert(1)>\nline 2".into())
        .unwrap();
    doc.set_guide_info("Guide <b>bold</b>".into(), String::new())
        .unwrap();
    doc.delete_step(ids[1]).unwrap();

    let zip = scratch.path("guide.zip");
    let snap = doc.snapshot();
    export::export_guide(&snap, &zip, &Progress::new(), Lang::En).unwrap();

    let names = zip_names(&zip);
    assert!(names.contains(&"report.html".into()));
    assert!(names.contains(&"steps.json".into()));
    assert!(!names.contains(&detached), "orphan image excluded");
    let steps: Vec<Step> = serde_json::from_slice(&zip_member(&zip, "steps.json")).unwrap();
    assert_eq!(steps.len(), 3);
    for (i, s) in steps.iter().enumerate() {
        assert_eq!(s.index, i + 1);
        if let Some(r) = &s.screenshot {
            assert!(names.contains(r), "every reference is in the archive");
        }
    }
    let html = String::from_utf8(zip_member(&zip, "report.html")).unwrap();
    assert!(!html.contains("<script>alert"), "title escaped");
    assert!(!html.contains("<img src=x"), "notes escaped");
    assert!(!html.contains("<b>bold"), "guide title escaped");
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("&#39;x&#39;"));
    assert!(html.contains("line 2"));
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "offline"
    );
    // Every <img src> of the report is in the archive.
    for part in html.split("<img src=\"").skip(1) {
        let src = &part[..part.find('"').unwrap()];
        assert!(
            names.contains(&src.to_string()),
            "{src} missing from the ZIP"
        );
    }
    // A22: a merged guide does not claim a global elapsed time.
    assert!(!html.contains("⏱"));
    // A legacy reader (v0.2.0 fields only) still parses the export.
    let raw: Vec<serde_json::Value> =
        serde_json::from_slice(&zip_member(&zip, "steps.json")).unwrap();
    for v in raw {
        for key in [
            "index",
            "timestamp_ms",
            "elapsed_ms",
            "action",
            "description",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
    }
    doc.close();
}

/// An exported guide can itself be reopened as a recording, edits included.
#[test]
fn exported_guide_reimports_with_its_edits() {
    let scratch = Scratch::new("reimport");
    let mut doc = merged_guide(&scratch);
    let id = doc.steps()[0].id.unwrap();
    doc.set_title(id, Some("Custom".into())).unwrap();
    let zip = scratch.path("guide.zip");
    export::export_guide(&doc.snapshot(), &zip, &Progress::new(), Lang::En).unwrap();
    let rec = import_recording(&zip, doc.assets(), &ImportLimits::default()).unwrap();
    assert_eq!(rec.steps[0].step.title_override.as_deref(), Some("Custom"));
    assert!(rec.steps.iter().all(|s| s.image_warning.is_none()));
    doc.close();
}

/// A17: an empty guide exports without trouble (and the replay script
/// guards against zero steps).
#[test]
fn a17_empty_guide_exports() {
    let scratch = Scratch::new("a17");
    let doc = new_doc(&scratch);
    let zip = scratch.path("empty.zip");
    export::export_guide(&doc.snapshot(), &zip, &Progress::new(), Lang::Fr).unwrap();
    let html = String::from_utf8(zip_member(&zip, "report.html")).unwrap();
    assert!(html.contains("n === 0"));
    let steps: Vec<Step> = serde_json::from_slice(&zip_member(&zip, "steps.json")).unwrap();
    assert!(steps.is_empty());
    doc.close();
}

/// A15: exporting to a place that cannot be written fails cleanly, and an
/// earlier export at the same path is left intact.
#[test]
fn a15_failed_export_keeps_previous_file() {
    let scratch = Scratch::new("a15");
    let doc = merged_guide(&scratch);
    let bad = scratch.path("no/such/dir/guide.zip");
    assert!(export::export_guide(&doc.snapshot(), &bad, &Progress::new(), Lang::En).is_err());

    // A previous good export, then a failing one (an image vanished).
    let zip = scratch.path("guide.zip");
    export::export_guide(&doc.snapshot(), &zip, &Progress::new(), Lang::En).unwrap();
    let before = sha(&zip);
    let snap = doc.snapshot();
    let victim = snap
        .steps
        .iter()
        .find_map(|s| s.screenshot.clone())
        .unwrap();
    std::fs::remove_file(snap.assets.path_of(&victim).unwrap()).unwrap();
    let err = export::export_guide(&snap, &zip, &Progress::new(), Lang::En).unwrap_err();
    assert!(matches!(err, ssr_core::project::SaveError::MissingAsset(_)));
    assert_eq!(sha(&zip), before, "previous export intact");
    let leftovers: Vec<_> = std::fs::read_dir(&scratch.0)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no temporary file left behind");
    doc.close();
}

/// A save over an existing project replaces it only once complete.
#[test]
fn failed_save_keeps_previous_project() {
    let scratch = Scratch::new("atomic-save");
    let mut doc = merged_guide(&scratch);
    let path = scratch.path("guide.ssrproj");
    doc.save_project(&path).unwrap();
    let before = sha(&path);
    let victim = doc
        .steps()
        .iter()
        .find_map(|s| s.screenshot.clone())
        .unwrap();
    std::fs::remove_file(doc.assets().path_of(&victim).unwrap()).unwrap();
    let id = doc.steps()[0].id.unwrap();
    doc.set_notes(id, "changed".into()).unwrap();
    assert!(doc.save_project(&path).is_err());
    assert_eq!(sha(&path), before);
    assert!(doc.is_dirty());
    doc.close();
}

/// US-11: a project from a newer format is refused and left untouched.
#[test]
fn future_schema_is_refused() {
    let scratch = Scratch::new("future");
    let path = scratch.path("future.ssrproj");
    write_zip(
        &path,
        &[
            (
                "project.json",
                br#"{"schema_version": 2, "title": "x", "created_at": "", "modified_at": ""}"#,
            ),
            ("steps.json", b"[]"),
        ],
    );
    let before = sha(&path);
    let err = EditorDocument::open_project(&path, workspace(&scratch), &ImportLimits::default())
        .map(|_| ())
        .unwrap_err();
    assert!(
        matches!(err.0, ImportError::UnsupportedVersion(2)),
        "{:?}",
        err.0
    );
    assert!(err.0.message(Lang::En).contains("newer version"));
    err.1.discard();
    assert_eq!(sha(&path), before);
}

/// A project image whose bytes do not match its name is not trusted.
#[test]
fn tampered_project_image_is_dropped_with_a_warning() {
    let scratch = Scratch::new("tamper");
    let mut doc = merged_guide(&scratch);
    let path = scratch.path("guide.ssrproj");
    doc.save_project(&path).unwrap();
    doc.close();
    // Rewrite the project with one image swapped for other bytes.
    let names = zip_names(&path);
    let target = names
        .iter()
        .find(|n| n.starts_with("assets/"))
        .unwrap()
        .clone();
    let other = webp(12, 12, [0, 0, 0]);
    let members: Vec<(String, Vec<u8>)> = names
        .iter()
        .map(|n| {
            let data = if *n == target {
                other.clone()
            } else {
                zip_member(&path, n)
            };
            (n.clone(), data)
        })
        .collect();
    let refs: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    let tampered = scratch.path("tampered.ssrproj");
    write_zip(&tampered, &refs);
    let (doc, warnings) =
        EditorDocument::open_project(&tampered, workspace(&scratch), &ImportLimits::default())
            .map_err(|(e, _)| e)
            .unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(doc.is_dirty());
    assert!(!doc
        .steps()
        .iter()
        .any(|s| s.screenshot.as_deref() == Some(target.as_str())));
    doc.close();
}

/// The legacy session export still works and is atomic too.
#[test]
fn legacy_session_export_still_works() {
    let scratch = Scratch::new("legacy-export");
    let dir = scratch.path("session");
    let mut session = ssr_core::Session::new(&dir, Lang::En).unwrap();
    session.record_click(ssr_core::Button::Left, Some((1, 2)), None, None);
    session.stop();
    let zip = scratch.path("s.zip");
    export::export_zip(&session, &zip, &Progress::new(), Lang::En).unwrap();
    let html = String::from_utf8(zip_member(&zip, "report.html")).unwrap();
    assert!(
        html.contains("⏱"),
        "a single session keeps its elapsed time"
    );
}
