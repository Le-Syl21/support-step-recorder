//! The start screen and the bands above the steps: the live recording
//! banner and the editor header with the project and export actions.

use eframe::egui;
use egui::{Margin, RichText, Stroke};

use crate::app::{App, Intent, Status};
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
                        recovery(app, ui);
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
                        feature(ui, Icon::Pencil, lang.pick(
                            "À l'arrêt, vous arrivez dans l'éditeur : renommez, décrivez, réordonnez ou supprimez les étapes, et combinez plusieurs enregistrements.",
                            "When you stop, you land in the editor: rename, describe, reorder or delete steps, and combine several recordings.",
                        ));
                        feature(ui, Icon::Report, lang.pick(
                            "Enregistrez le projet (.ssrproj) pour le reprendre plus tard, puis exportez un ZIP avec un rapport HTML à envoyer.",
                            "Save the project (.ssrproj) to pick it up later, then export a ZIP with an HTML report to send.",
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
                                app.request(Intent::NewRecording);
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if widgets::button(
                                ui,
                                Kind::Secondary,
                                Some(Icon::Folder),
                                lang.pick("Ouvrir un projet…", "Open project…"),
                            )
                            .on_hover_text(lang.pick(
                                "Reprendre un guide enregistré (.ssrproj).",
                                "Continue a saved guide (.ssrproj).",
                            ))
                            .clicked()
                            {
                                app.request(Intent::OpenProject);
                            }
                            if widgets::button(
                                ui,
                                Kind::Secondary,
                                Some(Icon::Import),
                                lang.pick("Ouvrir un enregistrement…", "Open recording…"),
                            )
                            .on_hover_text(lang.pick(
                                "Éditer un ZIP exporté plus tôt ; l'archive d'origine n'est pas modifiée.",
                                "Edit a ZIP exported earlier; the original archive is not modified.",
                            ))
                            .clicked()
                            {
                                app.request(Intent::OpenRecording);
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(lang.pick(
                                "Les captures restent sur cet ordinateur, dans un dossier privé, jusqu'à ce que vous enregistriez le projet ou exportiez le guide. Sous Wayland, le bureau vous demandera quelle fenêtre ou quel écran partager.",
                                "Screenshots stay on this computer, in a private folder, until you save the project or export the guide. On Wayland, your desktop will ask which window or screen to share.",
                            ))
                            .size(theme::SIZE_SMALL)
                            .color(p.text_faint),
                        );


                        if let Status::StartFailed(err) | Status::OpenFailed(err) = &app.status {
                            ui.add_space(12.0);
                            banner(ui, Icon::Warning, p.record, &format!(
                                "{} {err}",
                                lang.pick("Impossible :", "Could not proceed:")
                            ));
                        }
                        });
                    });
            });
        });
    });
}

/// Guides left unsaved by a crash: recover or discard each.
fn recovery(app: &mut App, ui: &mut egui::Ui) {
    if app.recoverable.is_empty() {
        return;
    }
    let lang = app.lang;
    let p = pal(ui.ctx());
    let mut action = None;
    egui::Frame::new()
        .fill(p.tint(p.warning, 24))
        .stroke(Stroke::new(1.0, p.tint(p.warning, 80)))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_SMALL))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::icon(ui, Icon::Warning, 16.0, p.warning);
                ui.label(
                    RichText::new(
                        lang.pick("Travail non enregistré retrouvé", "Unsaved work found"),
                    )
                    .strong()
                    .color(p.text),
                );
            });
            for r in &app.recoverable {
                ui.horizontal_wrapped(|ui| {
                    let when = chrono::DateTime::parse_from_rfc3339(&r.written_at)
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default();
                    ui.label(
                        RichText::new(format!(
                            "« {} » · {} {} · {when}",
                            r.title,
                            r.step_count,
                            lang.pick("étape(s)", "step(s)")
                        ))
                        .color(p.text),
                    );
                    if widgets::button(ui, Kind::Primary, None, lang.pick("Récupérer", "Recover"))
                        .clicked()
                    {
                        action = Some((true, r.dir.clone()));
                    }
                    if widgets::button(
                        ui,
                        Kind::Ghost,
                        Some(Icon::Trash),
                        lang.pick("Supprimer", "Discard"),
                    )
                    .clicked()
                    {
                        action = Some((false, r.dir.clone()));
                    }
                });
            }
        });
    ui.add_space(14.0);
    match action {
        Some((true, dir)) => app.request(Intent::Recover(dir)),
        Some((false, dir)) => app.discard_recoverable(&dir),
        None => {}
    }
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

