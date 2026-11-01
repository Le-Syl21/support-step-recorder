//! The steps of a session: the list with thumbnails on the left, the
//! selected step on the right (its screenshot and its metadata). Shared by
//! the recording (live, following the latest step) and review stages.

use std::sync::Arc;

use eframe::egui;
use egui::{
    text::LayoutJob, Align, CornerRadius, FontId, Galley, Margin, Rect, RichText, Role, Sense,
    Stroke, TextFormat, Vec2, WidgetInfo,
};
use ssr_core::{Action, Lang};

use crate::app::{App, Stage};
use crate::icons::{self, Icon};
use crate::steps::{self, Snapshot, StepRow};
use crate::theme::{self, pal};
use crate::viewer::Viewer;
use crate::widgets;

const ROW_HEIGHT: f32 = 68.0;
/// Thumbnail size in a list of normal width; narrower lists shrink it.
const THUMB: Vec2 = Vec2::new(88.0, 54.0);
/// Below this central width, the metadata goes above the screenshot.
const SIDE_META_MIN_WIDTH: f32 = 760.0;

pub fn body(app: &mut App, root: &mut egui::Ui, nav: bool) {
    let p = pal(root.ctx());
    let snapshot = app.snapshot.clone();

    // The list never takes more than about 40 % of a narrow window.
    let max_list = (root.available_width() * 0.4).clamp(200.0, 480.0);
    egui::Panel::left("steps")
        .resizable(true)
        .default_size(300.0)
        .size_range(200.0..=max_list)
        .frame(
            egui::Frame::new()
                .fill(p.bar)
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(10, 12)),
        )
        .show(root, |ui| steps_list(app, ui, snapshot.as_ref(), nav));

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::same(16)))
        .show(root, |ui| selected_step(app, ui, snapshot.as_ref()));
}

// ── The list ────────────────────────────────────────────────────────

fn steps_list(app: &mut App, ui: &mut egui::Ui, snap: Option<&Snapshot>, nav: bool) {
    let lang = app.lang;
    let p = pal(ui.ctx());
    let rows = snap.map(|s| s.rows.as_slice()).unwrap_or(&[]);
    // Keep the panel at its own width, whatever its content.
    ui.set_min_width(ui.available_width());
    ui.horizontal(|ui| {
        widgets::eyebrow(ui, lang.pick("Étapes", "Steps"));
        ui.label(
            RichText::new(rows.len().to_string())
                .size(theme::SIZE_EYEBROW)
                .strong()
                .color(p.accent_text),
        );
    });
    ui.add_space(2.0);

    if rows.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new(lang.pick("Aucune étape pour l'instant.", "No steps yet."))
                .color(p.text_faint),
        );
        return;
    }
    let Some(snap) = snap else { return };

    // While recording and following the latest step, scroll to new ones.
    let follow_new = app.selected.is_none() && rows.len() != app.seen_rows;
    app.seen_rows = rows.len();
    let current = app.selected.or_else(|| rows.last().map(|r| r.index));

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for row in rows {
                let is_sel = current == Some(row.index);
                let thumb = row
                    .screenshot
                    .as_deref()
                    .and_then(|name| app.textures.thumb(ui.ctx(), &snap.dir, name));
                let resp = step_row(ui, row, is_sel, thumb.as_ref(), lang);
                if resp.clicked() {
                    app.selected = Some(row.index);
                }
                if is_sel && (nav || follow_new) {
                    resp.scroll_to_me(Some(Align::Center));
                }
            }
        });
}

