//! The building blocks of every screen, drawn from the active palette.
//!
//! Custom widgets register a [`egui::WidgetInfo`] so that accessibility tools
//! (and egui_mcp, with the `inspection` feature) see them as real buttons.

use egui::{
    Color32, CornerRadius, FontId, Frame, Margin, Response, RichText, Role, Sense, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo,
};

use crate::icons::Icon;
use crate::theme::{self, pal};

/// The weight of a button.
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// The main action of a screen (accent fill).
    Primary,
    /// Start recording (record-red fill).
    Record,
    /// A secondary action (raised fill, bordered).
    Secondary,
    /// A quiet action (no fill until hovered).
    Ghost,
}

/// A button with an optional painted icon in front of its label.
pub fn button(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str) -> Response {
    draw_button(ui, kind, icon, text, false)
}

/// The same, larger: the one obvious action of a screen.
pub fn big_button(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str) -> Response {
    draw_button(ui, kind, icon, text, true)
}

fn draw_button(ui: &mut Ui, kind: Kind, icon: Option<Icon>, text: &str, large: bool) -> Response {
    let p = pal(ui.ctx());
    let enabled = ui.is_enabled();
    let font = FontId::proportional(if large { 16.0 } else { theme::SIZE_BODY });
    let (fill, fg, border) = match kind {
        Kind::Primary => (p.accent, p.on_accent, Color32::TRANSPARENT),
        Kind::Record => (p.record, p.on_accent, Color32::TRANSPARENT),
        Kind::Secondary => (p.raised, p.text, p.border),
        Kind::Ghost => (Color32::TRANSPARENT, p.text_muted, Color32::TRANSPARENT),
    };
    let fg = if enabled { fg } else { p.text_faint };

    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), fg);
    let pad = if large {
        Vec2::new(22.0, 12.0)
    } else {
        Vec2::new(12.0, 5.0)
    };
    let icon_size = if large { 16.0 } else { 14.0 };
    let gap = if icon.is_some() { 8.0 } else { 0.0 };
    let icon_w = if icon.is_some() { icon_size } else { 0.0 };
    let size = Vec2::new(
        pad.x * 2.0 + icon_w + gap + galley.size().x,
        (galley.size().y.max(icon_size) + pad.y * 2.0).max(ui.spacing().interact_size.y),
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let fill = match kind {
            _ if !enabled => p.raised,
            Kind::Ghost if hovered => p.raised,
            Kind::Secondary if hovered => p.tint(p.accent, 30),
            _ if pressed => fill.linear_multiply(0.85),
            _ if hovered => fill.lerp_to_gamma(Color32::WHITE, 0.10),
            _ => fill,
        };
        let border = if hovered && kind == Kind::Secondary {
            p.accent
        } else {
            border
        };
        let radius = if large {
            theme::RADIUS
        } else {
            theme::RADIUS_SMALL
        };
        painter.rect(
            rect,
            CornerRadius::same(radius),
            fill,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        let mut x = rect.left() + pad.x;
        if let Some(icon) = icon {
            let r = egui::Rect::from_center_size(
                egui::pos2(x + icon_size / 2.0, rect.center().y),
                Vec2::splat(icon_size),
            );
            icon.paint(painter, r, fg);
            x += icon_size + gap;
        }
        let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A borderless square button showing an icon, with a tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let p = pal(ui.ctx());
    let size = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let enabled = ui.is_enabled();
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, tip));
    if enabled && resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(theme::RADIUS_SMALL), p.raised);
    }
    let color = if enabled { p.text_muted } else { p.text_faint };
    icon.paint(ui.painter(), rect.shrink(size * 0.24), color);
    resp.on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One segment of a [`segmented`] control.
pub struct Seg<'a> {
    pub icon: Option<Icon>,
    pub label: Option<&'a str>,
    pub tip: &'a str,
}

