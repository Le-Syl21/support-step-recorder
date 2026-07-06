#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Interface egui/glow de Support Step Recorder.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use ssr_core::{export, session::Session, Lang, Progress, Recorder};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 640.0]),
        // Rendu via glow (OpenGL) — explicite.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Support Step Recorder",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

/// Ligne d'étape extraite de la session pour l'affichage (sans verrou).
///
/// On garde l'action et la fenêtre brutes (pas le texte déjà traduit) pour
/// pouvoir re-construire la description dans la langue courante à l'affichage.
#[derive(Clone)]
struct StepRow {
    index: usize,
    screenshot: Option<String>,
    /// Horodatage absolu de la capture (ms depuis l'epoch Unix).
    timestamp_ms: u128,
    /// Temps écoulé depuis le début de la session (ms).
    elapsed_ms: u128,
    action: ssr_core::Action,
    window: Option<ssr_core::WindowInfo>,
}

impl StepRow {
    /// Description lisible dans la langue donnée.
    fn description(&self, lang: Lang) -> String {
        ssr_core::Step::build_description(&self.action, self.window.as_ref(), lang)
    }
}

/// Instantané de la session pour un rendu de frame.
#[derive(Clone)]
struct Snapshot {
    dir: PathBuf,
    rows: Vec<StepRow>,
}

struct App {
    base_dir: PathBuf,
    recorder: Option<Recorder>,
    /// Dernière session terminée, conservée pour la revue et l'export.
    last: Option<Arc<Mutex<Session>>>,
    selected: Option<usize>,
    status: String,
    textures: HashMap<String, egui::TextureHandle>,
    /// Action longue en cours (capture, sauvegarde, compression…), partagée
    /// avec les threads de travail et affichée dans la barre de statut.
    progress: Progress,
    /// Export en cours dans un thread dédié : reçoit le résultat final.
    export: Option<Receiver<Result<PathBuf, String>>>,
    /// Dernier instantané rendu, conservé quand la session est momentanément
    /// verrouillée (par ex. pendant l'export) pour éviter tout clignotement.
    snapshot: Option<Snapshot>,
    /// Session arrêtée en attente d'une destination de sauvegarde.
    pending_save: Option<Arc<Mutex<Session>>>,
    /// Affiche la confirmation « session non sauvegardée » (après annulation).
    confirm_discard: bool,
    /// Langue de l'interface (et des rapports générés).
    lang: Lang,
}

impl App {
    fn new() -> Self {
        let lang = Lang::detect();
        Self {
            // Les captures sont volatiles : elles vivent dans un dossier temporaire
            // jusqu'à ce que l'utilisateur choisisse où enregistrer le rapport.
            base_dir: std::env::temp_dir().join("support-step-recorder"),
            recorder: None,
            last: None,
            selected: None,
            status: lang.pick("Prêt.", "Ready.").to_string(),
            textures: HashMap::new(),
            progress: Progress::new(),
            export: None,
            snapshot: None,
            pending_save: None,
            confirm_discard: false,
            lang,
        }
    }

