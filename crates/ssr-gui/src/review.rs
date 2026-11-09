//! The steps of a session or guide: the timeline with thumbnails on the
//! left, the selected step's screenshot in the middle and, in the editor,
//! its properties on the right (title, instructions, actions, metadata).
//! Shared by the recording (live, read-only) and editor stages.

use std::sync::Arc;

use eframe::egui;
use egui::{
    text::LayoutJob, Align, CornerRadius, FontId, Galley, Margin, Rect, RichText, Role, Sense,
    Stroke, TextFormat, Vec2, WidgetInfo,
};
use ssr_core::{Action, Lang};
use uuid::Uuid;

use crate::app::{App, Draft, Stage};
use crate::icons::{self, Icon};
use crate::steps::{self, Snapshot, StepRow};
use crate::theme::{self, pal};
use crate::viewer::Viewer;
use crate::widgets::{self, Kind};

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
    let current = snap.current(app.selected).map(|i| rows[i].id);
    let editable = snap.editable && app.can_edit();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut rects: Vec<(Uuid, Rect)> = Vec::with_capacity(rows.len());
            let multi = app.multi.len() > 1;
            for row in rows {
                let is_sel = current == Some(row.id);
                let in_selection = multi && app.multi.contains(&row.id);
                let thumb = row
                    .screenshot
                    .as_deref()
                    .and_then(|name| app.textures.thumb(ui.ctx(), &snap.dir, name));
                // While dragging a selection, every selected row is outlined.
                let dragged = app.dragging == Some(row.id)
                    || (app.dragging.is_some_and(|d| app.multi.contains(&d)) && in_selection);
                let state = RowState {
                    selected: is_sel,
                    in_selection,
                    dragged,
                    editable,
                    guide: snap.editable,
                };
                let resp = step_row(ui, row, state, thumb.as_ref(), lang);
                if resp.clicked() {
                    let modifiers = ui.input(|i| i.modifiers);
                    if snap.editable {
                        app.click_select(row.id, modifiers);
                    } else {
                        app.select(row.id);
                    }
                }
                if editable && resp.drag_started() {
                    // Dragging a row of the selection moves the selection.
                    if !app.is_selected(row.id) {
                        app.select(row.id);
                    }
                    app.dragging = Some(row.id);
                }
                if is_sel && (nav || follow_new) {
                    resp.scroll_to_me(Some(Align::Center));
                }
                rects.push((row.id, resp.rect));
            }
            if editable {
                drag_and_drop(app, ui, &rects);
            }
        });
}

/// While a step is dragged: show where it would land, scroll near the
/// edges, and move it on release.
fn drag_and_drop(app: &mut App, ui: &mut egui::Ui, rects: &[(Uuid, Rect)]) {
    let Some(dragged) = app.dragging else { return };
    let p = pal(ui.ctx());
    let (pointer, released) = ui.input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.any_released() || !i.pointer.any_down(),
        )
    });
    if !rects.iter().any(|(id, _)| *id == dragged) {
        app.dragging = None;
        return;
    }
    // Rows that move: the selection if the dragged row is part of it.
    let moving: Vec<usize> = if app.is_selected(dragged) {
        let sel = app.selection();
        rects
            .iter()
            .enumerate()
            .filter(|(_, (id, _))| sel.contains(id))
            .map(|(i, _)| i)
            .collect()
    } else {
        rects
            .iter()
            .position(|(id, _)| *id == dragged)
            .into_iter()
            .collect()
    };
    let (first, last) = (
        moving.first().copied().unwrap_or(0),
        moving.last().copied().unwrap_or(0),
    );
    let contiguous = last + 1 - first == moving.len();
    let Some(pos) = pointer else {
        if released {
            app.dragging = None;
        }
        return;
    };
    // Insertion slot: before the first row whose middle is below the pointer.
    let slot = rects
        .iter()
        .position(|(_, r)| pos.y < r.center().y)
        .unwrap_or(rects.len());
    let y = match rects.get(slot) {
        Some((_, r)) => r.top() - 2.0,
        None => rects.last().map(|(_, r)| r.bottom() + 2.0).unwrap_or(0.0),
    };
    let (left, right) = rects
        .first()
        .map(|(_, r)| (r.left(), r.right()))
        .unwrap_or_default();
    // Dropping a contiguous block next to itself changes nothing.
    let noop = contiguous && slot >= first && slot <= last + 1;
    if !noop {
        ui.painter().line_segment(
            [egui::pos2(left + 4.0, y), egui::pos2(right - 4.0, y)],
            Stroke::new(3.0, p.accent),
        );
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    // Scroll when dragging near the top or bottom of the list.
    let clip = ui.clip_rect();
    if pos.y < clip.top() + 24.0 {
        ui.scroll_with_delta(Vec2::new(0.0, 8.0));
        ui.ctx().request_repaint();
    } else if pos.y > clip.bottom() - 24.0 {
        ui.scroll_with_delta(Vec2::new(0.0, -8.0));
        ui.ctx().request_repaint();
    }
    if released {
        app.dragging = None;
        if !noop {
            app.drop_steps(dragged, slot);
        }
    }
}

