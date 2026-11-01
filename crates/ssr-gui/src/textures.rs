//! Screenshot textures: the full image of the selected step and small
//! thumbnails for the list, decoded lazily and cached by file name.

use std::collections::HashMap;
use std::path::Path;

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// Width of the thumbnails, in pixels (enough for a HiDPI list row).
const THUMB_WIDTH: u32 = 240;
/// Screenshots decoded per frame at most, so a long session scrolled into
/// view does not freeze the GUI: the rest come on the next frames.
const DECODES_PER_FRAME: usize = 3;

#[derive(Default)]
pub struct Textures {
    full: HashMap<String, TextureHandle>,
    thumbs: HashMap<String, TextureHandle>,
    budget: usize,
}

impl Textures {
    /// Call once per frame before any lookup.
    pub fn begin_frame(&mut self) {
        self.budget = DECODES_PER_FRAME;
    }

    pub fn clear(&mut self) {
        self.full.clear();
        self.thumbs.clear();
    }

    /// The full-size screenshot `name` of the session in `dir`.
    pub fn full(&mut self, ctx: &Context, dir: &Path, name: &str) -> Option<TextureHandle> {
        if let Some(t) = self.full.get(name) {
            return Some(t.clone());
        }
        // The selected step always gets decoded, budget or not.
        let img = image::open(dir.join(name)).ok()?.to_rgba8();
        let handle = upload(ctx, name, &img);
        self.full.insert(name.to_string(), handle.clone());
        Some(handle)
    }

    /// A thumbnail of `name`; `None` while it is not decoded yet (the file
    /// may still be written by the recorder, or the frame budget is spent).
    pub fn thumb(&mut self, ctx: &Context, dir: &Path, name: &str) -> Option<TextureHandle> {
        if let Some(t) = self.thumbs.get(name) {
            return Some(t.clone());
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return None;
        }
        self.budget -= 1;
        let img = image::open(dir.join(name)).ok()?.to_rgba8();
        let (w, h) = (img.width().max(1), img.height().max(1));
        let small = if w > THUMB_WIDTH {
            let th = ((h as u64 * THUMB_WIDTH as u64) / w as u64).max(1) as u32;
            image::imageops::thumbnail(&img, THUMB_WIDTH, th)
        } else {
            img
        };
        let handle = upload(ctx, &format!("thumb:{name}"), &small);
        self.thumbs.insert(name.to_string(), handle.clone());
        Some(handle)
    }
}

fn upload(ctx: &Context, name: &str, img: &image::RgbaImage) -> TextureHandle {
    let size = [img.width() as usize, img.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture(name, color, TextureOptions::LINEAR)
}
