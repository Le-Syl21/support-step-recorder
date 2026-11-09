//! Build sample data with dummy content: two v0.2.0-style recordings
//! (`recording-alpha.zip`, `recording-beta.zip`) and the guide merged and
//! edited from them (`master.ssrproj` + `master.zip`).
//!
//! ```bash
//! cargo run -p support-step-recorder-core --example sample_guide -- out/
//! ```

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use ssr_core::editor::EditorDocument;
use ssr_core::import::{import_recording, ImportLimits};
use ssr_core::workspace::Workspace;
use ssr_core::{export, Lang, Progress};
use zip::write::SimpleFileOptions;

/// A fake application window: title bar, side bar, a few "fields" and a
/// highlighted button.
fn window(accent: [u8; 3], highlight: (u32, u32)) -> Vec<u8> {
    let (w, h) = (640u32, 400u32);
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let px = |c: [u8; 3]| image::Rgba([c[0], c[1], c[2], 255]);
        if y < 32 {
            return px(accent);
        }
        if x < 150 {
            return px([236, 238, 242]);
        }
        let (hx, hy) = highlight;
        if x >= hx && x < hx + 120 && y >= hy && y < hy + 32 {
            return px([
                accent[0] / 2 + 100,
                accent[1] / 2 + 100,
                accent[2] / 2 + 100,
            ]);
        }
        for row in 0..4u32 {
            let top = 70 + row * 60;
            if (top..top + 26).contains(&y) && (180..560).contains(&x) {
                return if y == top || y == top + 25 || x == 180 || x == 559 {
                    px([180, 184, 192])
                } else {
                    px([255, 255, 255])
                };
            }
        }
        px([248, 249, 251])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::WebP).unwrap();
    out.into_inner()
}

fn click(index: usize, title: &str, app: &str, shot: &str) -> serde_json::Value {
    serde_json::json!({
        "index": index,
        "timestamp_ms": 1_762_000_000_000u64 + index as u64 * 4_000,
        "elapsed_ms": index as u64 * 4_000,
        "action": { "type": "click", "button": "left", "x": 400, "y": 300, "rel_x": 220, "rel_y": 140 },
        "window": { "title": title, "app_name": app, "pid": 4242, "x": 180, "y": 160, "width": 640, "height": 400 },
        "screenshot": shot,
        "description": format!("Left click on “{title}” ({app})")
    })
}

fn typed(index: usize, content: &str, title: &str, app: &str) -> serde_json::Value {
    serde_json::json!({
        "index": index,
        "timestamp_ms": 1_762_000_000_000u64 + index as u64 * 4_000,
        "elapsed_ms": index as u64 * 4_000,
        "action": { "type": "text", "content": content },
        "window": { "title": title, "app_name": app, "pid": 4242, "x": 180, "y": 160, "width": 640, "height": 400 },
        "description": format!("Typed “{content}” in “{title}” ({app})")
    })
}

fn write_zip(path: &Path, members: &[(&str, Vec<u8>)]) {
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, data) in members {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "samples".into()));
    fs::create_dir_all(&out).unwrap();

    let alpha = out.join("recording-alpha.zip");
    write_zip(
        &alpha,
        &[
            (
                "report.html",
                b"<!-- legacy report, ignored on import -->".to_vec(),
            ),
            (
                "steps.json",
                serde_json::to_vec_pretty(&vec![
                    click(1, "Settings", "example-settings", "step-0001.webp"),
                    typed(2, "printer-lab-2", "Settings", "example-settings"),
                    click(
                        3,
                        "Settings — Printers",
                        "example-settings",
                        "step-0002.webp",
                    ),
                ])
                .unwrap(),
            ),
            ("step-0001.webp", window([52, 101, 164], (180, 330))),
            ("step-0002.webp", window([52, 101, 164], (440, 330))),
        ],
    );
    let beta = out.join("recording-beta.zip");
    write_zip(
        &beta,
        &[
            ("report.html", b"<!-- legacy report -->".to_vec()),
            (
                "steps.json",
                serde_json::to_vec_pretty(&vec![
                    click(1, "Add Printer", "example-settings", "step-0001.webp"),
                    click(
                        2,
                        "Add Printer — Confirm",
                        "example-settings",
                        "step-0002.webp",
                    ),
                ])
                .unwrap(),
            ),
            // Same file names as Alpha, different pictures.
            ("step-0001.webp", window([46, 125, 50], (180, 330))),
            ("step-0002.webp", window([46, 125, 50], (440, 330))),
        ],
    );

    let base = out.join(".workspaces");
    let ws = Workspace::create_in(&base).unwrap();
    let mut doc = EditorDocument::new(ws, "Add a network printer", Lang::En).unwrap();
    let limits = ImportLimits::default();
    let a = import_recording(&alpha, doc.assets(), &limits).unwrap();
    let b = import_recording(&beta, doc.assets(), &limits).unwrap();
    doc.import_many(0, &[(&a, &[0, 1, 2][..]), (&b, &[0, 1][..])])
        .unwrap();
    doc.set_guide_info(
        "Add a network printer".into(),
        "Sample guide built from two separate recordings. All content is dummy data.".into(),
    )
    .unwrap();
    let ids: Vec<_> = doc.steps().iter().map(|s| s.id.unwrap()).collect();
    doc.set_title(ids[0], Some("Open Settings".into())).unwrap();
    doc.set_notes(ids[0], "Use the gear icon in the top bar.".into())
        .unwrap();
    doc.set_title(ids[1], Some("Type the printer name".into()))
        .unwrap();
    doc.set_notes(ids[1], "Ask the help desk for the exact name.".into())
        .unwrap();
    doc.set_title(ids[2], Some("Open the Printers page".into()))
        .unwrap();
    doc.set_title(ids[3], Some("Click “Add Printer”".into()))
        .unwrap();
    doc.set_title(ids[4], Some("Confirm".into())).unwrap();
    doc.set_notes(
        ids[4],
        "The printer appears in the list after a few seconds.".into(),
    )
    .unwrap();
    doc.add_manual_step(5, "Print a test page", None).unwrap();

    doc.save_project(&out.join("master.ssrproj")).unwrap();
    export::export_guide(
        &doc.snapshot(),
        &out.join("master.zip"),
        &Progress::new(),
        Lang::En,
    )
    .unwrap();
    doc.close();
    let _ = fs::remove_dir_all(&base);
    println!("Written to {}", out.display());
}
