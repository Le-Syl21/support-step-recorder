//! Modal layers: the unsaved-changes question, the privacy notice, the
//! guide details, "replace or insert?" for an added image, warnings, the
//! background-work progress and the enlarged screenshot.

use eframe::egui;
use egui::{Id, Margin, RichText, Sense};

use crate::app::{App, Intent};
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

/// "Save changes?" before something replaces or closes the guide.
pub fn unsaved_changes(app: &mut App, ctx: &egui::Context) {
    let Some(intent) = app.pending.clone() else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    let what = match intent {
        Intent::NewRecording => lang.pick(
            "Un nouvel enregistrement fermera ce guide.",
            "A new recording will close this guide.",
        ),
        Intent::OpenProject | Intent::OpenRecording | Intent::Recover(_) => lang.pick(
            "Ouvrir un autre guide fermera celui-ci.",
            "Opening another guide will close this one.",
        ),
        Intent::CloseGuide => lang.pick("Ce guide va être fermé.", "This guide will be closed."),
        Intent::Quit => lang.pick(
            "L'application va se fermer.",
            "The application is about to close.",
        ),
    };
    let message = format!(
        "{what} {}",
        lang.pick(
            "Ses modifications ne sont pas enregistrées dans un projet (un export ZIP ne suffit pas pour reprendre l'édition).",
            "Its changes are not saved to a project (an exported ZIP is not enough to keep editing).",
        )
    );
    #[derive(PartialEq)]
    enum Choice {
        None,
        Save,
        Discard,
        Cancel,
    }
    let choice = modal(
        ctx,
        "unsaved_changes",
        Icon::Warning,
        p.warning,
        lang.pick("Enregistrer les modifications ?", "Save changes?"),
        &message,
        |ui| {
            if widgets::button(
                ui,
                Kind::Primary,
                Some(Icon::Save),
                lang.pick("Enregistrer le projet…", "Save project…"),
            )
            .clicked()
            {
                return Choice::Save;
            }
            if widgets::button(
                ui,
                Kind::Secondary,
                Some(Icon::Trash),
                lang.pick("Ne pas enregistrer", "Don't save"),
            )
            .clicked()
            {
                return Choice::Discard;
            }
            if widgets::button(ui, Kind::Ghost, None, lang.pick("Annuler", "Cancel")).clicked() {
                return Choice::Cancel;
            }
            Choice::None
        },
    );
    let choice = if choice == Choice::None && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        Choice::Cancel
    } else {
        choice
    };
    match choice {
        Choice::Save => app.save_then(intent),
        Choice::Discard => app.perform(intent),
        Choice::Cancel => app.pending = None,
        Choice::None => {}
    }
}

/// Before the first recording: what is captured, and where it stays.
pub fn privacy(app: &mut App, ctx: &egui::Context) {
    if !app.privacy_prompt {
        return;
    }
    let lang = app.lang;
    let p = pal(ctx);
    let mut choice = 0;
    modal(
        ctx,
        "privacy",
        Icon::Keyboard,
        p.warning,
        lang.pick("Avant d'enregistrer", "Before recording"),
        lang.pick(
            "L'enregistreur capture une image de la fenêtre à chaque clic et le texte que vous tapez entre deux clics — y compris un mot de passe tapé par erreur. Rien ne quitte cet ordinateur. Après l'arrêt, relisez les étapes : vous pouvez masquer un texte saisi, retirer une capture ou supprimer une étape avant d'exporter. Sur un poste géré, vérifiez que la capture clavier est autorisée.",
            "The recorder takes a picture of the window at each click and keeps the text you type between clicks — including a password typed by mistake. Nothing leaves this computer. After stopping, review the steps: you can redact typed text, remove a screenshot or delete a step before exporting. On a managed workstation, check that keyboard capture is allowed.",
        ),
        |ui| {
            if widgets::button(
                ui,
                Kind::Record,
                Some(Icon::Record),
                lang.pick("J'ai compris, enregistrer", "I understand, record"),
            )
            .clicked()
            {
                choice = 1;
            }
            if widgets::button(ui, Kind::Ghost, None, lang.pick("Annuler", "Cancel")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                choice = 2;
            }
        },
    );
    match choice {
        1 => app.accept_privacy(),
        2 => app.privacy_prompt = false,
        _ => {}
    }
}

