//! Hostile archives: every one must be refused (or neutralised) before
//! anything is written outside the guide's workspace.

mod common;

use std::io::Write;

use common::*;
use ssr_core::import::{import_recording, ImportError, ImportLimits};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn steps_naming(name: &str) -> Vec<u8> {
    serde_json::to_vec(&vec![legacy_click(1, "x", Some(name))]).unwrap()
}

fn import(scratch: &Scratch, path: &std::path::Path) -> Result<usize, ImportError> {
    import_with(scratch, path, &ImportLimits::default())
}

fn import_with(
    scratch: &Scratch,
    path: &std::path::Path,
    limits: &ImportLimits,
) -> Result<usize, ImportError> {
    let doc = new_doc(scratch);
    let r = import_recording(path, doc.assets(), limits).map(|r| r.steps.len());
    doc.close();
    r
}

#[test]
fn manifest_pointing_outside_is_refused() {
    let scratch = Scratch::new("trav");
    for name in [
        "../../etc/passwd",
        "/etc/shadow",
        "..\\evil.webp",
        "C:/x.webp",
    ] {
        let p = scratch.path("t.zip");
        write_zip(&p, &[("steps.json", &steps_naming(name))]);
        let err = import(&scratch, &p).unwrap_err();
        assert!(matches!(err, ImportError::UnsafePath(_)), "{name}: {err:?}");
    }
}

#[test]
fn member_with_traversal_name_is_refused() {
    let scratch = Scratch::new("member-trav");
    let p = scratch.path("t.zip");
    write_zip(&p, &[("steps.json", b"[]"), ("..\\evil.webp", b"x")]);
    assert!(matches!(
        import(&scratch, &p),
        Err(ImportError::UnsafePath(_))
    ));
    let p2 = scratch.path("t2.zip");
    write_zip(&p2, &[("steps.json", b"[]"), ("../evil.webp", b"x")]);
    assert!(matches!(
        import(&scratch, &p2),
        Err(ImportError::UnsafePath(_))
    ));
}

#[test]
fn symlink_member_is_refused() {
    let scratch = Scratch::new("symlink");
    let p = scratch.path("t.zip");
    let f = std::fs::File::create(&p).unwrap();
    let mut z = ZipWriter::new(f);
    z.start_file("steps.json", SimpleFileOptions::default())
        .unwrap();
    z.write_all(&steps_naming("step-0001.webp")).unwrap();
    z.add_symlink(
        "step-0001.webp",
        "/etc/passwd",
        SimpleFileOptions::default(),
    )
    .unwrap();
    z.finish().unwrap();
    assert!(matches!(import(&scratch, &p), Err(ImportError::Symlink(_))));
}

#[test]
fn duplicate_entries_are_refused() {
    let scratch = Scratch::new("dup");
    // The zip writer refuses duplicates, so build the archive by hand: two
    // stored entries with the same name.
    let p = scratch.path("dup.zip");
    std::fs::write(&p, two_entries_same_name("steps.json", b"[]", b"[1]")).unwrap();
    let err = import(&scratch, &p).unwrap_err();
    assert!(
        matches!(
            err,
            ImportError::DuplicateEntry(_) | ImportError::NotAnArchive(_)
        ),
        "{err:?}"
    );
}

#[test]
fn malformed_or_missing_manifest() {
    let scratch = Scratch::new("manifest");
    let p = scratch.path("m.zip");
    write_zip(&p, &[("steps.json", b"{not json")]);
    assert!(matches!(
        import(&scratch, &p),
        Err(ImportError::BadManifest(_))
    ));
    write_zip(&p, &[("report.html", b"<html>")]);
    assert!(matches!(
        import(&scratch, &p),
        Err(ImportError::MissingManifest)
    ));
    std::fs::write(&p, b"this is no zip").unwrap();
    assert!(matches!(
        import(&scratch, &p),
        Err(ImportError::NotAnArchive(_))
    ));
}