/// How a row is drawn.
#[derive(Clone, Copy)]
struct RowState {
    /// The step shown on the right.
    selected: bool,
    /// Part of a multiple selection.
    in_selection: bool,
    dragged: bool,
    /// The row can be dragged (an editable guide, nothing running).
    editable: bool,
    /// A guide rather than a live recording: no "time since start".
    guide: bool,
}

fn step_row(
    ui: &mut egui::Ui,
    row: &StepRow,
    state: RowState,
    thumb: Option<&egui::TextureHandle>,
    lang: Lang,
) -> egui::Response {
    let RowState {
        selected,
        in_selection,
        dragged,
        editable,
        guide,
    } = state;
    let p = pal(ui.ctx());
    let width = ui.available_width();
    let sense = if editable {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), sense);
    let title = format!("{}. {}", row.index, row.short_title(lang));
    resp.widget_info(|| {
        WidgetInfo::selected(Role::ListItem, true, selected || in_selection, &title)
    });
    if !ui.is_rect_visible(rect) {
        return resp;
    }

    let painter = ui.painter();
    let radius = CornerRadius::same(theme::RADIUS_SMALL);
    if dragged {
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    if in_selection && !selected {
        painter.rect_filled(rect, radius, p.tint(p.accent, 26));
    }
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
            let icon = if row.is_manual() {
                Icon::Pencil
            } else if row.is_click() {
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
    // A guide may mix recordings: a "time since start" would be misleading.
    let show_offset = !guide && title_w + 6.0 + offset_g.size().x <= text_w;
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
        _ if row.is_manual() => {
            if row.notes.is_empty() {
                lang.pick("Étape rédigée", "Written step").into()
            } else {
                row.notes.lines().next().unwrap_or_default().to_string()
            }
        }
        _ if row.custom_title().is_some() => row.auto_description(lang),
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
    if let (Action::Text { .. }, false, false, Some(target)) = (
        &row.action,
        row.is_manual(),
        row.custom_title().is_some(),
        row.target(),
    ) {
        let third = one_line(
            ui,
            &target,
            FontId::proportional(theme::SIZE_SMALL),
            p.text_faint,
            text_w,
        );
        painter.galley(egui::pos2(x, top + 36.0), third, p.text_faint);
    }

    if editable {
        resp.on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text(lang.pick(
            "Glisser pour déplacer · Alt+↑/↓\nCtrl+clic / Maj+clic : sélectionner plusieurs étapes",
            "Drag to move · Alt+Up/Down\nCtrl+click / Shift+click: select several steps",
        ))
    } else {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
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
    let current = snap.current(app.selected).unwrap_or(0);
    let row = &snap.rows[current];
    let editable = snap.editable;

    let mut eyebrow = format!(
        "{} {} / {}",
        lang.pick("Étape", "Step"),
        current + 1,
        snap.rows.len()
    );
    if !editable {
        eyebrow.push_str(&format!(" · {}", steps::offset(row.elapsed_ms, lang)));
    }
    widgets::eyebrow(ui, &eyebrow);
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

    if !row.notes.is_empty() && !editable {
        ui.add(egui::Label::new(RichText::new(&row.notes).color(p.text_muted)).wrap());
        ui.add_space(4.0);
    }

    let side_meta = ui.available_width() >= if editable { 640.0 } else { SIDE_META_MIN_WIDTH };
    if side_meta {
        egui::Panel::right("step_meta")
            .resizable(false)
            .exact_size(if editable { 300.0 } else { 270.0 })
            .frame(egui::Frame::new().inner_margin(Margin {
                left: 14,
                ..Default::default()
            }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if editable {
                            properties(app, ui, row);
                            ui.separator();
                        }
                        metadata(ui, row, lang, None);
                    });
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
                    RichText::new(if editable {
                        lang.pick("Modifier l'étape", "Edit step")
                    } else {
                        lang.pick("Détails de l'étape", "Step details")
                    })
                    .size(theme::SIZE_SMALL)
                    .color(p.text_muted),
                )
                .id_salt("step_details")
                .default_open(editable)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(details_max)
                        .show(ui, |ui| {
                            if editable {
                                properties(app, ui, row);
                                ui.separator();
                            }
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
                    Action::Text { content } if !row.is_manual() => Some(content.as_str()),
                    _ => None,
                };
                let (icon, msg) = if row.is_manual() {
                    (
                        Icon::Pencil,
                        lang.pick(
                            "Étape rédigée, sans capture. « Image… » pour en ajouter une.",
                            "Written step, no screenshot. Use “Image…” to add one.",
                        ),
                    )
                } else if typed.is_some() {
                    (
                        Icon::Keyboard,
                        lang.pick(
                            "Étape de saisie clavier : pas de capture associée.",
                            "Keyboard input step: no screenshot.",
                        ),
                    )
                } else {
                    (
                        Icon::Camera,
                        lang.pick(
                            "Pas de capture pour cette étape.",
                            "No screenshot for this step.",
                        ),
                    )
                };
                placeholder(ui, icon, msg, typed);
            }
        });
}

