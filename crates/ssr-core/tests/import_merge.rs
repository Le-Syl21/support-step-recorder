//! Opening old recordings and merging several into one guide.

mod common;

use std::collections::HashSet;

use common::*;
use ssr_core::import::{import_image_file, import_recording, ImportLimits};
use ssr_core::{Action, Lang};

/// A06: a v0.2.0 `steps.json` (none of the editor fields) still loads, and
/// each step gets an id and its provenance.
#[test]
fn a06_legacy_steps_load_with_defaults() {
    let scratch = Scratch::new("a06");
    let alpha = alpha(&scratch.0);
    let doc = new_doc(&scratch);
    let rec = import_recording(&alpha, doc.assets(), &ImportLimits::default()).unwrap();
    assert_eq!(rec.label, "recording-alpha.zip");
    assert_eq!(rec.steps.len(), 2);
    for (i, s) in rec.steps.iter().enumerate() {
        assert!(s.image_warning.is_none());
        assert!(s.step.id.is_some());
        let src = s.step.source.as_ref().unwrap();
        assert_eq!(src.original_index, i + 1);
        assert_eq!(src.source_label, "recording-alpha.zip");
        assert_eq!(src.source_digest, rec.source_digest);
        assert!(s.step.title_override.is_none() && s.step.notes.is_empty());
    }
    assert!(matches!(rec.steps[1].step.action, Action::Text { .. }));
    assert!(rec.steps[1].step.screenshot.is_none());
    doc.close();
}

/// A07: Alpha then Beta make four steps; the two different
/// `step-0001.webp` stay two different images. Sources are untouched.
#[test]
fn a07_merge_two_recordings_without_collisions() {
    let scratch = Scratch::new("a07");
    let (alpha, beta) = (alpha(&scratch.0), beta(&scratch.0));
    let (alpha_sum, beta_sum) = (sha(&alpha), sha(&beta));
    let mut doc = new_doc(&scratch);
    let limits = ImportLimits::default();

    let a = import_recording(&alpha, doc.assets(), &limits).unwrap();
    doc.import_steps(doc.steps().len(), &a, &[0, 1]).unwrap();
    let b = import_recording(&beta, doc.assets(), &limits).unwrap();
    doc.import_steps(doc.steps().len(), &b, &[0, 1]).unwrap();

    assert_eq!(doc.steps().len(), 4);
    let shots: Vec<_> = doc
        .steps()
        .iter()
        .filter_map(|s| s.screenshot.clone())
        .collect();
    assert_eq!(shots.len(), 3);
    assert_eq!(
        shots.iter().collect::<HashSet<_>>().len(),
        3,
        "no overwrite"
    );
    for (i, s) in doc.steps().iter().enumerate() {
        assert_eq!(s.index, i + 1);
    }
    assert_eq!(doc.metadata().sources.len(), 2);
    assert_eq!(
        (sha(&alpha), sha(&beta)),
        (alpha_sum, beta_sum),
        "sources unchanged"
    );
    doc.close();
}

/// A08: a subset of Beta inserted after Alpha's first step.
#[test]
fn a08_import_a_subset_at_a_position() {
    let scratch = Scratch::new("a08");
    let (alpha, beta) = (alpha(&scratch.0), beta(&scratch.0));
    let mut doc = new_doc(&scratch);
    let limits = ImportLimits::default();
    let a = import_recording(&alpha, doc.assets(), &limits).unwrap();
    doc.import_steps(0, &a, &[0, 1]).unwrap();
    let b = import_recording(&beta, doc.assets(), &limits).unwrap();
    doc.import_steps(1, &b, &[1]).unwrap();
    let titles: Vec<_> = doc
        .steps()
        .iter()
        .map(|s| s.display_title(Lang::En))
        .collect();
    assert_eq!(titles.len(), 3);
    assert!(titles[0].contains("Settings"), "{titles:?}");
    assert!(titles[1].contains("Apply"), "{titles:?}");
    assert!(titles[2].starts_with("Typed"), "{titles:?}");
    doc.close();
}

/// A09: importing the same archive twice gives new ids each time, and the
/// identical image bytes are stored once.
#[test]
fn a09_same_source_twice() {
    let scratch = Scratch::new("a09");
    let beta = beta(&scratch.0);
    let mut doc = new_doc(&scratch);
    let limits = ImportLimits::default();
    let first = import_recording(&beta, doc.assets(), &limits).unwrap();
    doc.import_steps(0, &first, &[0, 1]).unwrap();
    let second = import_recording(&beta, doc.assets(), &limits).unwrap();
    assert!(second.created_assets.is_empty(), "deduplicated");
    doc.import_steps(2, &second, &[0, 1]).unwrap();
    let ids: HashSet<_> = doc.steps().iter().map(|s| s.id.unwrap()).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(doc.steps()[0].screenshot, doc.steps()[2].screenshot);
    doc.close();
}