/// Editor: the guide title, its saved / exported state, and the actions.
pub fn review_header(app: &mut App, root: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(root.ctx());
    let Some(doc) = &app.doc else { return };
    let title = doc.metadata().title.clone();
    let count = doc.steps().len();
    let undo = doc.can_undo();
    let redo = doc.can_redo();
    let editable = app.can_edit();
    // French labels are longer: fold the secondary actions into "More…"
    // sooner so Undo/Redo always stay visible.
    let compact = root.available_width()
        < match lang {
            ssr_core::Lang::Fr => 1320.0,
            ssr_core::Lang::En => 1180.0,
        };

    #[derive(Clone, Copy, PartialEq)]
    enum Act {
        Undo,
        Redo,
        Import,
        AddImage,
        AddStep,
        Save,
        SaveAs,
        Export,
        Info,
        New,
        OpenProject,
        OpenRecording,
        Close,
    }
    let act: std::cell::Cell<Option<Act>> = std::cell::Cell::new(None);

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
                        let steps_word = if count == 1 {
                            lang.pick("étape", "step")
                        } else {
                            lang.pick("étapes", "steps")
                        };
                        ui.horizontal(|ui| {
                            widgets::eyebrow(
                                ui,
                                &format!(
                                    "{} · {count} {steps_word}",
                                    lang.pick("Guide", "Guide")
                                ),
                            );
                            save_state(app, ui);
                        });
                        ui.horizontal(|ui| {
                            let r = ui.add(
                                egui::Label::new(
                                    RichText::new(&title).size(17.0).strong().color(p.text),
                                )
                                .truncate()
                                .sense(egui::Sense::click()),
                            );
                            if editable
                                && (r
                                    .on_hover_text(lang.pick(
                                        "Modifier le titre et l'introduction",
                                        "Edit the title and introduction",
                                    ))
                                    .clicked()
                                    || widgets::icon_button(
                                        ui,
                                        Icon::Pencil,
                                        lang.pick("Détails du guide…", "Guide details…"),
                                    )
                                    .clicked())
                            {
                                act.set(Some(Act::Info));
                            }
                        });
                    });
                },
                |ui| {
                    ui.add_enabled_ui(!app.is_busy(), |ui| {
                        // Laid out right to left.
                        ui.menu_button(lang.pick("Plus…", "More…"), |ui| {
                            let item = |ui: &mut egui::Ui, a: Act, label: &str| {
                                if ui.button(label).clicked() {
                                    act.set(Some(a));
                                    ui.close();
                                }
                            };
                            item(ui, Act::SaveAs, lang.pick("Enregistrer le projet sous…", "Save project as…"));
                            item(ui, Act::Info, lang.pick("Détails du guide…", "Guide details…"));
                            if compact {
                                ui.separator();
                                item(ui, Act::Import, lang.pick("Importer des étapes…", "Import steps…"));
                                item(ui, Act::AddImage, lang.pick("Ajouter une image…", "Add image…"));
                                item(ui, Act::AddStep, lang.pick("Ajouter une étape rédigée", "Add written step"));
                                item(ui, Act::New, lang.pick("Nouvel enregistrement", "New recording"));
                            }
                            ui.separator();
                            item(ui, Act::OpenProject, lang.pick("Ouvrir un projet…", "Open project…"));
                            item(ui, Act::OpenRecording, lang.pick("Ouvrir un enregistrement…", "Open recording…"));
                            item(ui, Act::Close, lang.pick("Fermer le guide", "Close guide"));
                        })
                        .response
                        .on_hover_text(lang.pick("Plus d'actions", "More actions"));
                        if !compact
                            && widgets::button(
                                ui,
                                Kind::Secondary,
                                Some(Icon::Record),
                                lang.pick("Nouvel enregistrement", "New recording"),
                            )
                            .clicked()
                        {
                            act.set(Some(Act::New));
                        }
                        if widgets::button(
                            ui,
                            Kind::Secondary,
                            Some(Icon::Report),
                            lang.pick("Exporter le guide…", "Export guide…"),
                        )
                        .on_hover_text(lang.pick(
                            "ZIP à partager : rapport HTML, steps.json et captures (Ctrl+E). Ne remplace pas l'enregistrement du projet.",
                            "ZIP to share: HTML report, steps.json and screenshots (Ctrl+E). Does not replace saving the project.",
                        ))
                        .clicked()
                        {
                            act.set(Some(Act::Export));
                        }
                        let kind = if app.is_dirty() { Kind::Primary } else { Kind::Secondary };
                        if widgets::button(
                            ui,
                            kind,
                            Some(Icon::Save),
                            lang.pick("Enregistrer le projet", "Save project"),
                        )
                        .on_hover_text(lang.pick(
                            "Projet éditable .ssrproj, pour reprendre plus tard (Ctrl+S).",
                            "Editable .ssrproj project, to continue later (Ctrl+S).",
                        ))
                        .clicked()
                        {
                            act.set(Some(Act::Save));
                        }
                        if !compact {
                            ui.add_enabled_ui(editable, |ui| {
                                if widgets::button(
                                    ui,
                                    Kind::Ghost,
                                    Some(Icon::Image),
                                    lang.pick("Image…", "Image…"),
                                )
                                .on_hover_text(lang.pick(
                                    "Ajouter une image PNG, JPEG ou WebP",
                                    "Add a PNG, JPEG or WebP image",
                                ))
                                .clicked()
                                {
                                    act.set(Some(Act::AddImage));
                                }
                                if widgets::button(
                                    ui,
                                    Kind::Ghost,
                                    Some(Icon::Plus),
                                    lang.pick("Étape", "Step"),
                                )
                                .on_hover_text(lang.pick(
                                    "Ajouter une étape rédigée après la sélection",
                                    "Add a written step after the selection",
                                ))
                                .clicked()
                                {
                                    act.set(Some(Act::AddStep));
                                }
                                if widgets::button(
                                    ui,
                                    Kind::Ghost,
                                    Some(Icon::Import),
                                    lang.pick("Importer…", "Import…"),
                                )
                                .on_hover_text(lang.pick(
                                    "Importer des étapes d'autres enregistrements (ZIP ou .ssrproj)",
                                    "Import steps from other recordings (ZIP or .ssrproj)",
                                ))
                                .clicked()
                                {
                                    act.set(Some(Act::Import));
                                }
                            });
                        }
                        ui.add_space(6.0);
                        let redo_tip = match redo {
                            Some(k) => format!("{} {} (Ctrl+Shift+Z)", lang.pick("Rétablir :", "Redo:"), k.label(lang)),
                            None => lang.pick("Rien à rétablir", "Nothing to redo").into(),
                        };
                        ui.add_enabled_ui(editable && redo.is_some(), |ui| {
                            if widgets::icon_button(ui, Icon::Redo, &redo_tip).clicked() {
                                act.set(Some(Act::Redo));
                            }
                        });
                        let undo_tip = match undo {
                            Some(k) => format!("{} {} (Ctrl+Z)", lang.pick("Annuler :", "Undo:"), k.label(lang)),
                            None => lang.pick("Rien à annuler", "Nothing to undo").into(),
                        };
                        ui.add_enabled_ui(editable && undo.is_some(), |ui| {
                            if widgets::icon_button(ui, Icon::Undo, &undo_tip).clicked() {
                                act.set(Some(Act::Undo));
                            }
                        });
                    });
                },
            );
        });

    match act.get() {
        Some(Act::Undo) => app.undo(),
        Some(Act::Redo) => app.redo(),
        Some(Act::Import) => app.import_recordings(),
        Some(Act::AddImage) => app.add_image(),
        Some(Act::AddStep) => app.add_written_step(),
        Some(Act::Save) => {
            app.commit_draft();
            app.save_project(false);
        }
        Some(Act::SaveAs) => {
            app.commit_draft();
            app.save_project(true);
        }
        Some(Act::Export) => {
            app.commit_draft();
            app.export_guide();
        }
        Some(Act::Info) => {
            if let Some(d) = &app.doc {
                app.guide_info =
                    Some((d.metadata().title.clone(), d.metadata().description.clone()));
            }
        }
        Some(Act::New) => app.request(Intent::NewRecording),
        Some(Act::OpenProject) => app.request(Intent::OpenProject),
        Some(Act::OpenRecording) => app.request(Intent::OpenRecording),
        Some(Act::Close) => app.request(Intent::CloseGuide),
        None => {}
    }
}