/// The guide's title and introduction.
pub fn guide_info(app: &mut App, ctx: &egui::Context) {
    let Some((mut title, mut intro)) = app.guide_info.take() else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    let mut choice = 0;
    egui::Modal::new(Id::unique("guide_info"))
        .frame(widgets_card_frame(ctx).inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(
                RichText::new(lang.pick("Détails du guide", "Guide details"))
                    .size(18.0)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(8.0);
            widgets::eyebrow(ui, lang.pick("Titre", "Title"));
            ui.add(egui::TextEdit::singleline(&mut title).desired_width(f32::INFINITY));
            ui.add_space(6.0);
            widgets::eyebrow(
                ui,
                lang.pick("Introduction (facultative)", "Introduction (optional)"),
            );
            ui.add(
                egui::TextEdit::multiline(&mut intro)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::button(
                    ui,
                    Kind::Primary,
                    Some(Icon::Check),
                    lang.pick("Valider", "Apply"),
                )
                .clicked()
                {
                    choice = 1;
                }
                if widgets::button(ui, Kind::Ghost, None, lang.pick("Annuler", "Cancel")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    choice = 2;
                }
            });
        });
    match choice {
        1 => {
            app.edit(|d| d.set_guide_info(title, intro));
        }
        2 => {}
        _ => app.guide_info = Some((title, intro)),
    }
}

/// An image was added while a step is selected: replace its screenshot, or
/// make a new step?
pub fn image_choice(app: &mut App, ctx: &egui::Context) {
    let Some(pending) = app.pending_image.clone() else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    let number = app
        .doc
        .as_ref()
        .and_then(|d| d.position(pending.target))
        .map(|i| i + 1)
        .unwrap_or(0);
    let has_shot = app
        .doc
        .as_ref()
        .and_then(|d| d.step(pending.target))
        .is_some_and(|s| s.screenshot.is_some());
    let mut choice = 0;
    modal(
        ctx,
        "image_choice",
        Icon::Image,
        p.accent_text,
        lang.pick("Utiliser cette image pour…", "Use this image for…"),
        lang.pick(
            "Le fichier d'origine n'est pas modifié : une copie est ajoutée au guide.",
            "The original file is not modified: a copy is added to the guide.",
        ),
        |ui| {
            let replace = if has_shot {
                format!(
                    "{} {number}",
                    lang.pick(
                        "Remplacer la capture de l'étape",
                        "Replace the screenshot of step"
                    )
                )
            } else {
                format!(
                    "{} {number}",
                    lang.pick("L'ajouter à l'étape", "Attach it to step")
                )
            };
            if widgets::button(ui, Kind::Primary, Some(Icon::Image), &replace).clicked() {
                choice = 1;
            }
            if widgets::button(
                ui,
                Kind::Secondary,
                Some(Icon::Plus),
                &format!(
                    "{} {number}",
                    lang.pick("Nouvelle étape après l'étape", "New step after step")
                ),
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
            app.pending_image = None;
            app.edit(|d| d.attach_screenshot(pending.target, pending.asset));
        }
        2 => {
            app.pending_image = None;
            let title = app.t("Nouvelle étape", "New step");
            if let Some(id) = app.edit(|d| {
                let pos = d
                    .position(pending.target)
                    .map(|p| p + 1)
                    .unwrap_or(d.steps().len());
                d.add_manual_step(pos, title, Some(pending.asset))
            }) {
                app.select(id);
            }
        }
        3 => app.pending_image = None,
        _ => {}
    }
}

/// A list of warnings to acknowledge.
pub fn notice(app: &mut App, ctx: &egui::Context) {
    let Some((title, lines)) = app.notice.clone() else {
        return;
    };
    let p = pal(ctx);
    let mut close = false;
    egui::Modal::new(Id::unique("notice"))
        .frame(widgets_card_frame(ctx).inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.horizontal(|ui| {
                icons::icon(ui, Icon::Warning, 20.0, p.warning);
                ui.label(RichText::new(&title).size(18.0).strong().color(p.text));
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for l in &lines {
                        ui.add(egui::Label::new(RichText::new(l).color(p.text_muted)).wrap());
                    }
                });
            ui.add_space(12.0);
            if widgets::button(ui, Kind::Primary, None, "OK").clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter))
            {
                close = true;
            }
        });
    if close {
        app.notice = None;
    }
}

/// A save, export, open or import running in the background.
pub fn job_progress(app: &App, ctx: &egui::Context, activity: Option<&str>) {
    let Some((fr, en)) = app.job_title() else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    egui::Modal::new(Id::unique("job_progress"))
        .frame(widgets_card_frame(ctx).inner_margin(Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(22.0).color(p.accent));
                ui.label(
                    RichText::new(lang.pick(fr, en))
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
            if let Some(dest) = app.job_dest() {
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
    let row = snap.current(app.selected).map(|i| &snap.rows[i]);
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