/// A10: an external PNG is copied into the guide; the original is untouched.
#[test]
fn a10_attach_an_external_png() {
    let scratch = Scratch::new("a10");
    let png_path = scratch.path("diagram.png");
    std::fs::write(&png_path, png(30, 20, [9, 9, 9])).unwrap();
    let before = sha(&png_path);
    let mut doc = new_doc(&scratch);
    let id = doc.add_manual_step(0, "Check the diagram", None).unwrap();
    let (asset, _) = import_image_file(&png_path, doc.assets(), &ImportLimits::default()).unwrap();
    assert!(asset.ends_with(".png"));
    doc.attach_screenshot(id, asset.clone()).unwrap();
    assert_eq!(
        doc.step(id).unwrap().screenshot.as_deref(),
        Some(asset.as_str())
    );
    assert_eq!(sha(&png_path), before);
    std::fs::remove_file(&png_path).unwrap();
    assert!(doc.assets().contains(&asset), "the guide owns its copy");
    doc.close();
}

/// A text-only file is refused as an image.
#[test]
fn rejects_a_non_image_file() {
    let scratch = Scratch::new("notimg");
    let p = scratch.path("notes.png");
    std::fs::write(&p, b"definitely not a png").unwrap();
    let doc = new_doc(&scratch);
    assert!(import_image_file(&p, doc.assets(), &ImportLimits::default()).is_err());
    doc.close();
}

/// A broken image does not sink the whole import: that step comes without
/// its screenshot, with a warning for the user.
#[test]
fn broken_image_becomes_a_text_only_step() {
    let scratch = Scratch::new("broken");
    let steps = serde_json::to_vec(&vec![
        legacy_click(1, "Good", Some("step-0001.webp")),
        legacy_click(2, "Bad", Some("step-0002.webp")),
        legacy_click(3, "Gone", Some("step-0003.webp")),
    ])
    .unwrap();
    let good = webp(10, 10, [1, 1, 1]);
    let path = scratch.path("partial.zip");
    write_zip(
        &path,
        &[
            ("steps.json", &steps),
            ("step-0001.webp", &good),
            ("step-0002.webp", b"RIFF\x10\0\0\0WEBPVP8 garbage"),
        ],
    );
    let doc = new_doc(&scratch);
    let rec = import_recording(&path, doc.assets(), &ImportLimits::default()).unwrap();
    assert!(rec.steps[0].image_warning.is_none() && rec.steps[0].step.screenshot.is_some());
    assert!(rec.steps[1].image_warning.is_some() && rec.steps[1].step.screenshot.is_none());
    assert!(rec.steps[2].image_warning.is_some() && rec.steps[2].step.screenshot.is_none());
    doc.close();
}

/// A cancelled import removes the images it had stored.
#[test]
fn cancelled_import_rolls_back_its_images() {
    let scratch = Scratch::new("rollback");
    let beta = beta(&scratch.0);
    let doc = new_doc(&scratch);
    let rec = import_recording(&beta, doc.assets(), &ImportLimits::default()).unwrap();
    assert_eq!(rec.created_assets.len(), 2);
    rec.rollback(doc.assets());
    for a in &rec.created_assets {
        assert!(!doc.assets().contains(a));
    }
    doc.close();
}

/// A20 (core side): merging four recordings stays consistent.
#[test]
fn four_recordings_into_one_guide() {
    let scratch = Scratch::new("four");
    let mut doc = new_doc(&scratch);
    let limits = ImportLimits::default();
    for i in 0..4u8 {
        let steps = serde_json::to_vec(&vec![
            legacy_click(1, &format!("R{i} a"), Some("step-0001.webp")),
            legacy_click(2, &format!("R{i} b"), Some("step-0002.webp")),
        ])
        .unwrap();
        let p = scratch.path(&format!("r{i}.zip"));
        write_zip(
            &p,
            &[
                ("steps.json", &steps),
                ("step-0001.webp", &webp(16, 16, [i * 50, 0, 0])),
                ("step-0002.webp", &webp(16, 16, [0, i * 50, 0])),
            ],
        );
        let rec = import_recording(&p, doc.assets(), &limits).unwrap();
        doc.import_steps(doc.steps().len(), &rec, &[0, 1]).unwrap();
    }
    assert_eq!(doc.steps().len(), 8);
    let last = doc.steps()[7].id.unwrap();
    doc.move_step(last, 0).unwrap();
    assert!(doc.steps()[0].display_title(Lang::En).contains("R3 b"));
    doc.close();
}
