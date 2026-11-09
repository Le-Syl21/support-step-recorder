//! The import dialog: the recordings just read, each with its steps
//! (thumbnail, title, checkbox), and where to insert the chosen ones.
//! Nothing touches the guide until "Import" is pressed.

use eframe::egui;
use egui::{Id, Margin, RichText, Vec2};
use ssr_core::import::ImportedRecording;

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{self, pal};
use crate::widgets::{self, Kind};

/// Where the chosen steps go.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Position {
    Append,
    AfterSelected,
    Start,
}

pub struct ImportDialog {
    pub recordings: Vec<ImportedRecording>,
    /// `checked[r][s]`: step `s` of recording `r` is chosen.
    pub checked: Vec<Vec<bool>>,
    pub position: Position,
}

impl ImportDialog {
    pub fn new(recordings: Vec<ImportedRecording>) -> Self {
        let checked = recordings
            .iter()
            .map(|r| vec![true; r.steps.len()])
            .collect();
        Self {
            recordings,
            checked,
            position: Position::Append,
        }
    }

    /// The chosen step indices, per recording.
    pub fn picks(&self) -> Vec<Vec<usize>> {
        self.checked
            .iter()
            .map(|c| {
                c.iter()
                    .enumerate()
                    .filter(|(_, &on)| on)
                    .map(|(i, _)| i)
                    .collect()
            })
            .collect()
    }

    fn count(&self) -> usize {
        self.checked.iter().flatten().filter(|&&c| c).count()
    }
}

pub fn import_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut dialog) = app.import.take() else {
        return;
    };
    let lang = app.lang;
    let p = pal(ctx);
    let dir = app
        .doc
        .as_ref()
        .map(|d| d.asset_root().to_path_buf())
        .unwrap_or_default();
    let has_selection = app.current_id().is_some();
    #[derive(PartialEq)]
    enum Choice {
        None,
        Import,
        Cancel,
    }
    let mut choice = Choice::None;

    let screen = ctx.content_rect();
    egui::Modal::new(Id::unique("import_dialog"))
        .frame(
            egui::Frame::new()
                .fill(p.surface)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(egui::CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::same(20)),
        )
        .show(ctx, |ui| {
            ui.set_width((screen.width() - 80.0).clamp(320.0, 760.0));
            ui.horizontal(|ui| {
                icons::icon(ui, Icon::Import, 22.0, p.accent_text);
                ui.label(
                    RichText::new(lang.pick("Importer des étapes", "Import steps"))
                        .size(18.0)
                        .strong()
                        .color(p.text),
                );
            });
            ui.label(
                RichText::new(lang.pick(
                    "Cochez les étapes à copier dans le guide. Les archives d'origine ne sont pas modifiées.",
                    "Tick the steps to copy into the guide. The original archives are not modified.",
                ))
                .color(p.text_muted),
            );
            ui.add_space(8.0);

            let list_h = (screen.height() - 300.0).clamp(160.0, 520.0);
            egui::ScrollArea::vertical()
                .max_height(list_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (r, rec) in dialog.recordings.iter().enumerate() {
                        let checked = &mut dialog.checked[r];
                        widgets::card(ui)
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    icons::icon(ui, Icon::Report, 16.0, p.text_muted);
                                    let title = rec.title.as_deref().unwrap_or(&rec.label);
                                    ui.label(RichText::new(title).strong().color(p.text));
                                    ui.label(
                                        RichText::new(format!(
                                            "· {} {}",
                                            rec.steps.len(),
                                            lang.pick("étape(s)", "step(s)")
                                        ))
                                        .color(p.text_faint),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button(lang.pick("Aucune", "None")).clicked() {
                                                checked.iter_mut().for_each(|c| *c = false);
                                            }
                                            if ui.small_button(lang.pick("Toutes", "All")).clicked() {
                                                checked.iter_mut().for_each(|c| *c = true);
                                            }
                                        },
                                    );
                                });
                                for (s, staged) in rec.steps.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut checked[s], "");
                                        let size = Vec2::new(64.0, 40.0);
                                        let thumb = staged
                                            .step
                                            .screenshot
                                            .as_deref()
                                            .and_then(|n| app.textures.thumb(ui.ctx(), &dir, n));
                                        let (rect, _) =
                                            ui.allocate_exact_size(size, egui::Sense::hover());
                                        ui.painter().rect_filled(
                                            rect,
                                            egui::CornerRadius::same(3),
                                            p.canvas,
                                        );
                                        match thumb {
                                            Some(tex) => {
                                                let img = tex.size_vec2();
                                                let k = (size.x / img.x).min(size.y / img.y);
                                                let r = egui::Rect::from_center_size(
                                                    rect.center(),
                                                    img * k,
                                                );
                                                ui.painter().image(
                                                    tex.id(),
                                                    r,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            }
                                            None => Icon::Keyboard.paint(
                                                ui.painter(),
                                                rect.shrink(10.0),
                                                p.text_faint,
                                            ),
                                        }
                                        ui.vertical(|ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(format!(
                                                        "{}. {}",
                                                        s + 1,
                                                        staged.step.display_title(lang)
                                                    ))
                                                    .color(p.text),
                                                )
                                                .truncate(),
                                            );
                                            if let Some(w) = &staged.image_warning {
                                                ui.horizontal(|ui| {
                                                    icons::icon(ui, Icon::Warning, 12.0, p.warning);
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(format!(
                                                                "{} ({w})",
                                                                lang.pick(
                                                                    "Capture illisible : l'étape sera importée sans image",
                                                                    "Unreadable screenshot: the step comes without its image"
                                                                )
                                                            ))
                                                            .size(theme::SIZE_SMALL)
                                                            .color(p.warning),
                                                        )
                                                        .truncate(),
                                                    );
                                                });
                                            }
                                        });
                                    });
                                }
                            });
                        ui.add_space(6.0);
                    }
                });

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(lang.pick("Insérer :", "Insert:")).color(p.text_muted));
                ui.radio_value(
                    &mut dialog.position,
                    Position::Append,
                    lang.pick("à la fin", "at the end"),
                );
                ui.add_enabled_ui(has_selection, |ui| {
                    ui.radio_value(
                        &mut dialog.position,
                        Position::AfterSelected,
                        lang.pick("après l'étape sélectionnée", "after the selected step"),
                    );
                });
                ui.radio_value(
                    &mut dialog.position,
                    Position::Start,
                    lang.pick("au début", "at the start"),
                );
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let n = dialog.count();
                ui.add_enabled_ui(n > 0, |ui| {
                    let label = format!(
                        "{} ({n})",
                        lang.pick("Importer la sélection", "Import selected")
                    );
                    if widgets::button(ui, Kind::Primary, Some(Icon::Import), &label).clicked() {
                        choice = Choice::Import;
                    }
                });
                if widgets::button(ui, Kind::Ghost, None, lang.pick("Annuler", "Cancel")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    choice = Choice::Cancel;
                }
            });
        });

    match choice {
        Choice::Import => app.confirm_import(dialog),
        Choice::Cancel => app.cancel_import(dialog),
        Choice::None => app.import = Some(dialog),
    }
}