fn step_row(
    ui: &mut egui::Ui,
    row: &StepRow,
    selected: bool,
    thumb: Option<&egui::TextureHandle>,
    lang: Lang,
) -> egui::Response {
    let p = pal(ui.ctx());
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());
    let title = format!("{}. {}", row.index, row.short_title(lang));
    resp.widget_info(|| WidgetInfo::selected(Role::ListItem, true, selected, &title));
    if !ui.is_rect_visible(rect) {
        return resp;
    }

    let painter = ui.painter();
    let radius = CornerRadius::same(theme::RADIUS_SMALL);
    if selected {
        painter.rect_filled(rect, radius, p.tint(p.accent, 40));
        let bar = Rect::from_min_size(
            rect.min + Vec2::new(0.0, 8.0),
            Vec2::new(3.0, rect.height() - 16.0),
        );
        painter.rect_filled(bar, CornerRadius::same(2), p.accent);
    } else if resp.hovered() {
        painter.rect_filled(rect, radius, p.raised);
    }

    // The thumbnail, or a pictogram for typing steps.
    let thumb_size = THUMB * ((width - 20.0) / 280.0).clamp(0.6, 1.0);
    let thumb_rect = Rect::from_min_size(
        rect.min + Vec2::new(10.0, (ROW_HEIGHT - thumb_size.y) / 2.0),
        thumb_size,
    );
    painter.rect_filled(thumb_rect, CornerRadius::same(4), p.canvas);
    match thumb {
        Some(tex) => {
            let img = tex.size_vec2();
            let s = (thumb_size.x / img.x).min(thumb_size.y / img.y);
            let r = Rect::from_center_size(thumb_rect.center(), img * s);
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            let icon = if row.is_click() {
                Icon::Camera
            } else {
                Icon::Keyboard
            };
            icon.paint(
                painter,
                Rect::from_center_size(thumb_rect.center(), Vec2::splat(thumb_size.y * 0.45)),
                p.text_faint,
            );
        }
    }
    painter.rect_stroke(
        thumb_rect,
        CornerRadius::same(4),
        Stroke::new(1.0, p.border),
        egui::StrokeKind::Inside,
    );

    // Three lines of text: title + offset, target, typed text or nothing.
    let x = thumb_rect.right() + 10.0;
    let text_w = (rect.right() - 8.0 - x).max(20.0);
    let offset = steps::offset(row.elapsed_ms, lang);
    let offset_g = painter.layout_no_wrap(
        offset,
        FontId::monospace(theme::SIZE_SMALL - 1.0),
        p.text_faint,
    );
    let title_font = FontId::proportional(theme::SIZE_BODY);
    let title_w = painter
        .layout_no_wrap(title.clone(), title_font.clone(), p.text)
        .size()
        .x;
    // The title wins over the offset when the list is narrow (the offset is
    // also in the step's details).
    let show_offset = title_w + 6.0 + offset_g.size().x <= text_w;
    let title_g = one_line(ui, &title, title_font, p.text, text_w);
    let top = rect.top() + 9.0;
    painter.galley(egui::pos2(x, top), title_g, p.text);
    if show_offset {
        painter.galley(
            egui::pos2(rect.right() - 8.0 - offset_g.size().x, top + 2.0),
            offset_g,
            p.text_faint,
        );
    }
    let second = match &row.action {
        Action::Text { content } => format!("\u{201c}{}\u{201d}", steps::shown_text(content)),
        Action::Click { .. } => row
            .target()
            .unwrap_or_else(|| lang.pick("Fenêtre inconnue", "Unknown window").into()),
    };
    let second_g = one_line(
        ui,
        &second,
        FontId::proportional(theme::SIZE_SMALL),
        p.text_muted,
        text_w,
    );
    painter.galley(egui::pos2(x, top + 20.0), second_g, p.text_muted);
    if let (Action::Text { .. }, Some(target)) = (&row.action, row.target()) {
        let third = one_line(
            ui,
            &target,
            FontId::proportional(theme::SIZE_SMALL),
            p.text_faint,
            text_w,
        );
        painter.galley(egui::pos2(x, top + 36.0), third, p.text_faint);
    }

    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// `text` laid out on one line, cut with an ellipsis to fit `width`.
fn one_line(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: egui::Color32,
    width: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = width.max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.fonts_mut(|f| f.layout_job(job))
}

// ── The selected step ───────────────────────────────────────────────