/// The pill telling whether the guide is saved, and where; and whether the
/// current state was exported.
fn save_state(app: &App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let p = pal(ui.ctx());
    let Some(doc) = &app.doc else { return };
    if app.is_busy() {
        widgets::pill(ui, None, lang.pick("En cours…", "Working…"), p.accent_text);
        return;
    }
    match (doc.is_dirty(), doc.project_path()) {
        (false, Some(path)) => {
            widgets::pill(
                ui,
                Some(Icon::Check),
                lang.pick("Enregistré", "Saved"),
                p.success,
            )
            .on_hover_text(path.display().to_string());
        }
        (_, path) => {
            widgets::pill(
                ui,
                Some(Icon::Warning),
                lang.pick("Modifications non enregistrées", "Unsaved changes"),
                p.warning,
            )
            .on_hover_text(match path {
                Some(p) => format!(
                    "{} {}",
                    lang.pick("Projet :", "Project:"),
                    p.display()
                ),
                None => lang
                    .pick(
                        "Ce guide n'a pas encore de fichier projet : enregistrez-le pour le reprendre plus tard.",
                        "This guide has no project file yet: save it to continue later.",
                    )
                    .into(),
            });
        }
    }
    if let Some(path) = doc.export_path() {
        let (text, color) = if doc.is_exported() {
            (lang.pick("Exporté", "Exported"), p.success)
        } else {
            (
                lang.pick("Export à refaire", "Export outdated"),
                p.text_faint,
            )
        };
        widgets::pill(ui, Some(Icon::Report), text, color)
            .on_hover_text(path.display().to_string());
    }
    if let Status::ExportFailed(err) | Status::SaveFailed(err) = &app.status {
        widgets::pill(
            ui,
            Some(Icon::Warning),
            lang.pick("Échec", "Failed"),
            p.record,
        )
        .on_hover_text(err);
    }
}