/// The editable side of a step: its title, instructions and actions. The
/// text is typed into a draft and written back on Enter / focus loss, so a
/// redraw never overwrites it; Escape drops the change.
fn properties(app: &mut App, ui: &mut egui::Ui, row: &StepRow) {
    if app.multi.len() > 1 {
        multi_properties(app, ui);
        return;
    }
    let lang = app.lang;
    let p = pal(ui.ctx());
    let enabled = app.can_edit();
    if app.draft.as_ref().is_some_and(|d| d.id != row.id) {
        app.commit_draft();
    }
    let draft = app.draft.get_or_insert_with(|| Draft {
        id: row.id,
        title: row.title_override.clone().unwrap_or_default(),
        notes: row.notes.clone(),
    });
    let auto = row.auto_description(lang);
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let mut commit = false;
    let mut revert_title = false;
    let mut revert_notes = false;

    ui.add_enabled_ui(enabled, |ui| {
        widgets::eyebrow(ui, lang.pick("Titre de l'étape", "Step title"));
        let r = ui.add(
            egui::TextEdit::singleline(&mut draft.title)
                .hint_text(&auto)
                .desired_width(f32::INFINITY),
        );
        if r.lost_focus() {
            if escape {
                revert_title = true;
            } else {
                commit = true;
            }
        }
        ui.horizontal(|ui| {
            let (txt, col) = if row.custom_title().is_some() {
                (
                    lang.pick("Titre personnalisé", "Custom title"),
                    p.accent_text,
                )
            } else {
                (
                    lang.pick("Titre automatique", "Automatic title"),
                    p.text_faint,
                )
            };
            ui.label(RichText::new(txt).size(theme::SIZE_SMALL).color(col));
            if row.custom_title().is_some()
                && ui
                    .small_button(lang.pick("Rétablir l'automatique", "Restore automatic"))
                    .clicked()
            {
                draft.title.clear();
                commit = true;
            }
        });
        ui.add_space(4.0);
        widgets::eyebrow(ui, lang.pick("Consigne détaillée", "Instructions"));
        let r = ui.add(
            egui::TextEdit::multiline(&mut draft.notes)
                .hint_text(lang.pick(
                    "Expliquez ce qu'il faut faire à cette étape…",
                    "Explain what to do at this step…",
                ))
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        if r.lost_focus() {
            if escape {
                revert_notes = true;
            } else {
                commit = true;
            }
        }
    });
    if revert_title {
        draft.title = row.title_override.clone().unwrap_or_default();
    }
    if revert_notes {
        draft.notes = row.notes.clone();
    }
    if commit {
        app.commit_draft();
    }

    ui.add_space(6.0);
    let position = app.snapshot.as_ref().and_then(|s| s.position(row.id));
    let count = app.snapshot.as_ref().map(|s| s.rows.len()).unwrap_or(0);
    let mut act = 0;
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(position.is_some_and(|p| p > 0), |ui| {
                if widgets::icon_button(
                    ui,
                    Icon::ArrowUp,
                    lang.pick("Monter (Alt+↑)", "Move up (Alt+Up)"),
                )
                .clicked()
                {
                    act = 1;
                }
            });
            ui.add_enabled_ui(position.is_some_and(|p| p + 1 < count), |ui| {
                if widgets::icon_button(
                    ui,
                    Icon::ArrowDown,
                    lang.pick("Descendre (Alt+↓)", "Move down (Alt+Down)"),
                )
                .clicked()
                {
                    act = 2;
                }
            });
            if widgets::button(
                ui,
                Kind::Ghost,
                Some(Icon::Trash),
                lang.pick("Supprimer l'étape", "Delete step"),
            )
            .on_hover_text(lang.pick(
                "Retire l'étape du guide (Suppr ; Ctrl+Z pour annuler)",
                "Removes the step from the guide (Delete; Ctrl+Z to undo)",
            ))
            .clicked()
            {
                act = 3;
            }
        });
        ui.horizontal_wrapped(|ui| {
            let label = if row.screenshot.is_some() {
                lang.pick("Remplacer la capture…", "Replace screenshot…")
            } else {
                lang.pick("Ajouter une capture…", "Add screenshot…")
            };
            if widgets::button(ui, Kind::Secondary, Some(Icon::Image), label).clicked() {
                act = 4;
            }
            if row.screenshot.is_some()
                && widgets::button(
                    ui,
                    Kind::Ghost,
                    Some(Icon::Minus),
                    lang.pick("Retirer la capture", "Remove screenshot"),
                )
                .on_hover_text(lang.pick(
                    "Garde l'étape et son texte ; seule l'image est retirée du guide.",
                    "Keeps the step and its text; only the image leaves the guide.",
                ))
                .clicked()
            {
                act = 5;
            }
            let typed = matches!(&row.action, Action::Text { content }
                if !content.is_empty() && content != ssr_core::editor::REDACTED);
            if typed
                && !row.is_manual()
                && widgets::button(
                    ui,
                    Kind::Ghost,
                    Some(Icon::Keyboard),
                    lang.pick("Masquer le texte saisi", "Redact typed text"),
                )
                .on_hover_text(lang.pick(
                    "Remplace le texte tapé (mot de passe…) par •••, dans le guide et l'export.",
                    "Replaces the typed text (a password…) with •••, in the guide and the export.",
                ))
                .clicked()
            {
                act = 6;
            }
        });
    });
    let id = row.id;
    match act {
        1 => {
            app.select(id);
            app.move_selected(-1);
        }
        2 => {
            app.select(id);
            app.move_selected(1);
        }
        3 => app.delete_step(id),
        4 => {
            app.select(id);
            app.add_image();
        }
        5 => {
            app.edit(|d| d.detach_screenshot(id));
        }
        6 => {
            app.edit(|d| d.redact_text(id));
        }
        _ => {}
    }
}