    /// Raccourci de traduction : renvoie la chaîne selon la langue courante.
    fn t(&self, fr: &'static str, en: &'static str) -> &'static str {
        self.lang.pick(fr, en)
    }

    /// Change la langue de l'interface.
    fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
        // Rafraîchit le statut au repos dans la nouvelle langue.
        if !self.is_recording() && self.export.is_none() {
            self.status = self.t("Prêt.", "Ready.").to_string();
        }
    }

    fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Session active : celle en cours d'enregistrement, sinon la dernière terminée.
    fn current(&self) -> Option<Arc<Mutex<Session>>> {
        self.recorder
            .as_ref()
            .map(|r| r.session.clone())
            .or_else(|| self.last.clone())
    }

    fn start(&mut self) {
        // Purge le dossier temporaire de la session précédente (les captures
        // n'ont d'intérêt qu'une fois le rapport enregistré par l'utilisateur).
        self.cleanup_previous_temp();

        let name = format!("session-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let dir = self.base_dir.join(name);
        match Recorder::start(dir.clone(), self.progress.clone(), self.lang) {
            Ok(rec) => {
                self.status = format!(
                    "{} [{}]…",
                    self.t("Enregistrement", "Recording"),
                    rec.backend_name()
                );
                self.recorder = Some(rec);
                self.last = None;
                self.selected = None;
                self.textures.clear();
            }
            Err(err) => {
                self.status = format!("{} {err}", self.t("Échec du démarrage :", "Start failed:"))
            }
        }
    }

    fn stop(&mut self) {
        if let Some(rec) = self.recorder.take() {
            let session = rec.stop();
            let n = session.lock().map(|s| s.steps.len()).unwrap_or(0);
            self.status = format!(
                "{} {n} {}",
                self.t("Enregistrement arrêté —", "Recording stopped —"),
                self.t("étape(s).", "step(s).")
            );
            self.last = Some(session.clone());
            // Enchaîne directement sur le choix de la destination.
            self.pending_save = Some(session);
            self.begin_save();
        }
    }

    /// Ouvre le dialogue « Enregistrer sous » puis lance la génération à
    /// l'emplacement choisi. Annuler déclenche la confirmation.
    fn begin_save(&mut self) {
        let Some(session) = self.pending_save.clone() else {
            return;
        };
        let default = default_zip_name(&session);
        let dest = rfd::FileDialog::new()
            .set_title(self.t(
                "Enregistrer le rapport (ZIP + HTML)",
                "Save report (ZIP + HTML)",
            ))
            .set_file_name(default)
            .add_filter(self.t("Archive ZIP", "ZIP archive"), &["zip"])
            .save_file();
        match dest {
            Some(path) => {
                self.confirm_discard = false;
                self.pending_save = None;
                self.export_to(session, path);
            }
            None => self.confirm_discard = true, // annulation → demander confirmation
        }
    }

    /// Génère le ZIP+HTML vers `path` dans un thread dédié.
    fn export_to(&mut self, session: Arc<Mutex<Session>>, mut path: PathBuf) {
        if path.extension().is_none() {
            path.set_extension("zip");
        }
        let progress = self.progress.clone();
        let lang = self.lang;
        let unreachable = self
            .t("Session inaccessible.", "Session unavailable.")
            .to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = match session.lock() {
                Ok(guard) => export::export_zip(&guard, &path, &progress, lang)
                    .map(|()| path)
                    .map_err(|err| err.to_string()),
                Err(_) => Err(unreachable),
            };
            progress.clear();
            let _ = tx.send(result);
        });
        self.status = self
            .t("Génération du rapport…", "Generating report…")
            .to_string();
        self.export = Some(rx);
    }

    /// Supprime le dossier temporaire de la dernière session.
    fn cleanup_previous_temp(&mut self) {
        if let Some(prev) = self.last.take() {
            if let Ok(s) = prev.lock() {
                let _ = std::fs::remove_dir_all(&s.dir);
            }
        }
    }

    /// Récupère le résultat de l'export terminé, s'il y en a un.
    fn poll_export(&mut self) {
        let Some(rx) = &self.export else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.status = format!("{} {}", self.t("Exporté :", "Exported:"), path.display());
                self.export = None;
            }
            Ok(Err(err)) => {
                self.status = format!("{} {err}", self.t("Échec de l'export :", "Export failed:"));
                self.export = None;
            }
            Err(TryRecvError::Empty) => {} // toujours en cours
            Err(TryRecvError::Disconnected) => {
                self.status = self
                    .t("Export interrompu.", "Export interrupted.")
                    .to_string();
                self.export = None;
            }
        }
    }

    /// Charge (et met en cache) la texture d'une capture.
    fn texture(
        &mut self,
        ctx: &egui::Context,
        dir: &std::path::Path,
        name: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(name) {
            return Some(t.clone());
        }
        let img = image::open(dir.join(name)).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let handle = ctx.load_texture(name, color, egui::TextureOptions::LINEAR);
        self.textures.insert(name.to_string(), handle.clone());
        Some(handle)
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll_export();

        // Action longue en cours (capture, sauvegarde, HTML, compression…).
        let activity = self.progress.get();

        // Tant qu'un travail tourne (enregistrement, action en cours, export),
        // les données évoluent depuis d'autres threads : rafraîchir régulièrement.
        if self.is_recording() || activity.is_some() || self.export.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        egui::Panel::top("toolbar").show(root, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if self.is_recording() {
                    paint_square(ui, egui::Color32::from_rgb(220, 70, 70));
                    if ui.button(self.t("Arrêter", "Stop")).clicked() {
                        self.stop();
                    }
                    ui.add_space(8.0);
                    paint_dot(ui, egui::Color32::RED);
                    ui.colored_label(egui::Color32::RED, self.t("enregistrement", "recording"));
                } else {
                    paint_dot(ui, egui::Color32::RED);
                    if ui.button(self.t("Démarrer", "Start")).clicked() {
                        self.start();
                    }
                }

                // À droite : bouton de langue (FR/EN) puis sélecteur de thème.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    theme_toggle(ui, self.lang);
                    // Bouton de langue : affiche la langue vers laquelle basculer.
                    let other = self.lang.toggled();
                    let label = other.pick("FR", "EN");
                    let tip = self.t("Passer en anglais", "Switch to French");
                    if ui.button(label).on_hover_text(tip).clicked() {
                        self.set_lang(other);
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::Panel::bottom("status").show(root, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                // Une action longue en cours prime sur le statut au repos.
                if let Some(action) = &activity {
                    ui.spinner();
                    ui.label(action);
                } else {
                    ui.label(&self.status);
                }
            });
            ui.add_space(2.0);
        });

        // Données nécessaires au rendu, extraites sous verrou puis relâchées.
        // `try_lock` : si la session est occupée (export en cours), on garde le
        // dernier instantané au lieu de bloquer l'IHM.
        if let Some(s) = self.current() {
            if let Ok(g) = s.try_lock() {
                let rows = g
                    .steps
                    .iter()
                    .map(|st| StepRow {
                        index: st.index,
                        screenshot: st.screenshot.clone(),
                        timestamp_ms: st.timestamp_ms,
                        elapsed_ms: st.elapsed_ms,
                        action: st.action.clone(),
                        window: st.window.clone(),
                    })
                    .collect();
                self.snapshot = Some(Snapshot {
                    dir: g.dir.clone(),
                    rows,
                });
            }
        } else {
            self.snapshot = None;
        }
        let snapshot = self.snapshot.clone();

        // Navigation clavier dans la liste : flèches haut/bas.
        let mut nav_scroll = false;
        if let Some(rows) = snapshot.as_ref().map(|s| &s.rows).filter(|r| !r.is_empty()) {
            let cur = self
                .selected
                .and_then(|sel| rows.iter().position(|r| r.index == sel))
                .unwrap_or(rows.len() - 1);
            let (up, down) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                )
            });
            if down && cur + 1 < rows.len() {
                self.selected = Some(rows[cur + 1].index);
                nav_scroll = true;
            } else if up && cur > 0 {
                self.selected = Some(rows[cur - 1].index);
                nav_scroll = true;
            }
        }

        egui::Panel::left("steps")
            .resizable(true)
            .default_size(320.0)
            .show(root, |ui| {
                ui.heading(self.t("Étapes", "Steps"));
                ui.separator();
                match &snapshot {
                    Some(snap) if !snap.rows.is_empty() => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for row in &snap.rows {
                                let label =
                                    format!("{}. {}", row.index, row.description(self.lang));
                                let selected = self.selected == Some(row.index);
                                let resp = ui.selectable_label(selected, label);
                                if resp.clicked() {
                                    self.selected = Some(row.index);
                                }
                                // Garde la sélection visible lors de la nav clavier.
                                if selected && nav_scroll {
                                    resp.scroll_to_me(Some(egui::Align::Center));
                                }
                            }
                        });
                    }
                    _ => {
                        ui.label(self.t("Aucune étape pour l'instant.", "No steps yet."));
                    }
                }
            });

        let lang = self.lang;
        egui::CentralPanel::default().show(root, |ui| {
            let Some(snap) = snapshot else {
                ui.centered_and_justified(|ui| {
                    ui.label(self.t(
                        "Démarrez un enregistrement pour capturer les étapes.",
                        "Start a recording to capture steps.",
                    ));
                });
                return;
            };

            let selected = self.selected.or_else(|| snap.rows.last().map(|r| r.index));
            let Some(sel) = selected else { return };
            let Some(row) = snap.rows.iter().find(|r| r.index == sel) else {
                return;
            };

            ui.heading(format!(
                "{} {} — {}",
                self.t("Étape", "Step"),
                row.index,
                row.description(lang)
            ));
            step_metadata(ui, row, lang);
            ui.separator();
            match &row.screenshot {
                Some(name) => {
                    if let Some(tex) = self.texture(&ctx, &snap.dir, name) {
                        egui::ScrollArea::both().show(ui, |ui| {
                            ui.add(
                                egui::Image::new(&tex)
                                    .max_width(ui.available_width())
                                    .maintain_aspect_ratio(true),
                            );
                        });
                    } else {
                        ui.label(self.t("Capture introuvable.", "Screenshot not found."));
                    }
                }
                None => {
                    ui.label(self.t(
                        "(Étape de saisie clavier — pas de capture associée.)",
                        "(Keyboard input step — no screenshot.)",
                    ));
                }
            }
        });

        // Confirmation après annulation du choix de destination.
        if self.confirm_discard {
            let (mut retry, mut discard) = (false, false);
            egui::Window::new(self.t("Session non enregistrée", "Session not saved"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.label(self.t(
                        "Si vous continuez, cette session ne sera pas sauvegardée.",
                        "If you continue, this session will not be saved.",
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        retry = ui
                            .button(self.t("Choisir un dossier", "Choose a folder"))
                            .clicked();
                        discard = ui
                            .button(self.t("Abandonner la session", "Discard session"))
                            .clicked();
                    });
                });
            if retry {
                self.confirm_discard = false;
                self.begin_save();
            } else if discard {
                self.confirm_discard = false;
                self.pending_save = None;
                self.status = self
                    .t(
                        "Session abandonnée (non enregistrée).",
                        "Session discarded (not saved).",
                    )
                    .to_string();
            }
        }
    }
}

