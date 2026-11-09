//! Shared helpers: synthetic v0.2.0-style recordings and scratch folders.
#![allow(dead_code)]

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use ssr_core::editor::EditorDocument;
use ssr_core::workspace::Workspace;
use ssr_core::Lang;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// A fresh scratch folder (removed by [`Scratch`]'s drop).
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Self {
        let p =
            std::env::temp_dir().join(format!("ssr-it-{tag}-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(w, h, rgb, image::ImageFormat::Png)
}

pub fn webp(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(w, h, rgb, image::ImageFormat::WebP)
}

fn encode(w: u32, h: u32, rgb: [u8; 3], fmt: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        // A little pattern so different colours give different bytes.
        let v = ((x + y) % 7) as u8;
        image::Rgba([rgb[0].saturating_add(v), rgb[1], rgb[2], 255])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, fmt).unwrap();
    out.into_inner()
}

/// One `steps.json` entry exactly as v0.2.0 wrote it (no editor fields).
pub fn legacy_click(index: usize, title: &str, screenshot: Option<&str>) -> serde_json::Value {
    let mut v = serde_json::json!({
        "index": index,
        "timestamp_ms": 1_761_990_000_000u64 + index as u64 * 1000,
        "elapsed_ms": index as u64 * 1000,
        "action": { "type": "click", "button": "left", "x": 10, "y": 20, "rel_x": 5, "rel_y": 6 },
        "window": { "title": title, "app_name": "demo", "pid": 42, "x": 5, "y": 14, "width": 800, "height": 600 },
        "description": format!("Left click on “{title}” (demo)")
    });
    if let Some(s) = screenshot {
        v["screenshot"] = serde_json::Value::String(s.into());
    }
    v
}

pub fn legacy_text(index: usize, content: &str) -> serde_json::Value {
    serde_json::json!({
        "index": index,
        "timestamp_ms": 1_761_990_000_000u64 + index as u64 * 1000,
        "elapsed_ms": index as u64 * 1000,
        "action": { "type": "text", "content": content },
        "description": format!("Typed “{content}” in the active window")
    })
}

/// Write a ZIP with the given members, in order.
pub fn write_zip(path: &Path, members: &[(&str, &[u8])]) {
    let f = fs::File::create(path).unwrap();
    let mut z = ZipWriter::new(f);
    let opts = SimpleFileOptions::default();
    for (name, data) in members {
        z.start_file(*name, opts).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}

/// "Alpha": a click with `step-0001.webp` (reddish) then a typing step.
pub fn alpha(dir: &Path) -> PathBuf {
    let steps = serde_json::to_vec_pretty(&vec![
        legacy_click(1, "Settings", Some("step-0001.webp")),
        legacy_text(2, "hunter2"),
    ])
    .unwrap();
    let img = webp(64, 40, [200, 30, 30]);
    let path = dir.join("recording-alpha.zip");
    write_zip(
        &path,
        &[
            ("report.html", b"<script>alert('never run')</script>"),
            ("steps.json", &steps),
            ("step-0001.webp", &img),
        ],
    );
    path
}

/// "Beta": two clicks; its `step-0001.webp` differs from Alpha's.
pub fn beta(dir: &Path) -> PathBuf {
    let steps = serde_json::to_vec_pretty(&vec![
        legacy_click(1, "Network", Some("step-0001.webp")),
        legacy_click(2, "Apply", Some("step-0002.webp")),
    ])
    .unwrap();
    let a = webp(64, 40, [20, 30, 200]);
    let b = webp(64, 40, [20, 200, 30]);
    let path = dir.join("recording-beta.zip");
    write_zip(
        &path,
        &[
            ("report.html", b"<html></html>"),
            ("steps.json", &steps),
            ("step-0001.webp", &a),
            ("step-0002.webp", &b),
        ],
    );
    path
}

/// An empty guide whose workspace lives under `scratch`.
pub fn new_doc(scratch: &Scratch) -> EditorDocument {
    let ws = Workspace::create_in(&scratch.path("workspaces")).unwrap();
    EditorDocument::new(ws, "Test guide", Lang::En).unwrap()
}

pub fn workspace(scratch: &Scratch) -> Workspace {
    Workspace::create_in(&scratch.path("workspaces")).unwrap()
}

/// Member names of a ZIP.
pub fn zip_names(path: &Path) -> Vec<String> {
    let f = fs::File::open(path).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().unwrap().to_string())
        .collect()
}

pub fn zip_member(path: &Path, name: &str) -> Vec<u8> {
    let f = fs::File::open(path).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let mut e = z.by_name(name).unwrap();
    let mut out = Vec::new();
    e.read_to_end(&mut out).unwrap();
    out
}

pub fn sha(path: &Path) -> String {
    ssr_core::assets::sha256_file(path).unwrap()
}