/// Several steps selected: act on all of them at once.
fn multi_properties(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(ui.ctx());
    let enabled = app.can_edit();
    let selection = app.selection();
    let n = selection.len();
    let count = app.snapshot.as_ref().map(|s| s.rows.len()).unwrap_or(0);
    let positions: Vec<usize> = app
        .snapshot
        .as_ref()
        .map(|s| selection.iter().filter_map(|id| s.position(*id)).collect())
        .unwrap_or_default();
    // Up is possible unless the selection is already packed at the top
    // (and likewise for down).
    let can_up = positions.iter().enumerate().any(|(k, &i)| i != k);
    let can_down = positions
        .iter()
        .rev()
        .enumerate()
        .any(|(k, &i)| i + 1 + k != count);

    ui.horizontal(|ui| {
        icons::icon(ui, Icon::Check, 16.0, p.accent_text);
        ui.label(
            RichText::new(format!(
                "{n} {}",
                lang.pick("étapes sélectionnées", "steps selected")
            ))
            .size(16.0)
            .strong()
            .color(p.text),
        );
    });
    let numbers: Vec<String> = positions.iter().map(|i| (i + 1).to_string()).collect();
    ui.add(
        egui::Label::new(
            RichText::new(format!(
                "{} {}",
                lang.pick("Étapes", "Steps"),
                numbers.join(", ")
            ))
            .size(theme::SIZE_SMALL)
            .color(p.text_muted),
        )
        .wrap(),
    );
    ui.add_space(8.0);
    let mut act = 0;
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(can_up, |ui| {
                if widgets::icon_button(
                    ui,
                    Icon::ArrowUp,
                    lang.pick("Monter (Alt+↑)", "Move up (Alt+Up)"),
                )
                .clicked()
                {
                    act = 1;
                }
            });
            ui.add_enabled_ui(can_down, |ui| {
                if widgets::icon_button(
                    ui,
                    Icon::ArrowDown,
                    lang.pick("Descendre (Alt+↓)", "Move down (Alt+Down)"),
                )
                .clicked()
                {
                    act = 2;
                }
            });
            let label = format!(
                "{} {n} {}",
                lang.pick("Supprimer les", "Delete"),
                lang.pick("étapes", "steps")
            );
            if widgets::button(ui, Kind::Ghost, Some(Icon::Trash), &label)
                .on_hover_text(
                    lang.pick("Suppr ; Ctrl+Z pour annuler", "Delete key; Ctrl+Z to undo"),
                )
                .clicked()
            {
                act = 3;
            }
        });
    });
    ui.add_space(6.0);
    ui.add(
        egui::Label::new(
            RichText::new(lang.pick(
                "Glissez une des étapes sélectionnées pour les déplacer ensemble. Ctrl+clic ajoute ou retire une étape, Maj+clic sélectionne une plage, Ctrl+A tout, Échap revient à une seule étape.",
                "Drag one of the selected steps to move them together. Ctrl+click adds or removes a step, Shift+click selects a range, Ctrl+A all, Esc goes back to one step.",
            ))
            .size(theme::SIZE_SMALL)
            .color(p.text_faint),
        )
        .wrap(),
    );
    if widgets::button(
        ui,
        Kind::Secondary,
        None,
        lang.pick("Une seule étape", "Single step"),
    )
    .clicked()
    {
        act = 4;
    }
    match act {
        1 => app.move_selected(-1),
        2 => app.move_selected(1),
        3 => app.delete_selection(),
        4 => app.clear_multi(),
        _ => {}
    }
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
                "Ce guide est vide. Importez des étapes d'un enregistrement, ajoutez une image ou une étape rédigée.",
                "This guide is empty. Import steps from a recording, add an image or a written step.",
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
    if !row.is_manual() {
        items.push((
            Icon::Clock,
            if row.source.is_some() {
                lang.pick("Dans son enregistrement", "In its recording")
            } else {
                lang.pick("Depuis le début", "Since start")
            },
            steps::offset(row.elapsed_ms, lang),
        ));
    }
    if row.is_manual() {
        items[0].1 = lang.pick("Créée le", "Created");
        items.push((
            Icon::Pencil,
            lang.pick("Type", "Kind"),
            lang.pick("Étape rédigée dans l'éditeur", "Step written in the editor")
                .into(),
        ));
        if let Some(name) = &row.screenshot {
            items.push((
                Icon::Camera,
                lang.pick("Capture", "Screenshot"),
                short_asset(name),
            ));
        }
        return show_items(ui, &items, grid);
    }
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
    if let Some(src) = &row.source {
        items.push((
            Icon::Report,
            lang.pick("Source", "Source"),
            format!(
                "{} · {} {}",
                src.source_label,
                lang.pick("étape", "step"),
                src.original_index
            ),
        ));
    }
    if let Some(name) = &row.screenshot {
        items.push((
            Icon::Camera,
            lang.pick("Capture", "Screenshot"),
            short_asset(name),
        ));
    }

    show_items(ui, &items, grid);
}

/// A screenshot reference, shortened for display.
fn short_asset(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    if base.chars().count() > 20 {
        format!("{}…", base.chars().take(16).collect::<String>())
    } else {
        base.to_string()
    }
}

fn show_items(ui: &mut egui::Ui, items: &[(Icon, &str, String)], grid: Option<usize>) {
    let p = pal(ui.ctx());
    let Some(columns) = grid else {
        for (icon, label, value) in items {
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
