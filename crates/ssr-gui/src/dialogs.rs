//! Modal layers: the "session not saved" confirmation, the "start over?"
//! confirmation, the export progress and the enlarged screenshot.

use eframe::egui;
use egui::{Id, Margin, RichText, Sense};

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{self, pal};
use crate::widgets::{self, Kind};

/// A modal card with an icon, a title and a message; `buttons` fills the row.
fn modal<R>(
    ctx: &egui::Context,
    id: &str,
    icon: Icon,
    tone: egui::Color32,
    title: &str,
    message: &str,
    buttons: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let p = pal(ctx);
    egui::Modal::new(Id::unique(id))
        .frame(widgets_card_frame(ctx).inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(34.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(9), p.tint(tone, 36));
                icon.paint(ui.painter(), rect.shrink(8.0), tone);
                ui.label(RichText::new(title).size(18.0).strong().color(p.text));
            });
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(message).color(p.text_muted)).wrap());
            ui.add_space(14.0);
            ui.horizontal_wrapped(buttons).inner
        })
        .inner
}

fn widgets_card_frame(ctx: &egui::Context) -> egui::Frame {
    let p = pal(ctx);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS))
        .shadow(ctx.global_style().visuals.window_shadow)
}

/// After the save dialog was cancelled.
pub fn confirm_discard(app: &mut App, ctx: &egui::Context) {
    if !app.confirm_discard {
        return;
    }
    let lang = app.lang;
    let p = pal(ctx);
    #[derive(PartialEq)]
    enum Choice {
        None,
        Retry,
        Review,
        Discard,
    }
    let choice = modal(
        ctx,
        "confirm_discard",
        Icon::Warning,
        p.warning,
        lang.pick("Session non enregistrée", "Session not saved"),
        lang.pick(
            "Si vous continuez, cette session ne sera pas sauvegardée. Vous pouvez choisir un emplacement, relire d'abord les étapes (et enregistrer plus tard), ou l'abandonner.",
            "If you continue, this session will not be saved. You can choose a location, review the steps first (and save later), or discard it.",
        ),
        |ui| {
            if widgets::button(
                ui,
                Kind::Primary,
                Some(Icon::Save),
                lang.pick("Choisir un emplacement…", "Choose a location…"),
            )
            .clicked()
            {
                return Choice::Retry;
            }
            if widgets::button(
                ui,
                Kind::Secondary,
                None,
                lang.pick("Relire d'abord", "Review first"),
            )
            .clicked()
            {
                return Choice::Review;
            }
            if widgets::button(
                ui,
                Kind::Ghost,
                Some(Icon::Trash),
                lang.pick("Abandonner la session", "Discard session"),
            )
            .clicked()
            {
                return Choice::Discard;
            }
            Choice::None
        },
    );
    // Escape keeps the session for review: the gentlest of the three.
    let choice = if choice == Choice::None && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        Choice::Review
    } else {
        choice
    };
    match choice {
        Choice::Retry => {
            app.confirm_discard = false;
            app.begin_save();
        }
        Choice::Review => app.keep_reviewing(),
        Choice::Discard => app.discard(),
        Choice::None => {}
    }
}

/// "New recording" while the reviewed session is not saved.
pub fn confirm_new(app: &mut App, ctx: &egui::Context) {
    if !app.confirm_new {
        return;
    }
    let lang = app.lang;
    let p = pal(ctx);
    let mut choice = 0;
    modal(
        ctx,
        "confirm_new",
        Icon::Warning,
        p.warning,
        lang.pick(
            "Commencer un nouvel enregistrement ?",
            "Start a new recording?",
        ),
        lang.pick(
            "La session actuelle n'est pas enregistrée : ses captures seront supprimées.",
            "The current session is not saved: its screenshots will be deleted.",
        ),
        |ui| {
            if widgets::button(
                ui,
                Kind::Primary,
                Some(Icon::Save),
                lang.pick("Enregistrer d'abord…", "Save first…"),
            )
            .clicked()
            {
                choice = 1;
            }
            if widgets::button(
                ui,
                Kind::Secondary,
                Some(Icon::Record),
                lang.pick("Commencer sans enregistrer", "Start without saving"),
            )
            .clicked()
            {
                choice = 2;
            }
            if widgets::button(ui, Kind::Ghost, None, lang.pick("Annuler", "Cancel")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                choice = 3;
            }
        },
    );
    match choice {
        1 => {
            app.confirm_new = false;
            app.save();
        }
        2 => app.start(),
        3 => app.confirm_new = false,
        _ => {}
    }
}

/// The export running in the background, with its current phase.
pub fn export_progress(app: &App, ctx: &egui::Context, activity: Option<&str>) {
    if !app.is_exporting() {
        return;
    }
    let lang = app.lang;
    let p = pal(ctx);
    egui::Modal::new(Id::unique("export_progress"))
        .frame(widgets_card_frame(ctx).inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(22.0).color(p.accent));
                ui.label(
                    RichText::new(lang.pick("Génération du rapport…", "Generating report…"))
                        .size(18.0)
                        .strong()
                        .color(p.text),
                );
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(activity.unwrap_or(lang.pick("Préparation…", "Preparing…")))
                    .color(p.text_muted),
            );
            if let Some(dest) = &app.export_dest {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    icons::icon(ui, Icon::Save, 14.0, p.text_faint);
                    ui.add(
                        egui::Label::new(
                            RichText::new(dest.display().to_string())
                                .size(theme::SIZE_SMALL)
                                .color(p.text_faint),
                        )
                        .truncate(),
                    );
                });
            }
        });
}

/// The selected screenshot over the whole window (Esc or the close button).
pub fn enlarged(app: &mut App, ctx: &egui::Context) {
    let Some(mut viewer) = app.enlarged.take() else {
        return;
    };
    let Some(snap) = app.snapshot.clone() else {
        return;
    };
    let row = app
        .selected
        .and_then(|s| snap.position(s))
        .or_else(|| snap.rows.len().checked_sub(1))
        .map(|i| &snap.rows[i]);
    let Some(name) = row.and_then(|r| r.screenshot.clone()) else {
        return;
    };
    let Some(tex) = app.textures.full(ctx, &snap.dir, &name) else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    let screen = ctx.content_rect();
    egui::Area::new(Id::unique("enlarged"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            ui.painter().rect_filled(screen, 0.0, p.bg);
            let inner = screen.shrink(12.0);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            child.horizontal(|ui| {
                if widgets::button(ui, Kind::Secondary, None, lang.pick("Fermer", "Close"))
                    .on_hover_text(lang.pick("Échap", "Esc"))
                    .clicked()
                {
                    close = true;
                }
                if let Some(row) = row {
                    ui.label(
                        RichText::new(format!(
                            "{} {} · {}",
                            lang.pick("Étape", "Step"),
                            row.index,
                            row.description(lang)
                        ))
                        .color(p.text_muted),
                    );
                }
            });
            child.add_space(4.0);
            viewer.show(&mut child, &tex, lang, false);
        });
    if !close {
        app.enlarged = Some(viewer);
    }
}