/// A row of mutually exclusive choices; returns the index clicked, if any.
pub fn segmented(ui: &mut Ui, segs: &[Seg<'_>], selected: usize) -> Option<usize> {
    let p = pal(ui.ctx());
    let h = ui.spacing().interact_size.y;
    let font = FontId::proportional(theme::SIZE_SMALL + 0.5);
    let widths: Vec<f32> = segs
        .iter()
        .map(|s| {
            let text_w = s.label.map_or(0.0, |l| {
                ui.painter()
                    .layout_no_wrap(l.to_owned(), font.clone(), p.text)
                    .size()
                    .x
            });
            match (s.icon, s.label) {
                (Some(_), Some(_)) => 14.0 + 6.0 + text_w + 18.0,
                (Some(_), None) => h,
                _ => text_w + 20.0,
            }
        })
        .collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(theme::RADIUS_SMALL),
        p.raised,
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );

    let mut clicked = None;
    let mut x = rect.left() + 2.0;
    for (i, (seg, w)) in segs.iter().zip(&widths).enumerate() {
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), Vec2::new(*w, h - 4.0));
        x += w;
        let id = ui.scope_id().with(("seg", seg.tip, i));
        let resp = ui.interact(r, id, Sense::click());
        let is_sel = i == selected;
        resp.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, is_sel, seg.tip));
        let painter = ui.painter();
        if is_sel {
            painter.rect(
                r,
                CornerRadius::same(theme::RADIUS_SMALL - 2),
                p.surface,
                Stroke::new(1.0, p.border),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            painter.rect_filled(
                r,
                CornerRadius::same(theme::RADIUS_SMALL - 2),
                p.tint(p.accent, 24),
            );
        }
        let color = if is_sel { p.accent_text } else { p.text_muted };
        let label_galley = seg
            .label
            .map(|l| painter.layout_no_wrap(l.to_owned(), font.clone(), color));
        match (seg.icon, label_galley) {
            (Some(icon), Some(g)) => {
                let content = 14.0 + 6.0 + g.size().x;
                let left = r.center().x - content / 2.0;
                let ir = egui::Rect::from_center_size(
                    egui::pos2(left + 7.0, r.center().y),
                    Vec2::splat(14.0),
                );
                icon.paint(painter, ir, color);
                painter.galley(
                    egui::pos2(left + 20.0, r.center().y - g.size().y / 2.0),
                    g,
                    color,
                );
            }
            (Some(icon), None) => {
                icon.paint(painter, r.shrink(r.height() * 0.22), color);
            }
            (None, Some(g)) => {
                painter.galley(r.center() - g.size() / 2.0, g, color);
            }
            (None, None) => {}
        }
        let resp = resp
            .on_hover_text(seg.tip)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// A surface with a border and rounded corners.
pub fn card(ui: &Ui) -> Frame {
    let p = pal(ui.ctx());
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(16))
}

/// A small rounded label tinted by `color`.
pub fn pill(ui: &mut Ui, icon: Option<Icon>, text: &str, color: Color32) -> Response {
    let p = pal(ui.ctx());
    let font = FontId::proportional(theme::SIZE_SMALL);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let icon_w = if icon.is_some() { 12.0 + 5.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + icon_w + 18.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(11), p.tint(color, 38));
    let mut x = rect.left() + 9.0;
    if let Some(icon) = icon {
        let r =
            egui::Rect::from_center_size(egui::pos2(x + 6.0, rect.center().y), Vec2::splat(12.0));
        icon.paint(painter, r, color);
        x += icon_w;
    }
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    resp
}

/// A small caps label above a group.
pub fn eyebrow(ui: &mut Ui, text: &str) {
    let p = pal(ui.ctx());
    ui.label(
        RichText::new(text.to_uppercase())
            .size(theme::SIZE_EYEBROW)
            .color(p.text_faint)
            .extra_letter_spacing(1.0)
            .strong(),
    );
}

/// A dot blinking once a second: the live recording indicator. It steps with
/// the recording's one-second refresh instead of animating, which would keep
/// redrawing the window for nothing.
pub fn live_dot(ui: &mut Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let pulse = if (ui.input(|i| i.time) as i64) % 2 == 0 {
        1.0
    } else {
        0.0
    };
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        size * (0.32 + 0.16 * pulse),
        color.gamma_multiply(0.25 * (1.0 - pulse) + 0.1),
    );
    painter.circle_filled(rect.center(), size * 0.26, color);
}