/// Affiche le bloc de métadonnées d'une étape (date, action, fenêtre).
fn step_metadata(ui: &mut egui::Ui, row: &StepRow, lang: Lang) {
    ui.add_space(2.0);

    // Date absolue (format humain) + temps écoulé depuis le début.
    let when = human_date(row.timestamp_ms);
    let elapsed = format!("T+{:.1}s", row.elapsed_ms as f64 / 1000.0);
    ui.label(format!(
        "{} {when}   ({elapsed})",
        lang.pick("Date :", "Date:")
    ));

    // Détail de l'action.
    match &row.action {
        ssr_core::Action::Click { button, x, y, .. } => {
            let coords = (*x)
                .zip(*y)
                .map(|(x, y)| format!("   —   position ({x}, {y})"))
                .unwrap_or_default();
            ui.label(format!(
                "{} {}{coords}",
                lang.pick("Action : clic", "Action: click"),
                button.label(lang)
            ));
        }
        ssr_core::Action::Text { content } => {
            let shown = content.replace('\n', "⏎");
            ui.label(match lang {
                Lang::Fr => format!("Action : saisie « {shown} »"),
                Lang::En => format!("Action: typed “{shown}”"),
            });
        }
    }

    // Fenêtre active (souvent inconnue sous Wayland).
    let title = row
        .window
        .as_ref()
        .map(|w| w.title.as_str())
        .filter(|t| !t.is_empty());
    let app = row
        .window
        .as_ref()
        .map(|w| w.app_name.as_str())
        .filter(|a| !a.is_empty());
    match (title, app) {
        (Some(t), Some(a)) => {
            ui.label(format!(
                "{} « {t} » ({a})",
                lang.pick("Fenêtre :", "Window:")
            ));
        }
        (Some(t), None) => {
            ui.label(format!("{} « {t} »", lang.pick("Fenêtre :", "Window:")));
        }
        (None, Some(a)) => {
            ui.label(format!(
                "{} {a}",
                lang.pick("Application :", "Application:")
            ));
        }
        (None, None) => {
            ui.weak(lang.pick(
                "Fenêtre : inconnue (non exposée sous Wayland)",
                "Window: unknown (not exposed under Wayland)",
            ));
        }
    }
}

