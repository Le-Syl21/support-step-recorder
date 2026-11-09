//! The frame around every screen: the top bar (brand, zoom, theme,
//! language) and the status bar (current activity, community link).

use eframe::egui;
use egui::{CornerRadius, Margin, RichText, Stroke, ThemePreference};
use ssr_core::Lang;

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{self, pal};
use crate::widgets::{self, Seg};

const DISCORD_URL: &str = "https://discord.gg/T37DYHmt2j";

pub fn top_bar(app: &mut App, root: &mut egui::Ui) {
    let p = pal(root.ctx());
    egui::Panel::top("top_bar")
        .frame(
            egui::Frame::new()
                .fill(p.bar)
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(16, 8)),
        )
        .show(root, |ui| {
            egui::Sides::new()
                .shrink_left()
                .truncate()
                .show(ui, brand, |ui| {
                    // Laid out right to left: language, theme, zoom.
                    language_switch(app, ui);
                    ui.add_space(4.0);
                    theme_switch(app, ui);
                    ui.add_space(4.0);
                    zoom_control(app, ui);
                });
        });
}

/// The app mark (a record dot over a list of steps) and its name.
fn brand(ui: &mut egui::Ui) {
    let p = pal(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(26.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(7), p.accent);
    for (i, y) in [0.36f32, 0.52, 0.68].iter().enumerate() {
        let y = rect.top() + rect.height() * y;
        let w = if i == 1 { 0.34 } else { 0.42 };
        painter.line_segment(
            [
                egui::pos2(rect.left() + rect.width() * 0.22, y),
                egui::pos2(rect.left() + rect.width() * (0.22 + w), y),
            ],
            Stroke::new(1.8, p.on_accent),
        );
    }
    painter.circle_filled(
        egui::pos2(rect.right() - 7.0, rect.top() + 7.0),
        4.0,
        p.record,
    );
    painter.circle_stroke(
        egui::pos2(rect.right() - 7.0, rect.top() + 7.0),
        4.0,
        Stroke::new(1.2, p.on_accent),
    );
    ui.add(
        egui::Label::new(
            RichText::new("Support Step Recorder")
                .size(16.0)
                .strong()
                .color(p.text),
        )
        .truncate(),
    );
    ui.label(widgets_mono(
        &format!("v{}", env!("CARGO_PKG_VERSION")),
        p.text_faint,
    ));
}

fn language_switch(app: &mut App, ui: &mut egui::Ui) {
    let segs = [
        Seg {
            icon: None,
            label: Some("FR"),
            tip: "Français",
        },
        Seg {
            icon: None,
            label: Some("EN"),
            tip: "English",
        },
    ];
    let selected = match app.lang {
        Lang::Fr => 0,
        Lang::En => 1,
    };
    if let Some(i) = widgets::segmented(ui, &segs, selected) {
        app.set_lang(if i == 0 { Lang::Fr } else { Lang::En });
    }
}

fn theme_switch(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let segs = [
        Seg {
            icon: Some(Icon::Sun),
            label: None,
            tip: lang.pick("Thème clair", "Light theme"),
        },
        Seg {
            icon: Some(Icon::Moon),
            label: None,
            tip: lang.pick("Thème sombre", "Dark theme"),
        },
        Seg {
            icon: Some(Icon::Monitor),
            label: None,
            tip: lang.pick("Suivre le système", "Follow the system"),
        },
    ];
    let current = ui.ctx().options(|o| o.theme_preference);
    let selected = match current {
        ThemePreference::Light => 0,
        ThemePreference::Dark => 1,
        ThemePreference::System => 2,
    };
    if let Some(i) = widgets::segmented(ui, &segs, selected) {
        let pref = [
            ThemePreference::Light,
            ThemePreference::Dark,
            ThemePreference::System,
        ][i];
        ui.ctx().set_theme(pref);
        app.prefs.set_theme(pref);
    }
}

/// − 100 % + : the GUI zoom, also on Ctrl+plus / Ctrl+minus / Ctrl+0.
fn zoom_control(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let ctx = ui.ctx().clone();
    let zoom = ctx.zoom_factor();
    ui.spacing_mut().item_spacing.x = 2.0;
    // Right to left: plus, the percentage, minus.
    ui.add_enabled_ui(zoom < theme::ZOOM_MAX - 1e-3, |ui| {
        if widgets::icon_button(
            ui,
            Icon::Plus,
            lang.pick("Agrandir (Ctrl +)", "Zoom in (Ctrl +)"),
        )
        .clicked()
        {
            ctx.set_zoom_factor(theme::clamp_zoom(zoom + theme::ZOOM_STEP));
        }
    });
    let p = pal(&ctx);
    let label = ui
        .add(
            egui::Button::new(widgets_mono(
                &format!("{:.0} %", zoom * 100.0),
                p.text_muted,
            ))
            .frame(false)
            .min_size(egui::vec2(48.0, 0.0)),
        )
        .on_hover_text(lang.pick("Taille normale (Ctrl 0)", "Reset zoom (Ctrl 0)"));
    if label.clicked() {
        ctx.set_zoom_factor(1.0);
    }
    ui.add_enabled_ui(zoom > theme::ZOOM_MIN + 1e-3, |ui| {
        if widgets::icon_button(
            ui,
            Icon::Minus,
            lang.pick("Réduire (Ctrl -)", "Zoom out (Ctrl -)"),
        )
        .clicked()
        {
            ctx.set_zoom_factor(theme::clamp_zoom(zoom - theme::ZOOM_STEP));
        }
    });
    app.prefs.zoom = theme::clamp_zoom(ctx.zoom_factor());
}

pub fn status_bar(app: &App, root: &mut egui::Ui, activity: Option<&str>) {
    let p = pal(root.ctx());
    let lang = app.lang;
    egui::Panel::bottom("status_bar")
        .frame(
            egui::Frame::new()
                .fill(p.bar)
                .stroke(Stroke::new(1.0, p.border))
                .inner_margin(Margin::symmetric(16, 4)),
        )
        .show(root, |ui| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    // A long action in progress wins over the idle status.
                    if let Some(action) = activity {
                        ui.add(egui::Spinner::new().size(14.0));
                        ui.add(
                            egui::Label::new(RichText::new(action).color(p.text_muted)).truncate(),
                        );
                    } else {
                        ui.add(
                            egui::Label::new(
                                RichText::new(app.status.text(lang))
                                    .size(theme::SIZE_SMALL)
                                    .color(if app.status.is_error() {
                                        p.record
                                    } else {
                                        p.text_muted
                                    }),
                            )
                            .truncate(),
                        );
                    }
                },
                |ui| {
                    let tip = lang.pick("Communauté & support", "Community & support");
                    ui.hyperlink_to(
                        RichText::new("Discord").size(theme::SIZE_SMALL),
                        DISCORD_URL,
                    )
                    .on_hover_text(tip);
                    icons::icon(ui, Icon::Chat, 14.0, p.accent_text);
                },
            );
        });
}

fn widgets_mono(text: &str, color: egui::Color32) -> RichText {
    RichText::new(text)
        .monospace()
        .size(theme::SIZE_SMALL)
        .color(color)
}