#[test]
fn budgets_are_enforced() {
    let scratch = Scratch::new("budget");
    let p = scratch.path("b.zip");
    let many: Vec<_> = (1..=20).map(|i| legacy_text(i, "x")).collect();
    write_zip(&p, &[("steps.json", &serde_json::to_vec(&many).unwrap())]);
    let tight = ImportLimits {
        max_steps: 10,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_with(&scratch, &p, &tight),
        Err(ImportError::TooManySteps(20))
    ));

    // An oversized manifest (highly compressible: a zip bomb in miniature).
    let big = vec![b' '; 2 * 1024 * 1024];
    write_zip(&p, &[("steps.json", &big)]);
    let small_json = ImportLimits {
        max_json_bytes: 1024 * 1024,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_with(&scratch, &p, &small_json),
        Err(ImportError::TooLarge(_))
    ));
    let small_total = ImportLimits {
        max_total_uncompressed: 1024,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_with(&scratch, &p, &small_total),
        Err(ImportError::TooLarge(_))
    ));

    let entries: Vec<(String, Vec<u8>)> = (0..30).map(|i| (format!("f{i}"), vec![0u8])).collect();
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    write_zip(&p, &refs);
    let few = ImportLimits {
        max_entries: 10,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_with(&scratch, &p, &few),
        Err(ImportError::TooManyEntries(30))
    ));
}

#[test]
fn huge_declared_dimensions_are_refused_before_decoding() {
    let scratch = Scratch::new("bomb");
    // A valid PNG header announcing 100 000 × 100 000 pixels.
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend_from_slice(&(ihdr.len() as u32).to_be_bytes());
    let mut chunk = b"IHDR".to_vec();
    chunk.extend_from_slice(&ihdr);
    png.extend_from_slice(&chunk);
    png.extend_from_slice(&crc32(&chunk).to_be_bytes());
    for (kind, data) in [
        (&b"IDAT"[..], &b"\x78\x9c\x03\x00\x00\x00\x00\x01"[..]),
        (b"IEND", b""),
    ] {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        png.extend_from_slice(&c);
        png.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let p = scratch.path("bomb.zip");
    write_zip(
        &p,
        &[
            ("steps.json", &steps_naming("step-0001.png")),
            ("step-0001.png", &png),
        ],
    );
    let doc = new_doc(&scratch);
    let rec = import_recording(&p, doc.assets(), &ImportLimits::default()).unwrap();
    let w = rec.steps[0].image_warning.as_deref().unwrap();
    assert!(w.contains("too large"), "{w}");
    assert!(rec.steps[0].step.screenshot.is_none());
    doc.close();
}

#[test]
fn report_html_is_never_read() {
    let scratch = Scratch::new("html");
    let p = alpha(&scratch.0); // its report.html holds a script
    let doc = new_doc(&scratch);
    import_recording(&p, doc.assets(), &ImportLimits::default()).unwrap();
    let mut stored = Vec::new();
    for e in std::fs::read_dir(doc.asset_root().join("assets"))
        .unwrap()
        .flatten()
    {
        stored.push(std::fs::read(e.path()).unwrap());
    }
    assert!(stored.iter().all(|b| !b.windows(6).any(|w| w == b"script")));
    doc.close();
}

#[test]
fn import_from_read_only_location_leaves_source_alone() {
    let scratch = Scratch::new("ro");
    let p = beta(&scratch.0);
    let before = sha(&p);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o444)).unwrap();
    }
    assert_eq!(import(&scratch, &p).unwrap(), 2);
    assert_eq!(sha(&p), before);
    let siblings = std::fs::read_dir(&scratch.0).unwrap().count();
    assert_eq!(siblings, 2, "only the source and the workspaces folder");
}

// ── Helpers ────────────────────────────────────────────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// A minimal ZIP (stored entries) with two members of the same name.
fn two_entries_same_name(name: &str, a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for data in [a, b] {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        // Central directory record.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 2, 0, 2, 0]);
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}
