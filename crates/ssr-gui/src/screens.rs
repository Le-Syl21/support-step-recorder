//! The start screen and the bands above the steps: the live recording
//! banner and the review header with the save actions.

use eframe::egui;
use egui::{Margin, RichText, Stroke};

use crate::app::{App, Status};
use crate::icons::{self, Icon};
use crate::steps;
use crate::theme::{self, pal};
use crate::widgets::{self, Kind};

/// Idle: what the tool does, and one obvious Record button.
pub fn idle(app: &mut App, root: &mut egui::Ui) {
    let lang = app.lang;
    egui::CentralPanel::default().show(root, |ui| {
        let p = pal(ui.ctx());
        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width().min(560.0);
            let spare = ui.available_height() - 470.0;
            ui.add_space((spare / 2.0).clamp(12.0, 120.0));
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                widgets::card(ui)
                    .inner_margin(Margin::same(28))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // The card reads left-aligned inside the centred column.
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        widgets::eyebrow(ui, lang.pick("Nouvel enregistrement", "New recording"));
                        ui.label(
                            RichText::new(lang.pick(
                                "Montrez ce qui se passe, clic par clic",
                                "Show what happens, click by click",
                            ))
                            .size(theme::SIZE_TITLE)
                            .strong()
                            .color(p.text),
                        );
                        ui.label(RichText::new(lang.pick(
                            "Support Step Recorder note chacune de vos actions pour que le support voie exactement ce qui s'est passé.",
                            "Support Step Recorder notes each of your actions so support can see exactly what happened.",
                        )).color(p.text_muted));
                        ui.add_space(14.0);

                        feature(ui, Icon::Mouse, lang.pick(
                            "Chaque clic prend une capture de la fenêtre, avec le bouton et la position.",
                            "Every click takes a screenshot of the window, with the button and the position.",
                        ));
                        feature(ui, Icon::Keyboard, lang.pick(
                            "Le texte tapé entre deux clics est enregistré comme une étape.",
                            "Text typed between two clicks is recorded as a step.",
                        ));
                        feature(ui, Icon::Report, lang.pick(
                            "À l'arrêt, vous relisez les étapes puis enregistrez un ZIP avec un rapport HTML à envoyer.",
                            "When you stop, you review the steps and save a ZIP with an HTML report to send.",
                        ));

                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            if widgets::big_button(
                                ui,
                                Kind::Record,
                                Some(Icon::Record),
                                lang.pick("Démarrer l'enregistrement", "Start recording"),
                            )
                            .clicked()
                            {
                                app.start();
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(lang.pick(
                                "Les captures restent sur cet ordinateur, dans un dossier temporaire, jusqu'à ce que vous enregistriez le rapport. Sous Wayland, le bureau vous demandera quelle fenêtre ou quel écran partager.",
                                "Screenshots stay on this computer, in a temporary folder, until you save the report. On Wayland, your desktop will ask which window or screen to share.",
                            ))
                            .size(theme::SIZE_SMALL)
                            .color(p.text_faint),
                        );

                        if let Status::StartFailed(err) = &app.status {
                            ui.add_space(12.0);
                            banner(ui, Icon::Warning, p.record, &format!(
                                "{} {err}",
                                lang.pick("Impossible de démarrer :", "Could not start:")
                            ));
                        }
                        });
                    });
            });
        });
    });
}

fn feature(ui: &mut egui::Ui, icon: Icon, text: &str) {
    let p = pal(ui.ctx());
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(30.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), p.tint(p.accent, 28));
        icon.paint(ui.painter(), rect.shrink(7.0), p.accent_text);
        ui.add(egui::Label::new(RichText::new(text).color(p.text)).wrap());
    });
    ui.add_space(2.0);
}

/// A tinted line with an icon: an error or a notice.
fn banner(ui: &mut egui::Ui, icon: Icon, color: egui::Color32, text: &str) {
    let p = pal(ui.ctx());
    egui::Frame::new()
        .fill(p.tint(color, 30))
        .stroke(Stroke::new(1.0, p.tint(color, 90)))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SMALL))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::icon(ui, icon, 16.0, color);
                ui.add(egui::Label::new(RichText::new(text).color(p.text)).wrap());
            });
        });
}

