//! Screenshot viewer: fit to view by default, wheel (or pinch) zooms around
//! the pointer, dragging pans, double-click toggles fit / actual pixels.

use egui::{
    pos2, vec2, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2,
};

use crate::icons::Icon;
use crate::theme::{self, pal};
use crate::widgets;

/// Zoom relative to "fit", at most this many times larger.
const MAX_ZOOM: f32 = 12.0;

#[derive(Clone, Debug)]
pub struct Viewer {
    /// The texture this state belongs to; another one resets the view.
    for_texture: Option<egui::TextureId>,
    /// Multiplier of the fit scale (1 = fit to view).
    zoom: f32,
    /// Offset of the image centre from the canvas centre, in points.
    pan: Vec2,
    /// How far "fit" may enlarge a small screenshot (1 = never upscale).
    max_fit: f32,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            for_texture: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            max_fit: 1.0,
        }
    }
}

/// What the toolbar asked for.
#[derive(Default)]
pub struct ToolbarActions {
    pub enlarge: bool,
}

impl Viewer {
    /// A viewer whose "fit" also enlarges small screenshots (the full-window view).
    pub fn enlarging() -> Self {
        Self {
            max_fit: 3.0,
            ..Self::default()
        }
    }

    pub fn fit(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    /// The zoom/fit toolbar, then the canvas filling the rest of `ui`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tex: &TextureHandle,
        lang: ssr_core::Lang,
        can_enlarge: bool,
    ) -> ToolbarActions {
        let mut actions = ToolbarActions::default();
        if self.for_texture != Some(tex.id()) {
            *self = Self {
                for_texture: Some(tex.id()),
                max_fit: self.max_fit,
                ..Self::default()
            };
        }

        // The canvas size is needed for the percentage, so measure it first.
        let toolbar_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        let canvas_size = ui.available_size() - vec2(0.0, toolbar_h);
        let fit = fit_scale(tex.size_vec2(), canvas_size, self.max_fit);
        let ppp = ui.ctx().pixels_per_point();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if widgets::icon_button(ui, Icon::Minus, lang.pick("Dézoomer", "Zoom out")).clicked() {
                self.zoom_by(1.0 / 1.25);
            }
            let pct = format!("{:.0} %", fit * self.zoom * ppp * 100.0);
            let p = pal(ui.ctx());
            ui.add_sized(
                vec2(56.0, ui.spacing().interact_size.y),
                egui::Label::new(widgets_text(&pct, p.text_muted)),
            )
            .on_hover_text(lang.pick(
                "Taille réelle de la capture, en pixels",
                "Size relative to the screenshot's actual pixels",
            ));
            if widgets::icon_button(ui, Icon::Plus, lang.pick("Zoomer", "Zoom in")).clicked() {
                self.zoom_by(1.25);
            }
            ui.add_space(6.0);
            if widgets::icon_button(ui, Icon::Fit, lang.pick("Ajuster à la vue", "Fit to view"))
                .clicked()
            {
                self.fit();
            }
            if can_enlarge
                && widgets::icon_button(
                    ui,
                    Icon::Expand,
                    lang.pick("Agrandir (Échap pour fermer)", "Enlarge (Esc to close)"),
                )
                .clicked()
            {
                actions.enlarge = true;
            }
            ui.add_space(8.0);
            let p = pal(ui.ctx());
            ui.add(
                egui::Label::new(
                    egui::RichText::new(lang.pick(
                        "Molette : zoom · glisser : déplacer · double-clic : 1:1",
                        "Wheel: zoom · drag: pan · double-click: 1:1",
                    ))
                    .size(theme::SIZE_SMALL)
                    .color(p.text_faint),
                )
                .truncate(),
            );
        });

        let size = ui.available_size().max(vec2(40.0, 40.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.canvas(ui, rect, &resp, tex);
        actions
    }

    /// Paint the image in `rect` and react to the pointer on `resp`.
    pub fn canvas(&mut self, ui: &Ui, rect: Rect, resp: &Response, tex: &TextureHandle) {
        let p = pal(ui.ctx());
        let img = tex.size_vec2();
        let fit = fit_scale(img, rect.size(), self.max_fit);
        let ppp = ui.ctx().pixels_per_point();

        // Zoom around the pointer: the image point under it stays put.
        if resp.hovered() {
            let (scroll, pinch, pointer) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                )
            });
            let factor = (scroll * 0.0025).exp() * pinch;
            if (factor - 1.0).abs() > 1e-4 {
                if let Some(at) = pointer {
                    self.zoom_at(factor, at, rect, fit);
                }
            }
        }
        if resp.dragged() {
            self.pan += resp.drag_delta();
        }
        if resp.double_clicked() {
            let actual = 1.0 / (fit * ppp);
            if (self.zoom - 1.0).abs() < 1e-3 && (actual - 1.0).abs() > 1e-3 {
                if let Some(at) = resp.interact_pointer_pos() {
                    self.zoom_at(actual / self.zoom, at, rect, fit);
                }
            } else {
                self.fit();
            }
        }

        let scale = fit * self.zoom;
        let shown = img * scale;
        self.clamp_pan(shown, rect.size());
        let image_rect = Rect::from_center_size(rect.center() + self.pan, shown);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(theme::RADIUS_SMALL), p.canvas);
        painter.image(
            tex.id(),
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        painter.rect_stroke(
            image_rect,
            0.0,
            Stroke::new(1.0, p.border),
            StrokeKind::Outside,
        );
        if self.zoom > 1.0 + 1e-3 {
            resp.clone().on_hover_cursor(if resp.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        let z = (self.zoom * factor).clamp(1.0 / MAX_ZOOM, MAX_ZOOM);
        self.pan *= z / self.zoom;
        self.zoom = z;
    }

    fn zoom_at(&mut self, factor: f32, at: egui::Pos2, rect: Rect, fit: f32) {
        let old = fit * self.zoom;
        let z = (self.zoom * factor).clamp(1.0 / MAX_ZOOM, MAX_ZOOM);
        let new = fit * z;
        let center = rect.center() + self.pan;
        let image_point = (at - center) / old;
        let new_center = at - image_point * new;
        self.pan = new_center - rect.center();
        self.zoom = z;
    }

    /// Keep the image on screen: centred when smaller than the canvas, its
    /// edges no further in than the canvas edges otherwise.
    fn clamp_pan(&mut self, shown: Vec2, canvas: Vec2) {
        let limit = ((shown - canvas) / 2.0).max(Vec2::ZERO);
        self.pan = self.pan.clamp(-limit, limit);
    }
}

/// The scale that fits `img` in `canvas`, at most `max`.
fn fit_scale(img: Vec2, canvas: Vec2, max: f32) -> f32 {
    if img.x <= 0.0 || img.y <= 0.0 {
        return 1.0;
    }
    (canvas.x / img.x)
        .min(canvas.y / img.y)
        .clamp(0.01, max.max(0.01))
}

fn widgets_text(text: &str, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(text)
        .size(theme::SIZE_SMALL)
        .color(color)
        .monospace()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_respects_its_cap() {
        assert_eq!(fit_scale(vec2(100.0, 50.0), vec2(800.0, 600.0), 1.0), 1.0);
        assert_eq!(fit_scale(vec2(1600.0, 800.0), vec2(800.0, 600.0), 1.0), 0.5);
        assert_eq!(fit_scale(vec2(100.0, 100.0), vec2(800.0, 600.0), 3.0), 3.0);
    }

    #[test]
    fn zoom_at_keeps_the_point_under_the_pointer() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let mut v = Viewer::default();
        let at = pos2(600.0, 200.0);
        let fit = 0.5;
        let before = (at - (rect.center() + v.pan)) / (fit * v.zoom);
        v.zoom_at(2.0, at, rect, fit);
        let after = (at - (rect.center() + v.pan)) / (fit * v.zoom);
        assert!((before - after).length() < 1e-3);
    }
}