fn selected_step(app: &mut App, ui: &mut egui::Ui, snap: Option<&Snapshot>) {
    let lang = app.lang;
    let p = pal(ui.ctx());
    let Some(snap) = snap.filter(|s| !s.rows.is_empty()) else {
        empty_state(app, ui);
        return;
    };
    let current = app
        .selected
        .and_then(|sel| snap.position(sel))
        .unwrap_or(snap.rows.len() - 1);
    let row = &snap.rows[current];

    widgets::eyebrow(
        ui,
        &format!(
            "{} {} / {} · {}",
            lang.pick("Étape", "Step"),
            current + 1,
            snap.rows.len(),
            steps::offset(row.elapsed_ms, lang)
        ),
    );
    ui.add(
        egui::Label::new(
            RichText::new(row.description(lang))
                .size(17.0)
                .strong()
                .color(p.text),
        )
        .wrap(),
    );
    ui.add_space(6.0);

    let side_meta = ui.available_width() >= SIDE_META_MIN_WIDTH;
    if side_meta {
        egui::Panel::right("step_meta")
            .resizable(false)
            .exact_size(270.0)
            .frame(egui::Frame::new().inner_margin(Margin {
                left: 14,
                ..Default::default()
            }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| metadata(ui, row, lang, None));
            });
    } else {
        // Narrow: the screenshot keeps the room, the details fold away (and
        // scroll past a third of the height when unfolded).
        let details_max = (ui.available_height() * 0.35).max(80.0);
        widgets::card(ui)
            .inner_margin(Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::CollapsingHeader::new(
                    RichText::new(lang.pick("Détails de l'étape", "Step details"))
                        .size(theme::SIZE_SMALL)
                        .color(p.text_muted),
                )
                .id_salt("step_details")
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(details_max)
                        .show(ui, |ui| {
                            let columns = if ui.available_width() >= 520.0 { 2 } else { 1 };
                            metadata(ui, row, lang, Some(columns));
                        });
                });
            });
        ui.add_space(8.0);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show(ui, |ui| match &row.screenshot {
            Some(name) => match app.textures.full(ui.ctx(), &snap.dir, name) {
                Some(tex) => {
                    let actions = app.viewer.show(ui, &tex, lang, true);
                    if actions.enlarge {
                        app.enlarged = Some(Viewer::enlarging());
                    }
                }
                None => {
                    let msg = if app.stage() == Stage::Recording {
                        lang.pick("Capture en cours d'écriture…", "Screenshot being written…")
                    } else {
                        lang.pick("Capture introuvable.", "Screenshot not found.")
                    };
                    placeholder(ui, Icon::Camera, msg, None);
                    // Only while recording: the file is about to appear. A
                    // screenshot missing afterwards will not come back.
                    if app.stage() == Stage::Recording {
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(300));
                    }
                }
            },
            None => {
                let typed = match &row.action {
                    Action::Text { content } => Some(content.as_str()),
                    Action::Click { .. } => None,
                };
                placeholder(
                    ui,
                    Icon::Keyboard,
                    lang.pick(
                        "Étape de saisie clavier : pas de capture associée.",
                        "Keyboard input step: no screenshot.",
                    ),
                    typed,
                );
            }
        });
}

/// Before the first step: what to do next.
fn empty_state(app: &App, ui: &mut egui::Ui) {
    let lang = app.lang;
    if app.stage() == Stage::Recording {
        placeholder(
            ui,
            Icon::Mouse,
            lang.pick(
                "En attente du premier clic… Chaque clic dans une autre fenêtre ajoute une étape ici.",
                "Waiting for the first click… Each click in another window adds a step here.",
            ),
            None,
        );
    } else {
        placeholder(
            ui,
            Icon::Mouse,
            lang.pick(
                "Aucune étape n'a été enregistrée.",
                "No steps were recorded.",
            ),
            None,
        );
    }
}

/// A centred pictogram with a message (and the typed text, for typing steps).
fn placeholder(ui: &mut egui::Ui, icon: Icon, msg: &str, typed: Option<&str>) {
    let p = pal(ui.ctx());
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(rect, CornerRadius::same(theme::RADIUS_SMALL), p.canvas);
    let content_h = if typed.is_some() { 160.0 } else { 90.0 };
    ui.add_space(((rect.height() - content_h) / 2.0).max(8.0));
    ui.vertical_centered(|ui| {
        ui.set_max_width((rect.width() - 32.0).clamp(100.0, 560.0));
        icons::icon(ui, icon, 36.0, p.text_faint);
        ui.add(egui::Label::new(RichText::new(msg).color(p.text_muted)).wrap());
        if let Some(text) = typed {
            ui.add_space(6.0);
            widgets::card(ui).show(ui, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(text).monospace().size(16.0).color(p.text))
                        .wrap()
                        .selectable(true),
                );
            });
        }
    });
}