/// Recording: a red band with the live state, elapsed time, step count and Stop.
pub fn recording_banner(app: &mut App, root: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(root.ctx());
    let (elapsed, count) = app
        .snapshot
        .as_ref()
        .map(|s| {
            let secs = (chrono::Local::now() - s.started_at).num_seconds().max(0) as u64;
            (secs, s.rows.len())
        })
        .unwrap_or((0, 0));
    let backend = app.backend_name().unwrap_or("");

    egui::Panel::top("stage_band")
        .frame(
            egui::Frame::new()
                .fill(p.tint(p.record, 26))
                .stroke(Stroke::new(1.0, p.tint(p.record, 80)))
                .inner_margin(Margin::symmetric(18, 12)),
        )
        .show(root, |ui| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    widgets::live_dot(ui, 22.0, p.record);
                    ui.label(
                        RichText::new(lang.pick("Enregistrement", "Recording"))
                            .size(18.0)
                            .strong()
                            .color(p.record),
                    );
                    ui.add_space(8.0);
                    stat(ui, Icon::Clock, &steps::clock(elapsed), lang.pick("Durée", "Elapsed"));
                    stat(
                        ui,
                        Icon::Camera,
                        &count.to_string(),
                        lang.pick("Étapes", "Steps"),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(lang.pick(
                                "Utilisez l'application à documenter, puis revenez ici pour arrêter.",
                                "Use the application you want to document, then come back here to stop.",
                            ))
                            .size(theme::SIZE_SMALL)
                            .color(p.text_muted),
                        )
                        .truncate(),
                    )
                    .on_hover_text(format!(
                        "{} {backend}",
                        lang.pick("Capture des entrées :", "Input capture:")
                    ));
                },
                |ui| {
                    if widgets::big_button(
                        ui,
                        Kind::Record,
                        Some(Icon::Stop),
                        lang.pick("Arrêter", "Stop"),
                    )
                    .clicked()
                    {
                        app.stop();
                    }
                },
            );
        });
}

/// A figure with its icon and a caption in a tooltip.
fn stat(ui: &mut egui::Ui, icon: Icon, value: &str, caption: &str) {
    let p = pal(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        icons::icon(ui, icon, 16.0, p.text_muted);
        ui.label(
            RichText::new(value)
                .monospace()
                .size(16.0)
                .strong()
                .color(p.text),
        );
    })
    .response
    .on_hover_text(caption);
    ui.add_space(6.0);
}

/// Review: the session summary, whether it is saved, and the actions.
pub fn review_header(app: &mut App, root: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(root.ctx());
    let (count, duration, started) = app
        .snapshot
        .as_ref()
        .map(|s| {
            let last = s.rows.last().map(|r| r.elapsed_ms).unwrap_or(0);
            let started = s.started_at.format(match lang {
                ssr_core::Lang::Fr => "%d/%m/%Y %H:%M",
                ssr_core::Lang::En => "%Y-%m-%d %H:%M",
            });
            (s.rows.len(), (last / 1000) as u64, started.to_string())
        })
        .unwrap_or_default();
    let busy = app.is_exporting();
    let saved = app.saved_to.is_some();
    let (mut save, mut new) = (false, false);

    egui::Panel::top("stage_band")
        .frame(
            egui::Frame::new()
                .fill(p.bar)
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(18, 10)),
        )
        .show(root, |ui| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        widgets::eyebrow(
                            ui,
                            &format!("{} · {started}", lang.pick("Session", "Session")),
                        );
                        ui.horizontal(|ui| {
                            let steps_word = if count == 1 {
                                lang.pick("étape", "step")
                            } else {
                                lang.pick("étapes", "steps")
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{count} {steps_word} · {}",
                                    steps::clock(duration)
                                ))
                                .size(17.0)
                                .strong()
                                .color(p.text),
                            );
                            save_state(app, ui);
                        });
                    });
                },
                |ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        let (kind, label) = if saved {
                            (
                                Kind::Secondary,
                                lang.pick("Enregistrer une copie…", "Save a copy…"),
                            )
                        } else {
                            (
                                Kind::Primary,
                                lang.pick("Enregistrer le rapport…", "Save report…"),
                            )
                        };
                        save = widgets::button(ui, kind, Some(Icon::Save), label).clicked();
                        new = widgets::button(
                            ui,
                            Kind::Secondary,
                            Some(Icon::Record),
                            lang.pick("Nouvel enregistrement", "New recording"),
                        )
                        .clicked();
                    });
                },
            );
        });
    if save {
        app.save();
    }
    if new {
        app.request_start();
    }
}

/// The pill telling whether the session is saved, and where.
fn save_state(app: &App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(ui.ctx());
    if app.is_exporting() {
        widgets::pill(
            ui,
            None,
            lang.pick("Enregistrement…", "Saving…"),
            p.accent_text,
        );
    } else if let Some(path) = &app.saved_to {
        widgets::pill(
            ui,
            Some(Icon::Check),
            lang.pick("Enregistré", "Saved"),
            p.success,
        )
        .on_hover_text(path.display().to_string());
        ui.add(
            egui::Label::new(
                RichText::new(path.display().to_string())
                    .size(theme::SIZE_SMALL)
                    .color(p.text_faint),
            )
            .truncate(),
        );
    } else if let Status::ExportFailed(err) = &app.status {
        widgets::pill(
            ui,
            Some(Icon::Warning),
            lang.pick("Échec de l'export", "Export failed"),
            p.record,
        )
        .on_hover_text(err);
    } else {
        widgets::pill(
            ui,
            Some(Icon::Warning),
            lang.pick("Non enregistré", "Not saved"),
            p.warning,
        )
        .on_hover_text(lang.pick(
            "Les captures sont dans un dossier temporaire : enregistrez le rapport pour les garder.",
            "The screenshots are in a temporary folder: save the report to keep them.",
        ));
    }
}