/// Nom de fichier ZIP par défaut, dérivé du dossier de session.
fn default_zip_name(session: &Arc<Mutex<Session>>) -> String {
    session
        .lock()
        .ok()
        .and_then(|s| s.dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .map(|n| format!("{n}.zip"))
        .unwrap_or_else(|| "session.zip".to_string())
}

/// Formate un horodatage Unix (ms) en date locale lisible.
fn human_date(ms: u128) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%d/%m/%Y %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "?".to_string())
}

/// Peint une pastille ronde pleine à la position courante (indicateur d'état).
///
/// On dessine la forme plutôt que d'utiliser un glyphe : les polices d'egui ne
/// couvrent pas les formes géométriques (`●` `■` `◐`) et afficheraient du tofu.
fn paint_dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

/// Peint un petit carré plein (idem, cf. [`paint_dot`]).
fn paint_square(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
    let sq = egui::Rect::from_center_size(rect.center(), egui::vec2(9.0, 9.0));
    ui.painter().rect_filled(sq, 1.0, color);
}

/// Bouton de thème peint (aucun glyphe) qui cycle clair → obscur → système.
/// L'icône est un disque : vide (clair), plein (obscur), à moitié (système).
fn theme_toggle(ui: &mut egui::Ui, lang: Lang) {
    use egui::ThemePreference::{Dark, Light, System};
    let current = ui.ctx().options(|opt| opt.theme_preference);
    let tip = match current {
        Light => lang.pick(
            "Thème : clair (cliquer pour obscur)",
            "Theme: light (click for dark)",
        ),
        Dark => lang.pick(
            "Thème : obscur (cliquer pour système)",
            "Theme: dark (click for system)",
        ),
        System => lang.pick(
            "Thème : système (cliquer pour clair)",
            "Theme: system (click for light)",
        ),
    };

    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    let c = rect.center();
    let r = 7.0;
    let col = ui.visuals().text_color();
    let p = ui.painter();
    p.circle_stroke(c, r, egui::Stroke::new(1.5, col));
    match current {
        Light => {} // disque vide
        Dark => {
            p.circle_filled(c, r, col); // disque plein
        }
        System => {
            // Moitié droite pleine.
            use egui::epaint::PathShape;
            let pts: Vec<egui::Pos2> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (i as f32 / 16.0);
                    egui::pos2(c.x + r * a.cos(), c.y + r * a.sin())
                })
                .collect();
            p.add(PathShape::convex_polygon(pts, col, egui::Stroke::NONE));
        }
    }

    if resp.on_hover_text(tip).clicked() {
        let next = match current {
            Light => Dark,
            Dark => System,
            System => Light,
        };
        ui.ctx().set_theme(next);
    }
}