/// The facts of a step: stacked label over value (`None`, the side column),
/// or a compact grid of `columns` label/value pairs per line.
fn metadata(ui: &mut egui::Ui, row: &StepRow, lang: Lang, grid: Option<usize>) {
    let mut items: Vec<(Icon, &str, String)> = Vec::new();
    items.push((
        Icon::Clock,
        lang.pick("Heure", "Time"),
        steps::human_date(row.timestamp_ms, lang),
    ));
    items.push((
        Icon::Clock,
        lang.pick("Depuis le début", "Since start"),
        steps::offset(row.elapsed_ms, lang),
    ));
    match &row.action {
        Action::Click {
            button,
            x,
            y,
            rel_x,
            rel_y,
        } => {
            items.push((
                Icon::Mouse,
                lang.pick("Action", "Action"),
                lang.pick("Clic", "Click").into(),
            ));
            items.push((
                Icon::Mouse,
                lang.pick("Bouton", "Button"),
                button.label(lang),
            ));
            let pos = match (x.zip(*y), rel_x.zip(*rel_y)) {
                (Some((x, y)), Some((rx, ry))) => format!(
                    "{x}, {y} {}\n{rx}, {ry} {}",
                    lang.pick("à l'écran", "on screen"),
                    lang.pick("dans la fenêtre", "in the window"),
                ),
                (Some((x, y)), None) => format!("{x}, {y} {}", lang.pick("à l'écran", "on screen")),
                _ => lang
                    .pick("non disponible (Wayland)", "not available (Wayland)")
                    .into(),
            };
            items.push((Icon::Mouse, lang.pick("Position", "Position"), pos));
        }
        Action::Text { content } => {
            items.push((
                Icon::Keyboard,
                lang.pick("Action", "Action"),
                lang.pick("Saisie au clavier", "Typing").into(),
            ));
            items.push((
                Icon::Keyboard,
                lang.pick("Texte saisi", "Typed text"),
                steps::shown_text(content),
            ));
        }
    }

    let window = row.window.as_ref();
    let title = window.map(|w| w.title.as_str()).filter(|t| !t.is_empty());
    let app_name = window
        .map(|w| w.app_name.as_str())
        .filter(|a| !a.is_empty());
    items.push((
        Icon::Window,
        lang.pick("Fenêtre", "Window"),
        match (window, title) {
            (_, Some(t)) => t.to_owned(),
            (Some(_), None) => lang.pick("(sans titre)", "(untitled)").into(),
            (None, None) => lang
                .pick(
                    "inconnue (non exposée sous Wayland)",
                    "unknown (not exposed under Wayland)",
                )
                .into(),
        },
    ));
    if let Some(a) = app_name {
        let pid = window.map(|w| w.pid).unwrap_or(0);
        let value = if pid > 0 {
            format!("{a} (PID {pid})")
        } else {
            a.to_owned()
        };
        items.push((Icon::Window, lang.pick("Application", "Application"), value));
    }
    if let Some(w) = window.filter(|w| w.width > 0 && w.height > 0) {
        items.push((
            Icon::Window,
            lang.pick("Géométrie", "Geometry"),
            format!("{} × {} @ {}, {}", w.width, w.height, w.x, w.y),
        ));
    }
    if let Some(name) = &row.screenshot {
        items.push((
            Icon::Camera,
            lang.pick("Capture", "Screenshot"),
            name.clone(),
        ));
    }

    let p = pal(ui.ctx());
    let Some(columns) = grid else {
        for (icon, label, value) in &items {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                icons::icon(ui, *icon, 13.0, p.text_faint);
                widgets::eyebrow(ui, label);
            });
            ui.add_space(-6.0);
            ui.add(
                egui::Label::new(RichText::new(value).color(p.text))
                    .wrap()
                    .selectable(true),
            );
            ui.add_space(2.0);
        }
        return;
    };
    {
        egui::Grid::new("step_meta_grid")
            .num_columns(columns * 2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (i, (_, label, value)) in items.iter().enumerate() {
                    ui.label(
                        RichText::new(*label)
                            .size(theme::SIZE_SMALL)
                            .color(p.text_faint),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(value.replace('\n', " · "))
                                .size(theme::SIZE_SMALL)
                                .color(p.text),
                        )
                        .truncate()
                        .selectable(true),
                    );
                    if i % columns == columns - 1 {
                        ui.end_row();
                    }
                }
            });
    }
}
