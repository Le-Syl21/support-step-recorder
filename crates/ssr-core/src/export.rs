//! Export d'une session : JSON, rapport HTML rejouable et archive ZIP.
//!
//! Deux entrées : [`export_zip`] pour une [`Session`] brute (API historique)
//! et [`export_guide`] pour un guide édité (titres, consignes, ordre, images
//! dédupliquées). Les deux écrivent l'archive dans un fichier temporaire voisin
//! puis le renomment : un export raté laisse l'ancien fichier intact.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::editor::GuideSnapshot;
use crate::i18n::Lang;
use crate::model::{Action, Step, StepKind};
use crate::progress::Progress;
use crate::project::{self, SaveError};
use crate::session::Session;

/// Écrit `steps.json` et `report.html` (dans la langue `lang`) dans le dossier.
pub fn write_report(session: &Session, lang: Lang) -> io::Result<()> {
    let json = serde_json::to_string_pretty(&session.steps).map_err(io::Error::other)?;
    fs::write(session.dir.join("steps.json"), json)?;
    fs::write(session.dir.join("report.html"), render_html(session, lang))?;
    Ok(())
}

/// Génère le rapport puis empaquette tout (HTML, JSON, WebP) dans une archive ZIP.
///
/// `progress` reçoit les phases longues (création du HTML, compression) pour
/// affichage par l'IHM.
pub fn export_zip(
    session: &Session,
    zip_path: &Path,
    progress: &Progress,
    lang: Lang,
) -> io::Result<()> {
    progress.set(lang.msg_building_html());
    write_report(session, lang)?;

    progress.set(lang.msg_compressing());
    let mut names = vec!["report.html".to_string(), "steps.json".to_string()];
    for step in &session.steps {
        if let Some(name) = &step.screenshot {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    project::write_atomically(zip_path, |file| {
        let mut zip = ZipWriter::new(file);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for name in &names {
            let path = session.dir.join(name);
            if !path.exists() {
                continue;
            }
            let data = fs::read(&path)?;
            zip.start_file(name.as_str(), opts)
                .map_err(io::Error::other)?;
            zip.write_all(&data)?;
        }
        zip.finish().map_err(io::Error::other)
    })
}

/// Exporte un guide édité : `report.html`, `steps.json` (numérotés 1..N, avec
/// titres et consignes) et uniquement les images encore référencées.
///
/// Une image manquante est une erreur : on n'écrit jamais un rapport aux
/// images cassées.
pub fn export_guide(
    guide: &GuideSnapshot,
    zip_path: &Path,
    progress: &Progress,
    lang: Lang,
) -> Result<(), SaveError> {
    progress.set(lang.msg_building_html());
    let steps = project::numbered(&guide.steps);
    let assets = project::referenced_assets(&steps, &guide.assets)?;
    let html = render_guide_html(
        &GuideView {
            title: &guide.metadata.title,
            description: &guide.metadata.description,
            date: guide_date(&guide.metadata.created_at),
            steps: &steps,
            show_elapsed: false,
        },
        lang,
    );
    let json = serde_json::to_vec_pretty(&steps).map_err(io::Error::other)?;

    progress.set(lang.msg_compressing());
    project::write_atomically(zip_path, |file| {
        let mut zip = ZipWriter::new(file);
        let text =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let raw = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("report.html", text)
            .map_err(io::Error::other)?;
        zip.write_all(html.as_bytes())?;
        zip.start_file("steps.json", text)
            .map_err(io::Error::other)?;
        zip.write_all(&json)?;
        for a in &assets {
            let bytes = guide.assets.read(a)?;
            zip.start_file(a.as_str(), raw).map_err(io::Error::other)?;
            zip.write_all(&bytes)?;
        }
        zip.finish().map_err(io::Error::other)
    })?;
    Ok(())
}

/// The date shown under the title: the guide's creation, local time.
fn guide_date(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Ce que le gabarit HTML affiche.
pub struct GuideView<'a> {
    pub title: &'a str,
    /// Introduction (texte brut).
    pub description: &'a str,
    pub date: String,
    /// Étapes dans l'ordre du guide, déjà numérotées.
    pub steps: &'a [Step],
    /// Afficher le temps écoulé depuis le début : vrai pour une session brute,
    /// faux pour un guide qui peut mêler plusieurs enregistrements (un temps
    /// « global » y serait faux).
    pub show_elapsed: bool,
}

/// Construit le rapport HTML autoportant (steps + lecteur de relecture).
pub fn render_html(session: &Session, lang: Lang) -> String {
    render_guide_html(
        &GuideView {
            title: "Support Step Recorder",
            description: "",
            date: session.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            steps: &session.steps,
            show_elapsed: true,
        },
        lang,
    )
}

/// Le rapport HTML d'un guide. Tout texte venant de l'utilisateur ou de la
/// capture est échappé ; aucune ressource distante.
pub fn render_guide_html(view: &GuideView<'_>, lang: Lang) -> String {
    let mut steps_html = String::new();
    for (i, step) in view.steps.iter().enumerate() {
        steps_html.push_str(&render_step(step, i + 1, lang, view.show_elapsed));
    }

    let date = &view.date;
    let count = view.steps.len();
    let title = escape(view.title);
    let intro = if view.description.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<section class=\"intro\">{}</section>",
            escape(view.description)
        )
    };

    // Chaînes traduites injectées dans le gabarit.
    let lang_code = lang.code();
    let t_subtitle = if view.show_elapsed {
        lang.pick("étape(s) — enregistré le", "step(s) — recorded on")
    } else {
        lang.pick("étape(s) — guide créé le", "step(s) — guide created on")
    };
    let t_replay = lang.pick("Mode relecture", "Replay mode");
    let t_prev = lang.pick("Précédent", "Previous");
    let t_next = lang.pick("Suivant", "Next");
    let t_step = lang.pick("Étape", "Step");

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang_code}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="Support Step Recorder">
<title>{title}</title>
<style>
  :root {{ color-scheme: light dark; }}
  /* La barre collée en haut a une hauteur variable ; on réserve la place au
     défilement pour que les étapes ne se cachent pas dessous. */
  html {{ scroll-padding-top: 8.5rem; }}
  body {{ font-family: system-ui, sans-serif; margin: 0; background: #1115; }}
  /* Header + toolbar forment UN seul bloc collé (pas de décalage entre eux). */
  .topbar {{ position: sticky; top: 0; z-index: 10; }}
  header {{ padding: 1rem 1.5rem; background: #2b6cb0; color: #fff; }}
  header h1 {{ margin: 0 0 .25rem; font-size: 1.2rem; }}
  header .meta {{ font-size: .85rem; opacity: .9; }}
  .toolbar {{ padding: .75rem 1.5rem; display: flex; gap: .5rem; align-items: center;
             background: #232830; border-bottom: 1px solid #0006; }}
  button {{ font: inherit; padding: .35rem .8rem; border: 0; border-radius: .4rem;
           background: #2b6cb0; color: #fff; cursor: pointer; }}
  button:hover {{ background: #2c5282; }}
  /* Bouton relecture « enfoncé » quand le mode est actif. */
  button.active {{ background: #1a4971; box-shadow: inset 0 2px 5px #0007; }}
  /* Navigation visible uniquement en mode relecture. */
  .nav {{ display: none; }}
  body.replay .nav {{ display: inline-block; }}
  main {{ padding: 1.5rem; max-width: 960px; margin: 0 auto; }}
  .step {{ background: #fff2; border: 1px solid #8884; border-radius: .6rem;
          padding: 1rem; margin-bottom: 1.25rem; }}
  .step h2 {{ font-size: 1rem; margin: 0 0 .5rem; }}
  .step .num {{ display: inline-block; min-width: 1.6rem; height: 1.6rem; line-height: 1.6rem;
              text-align: center; background: #2b6cb0; color: #fff; border-radius: 50%;
              margin-right: .5rem; }}
  .step .ctx {{ font-size: .8rem; opacity: .75; margin: 0 0 .5rem; }}
  .step img {{ max-width: 100%; height: auto; border: 1px solid #8886; border-radius: .4rem;
             cursor: zoom-in; }}
  .step.typed {{ border-left: 4px solid #d69e2e; }}
  .step .notes {{ white-space: pre-wrap; margin: 0 0 .75rem; line-height: 1.45; }}
  .intro {{ white-space: pre-wrap; margin: 0 0 1.25rem; line-height: 1.5; }}
  /* Impression : pas de barre collée, une étape ne se coupe pas en deux. */
  @media print {{
    .topbar {{ position: static; }}
    .toolbar, #lightbox {{ display: none !important; }}
    body {{ background: #fff; color: #000; }}
    .step {{ break-inside: avoid; display: block !important; background: none; }}
    .step img {{ max-height: 60vh; }}
  }}
  body.replay .step {{ display: none; }}
  body.replay .step.current {{ display: block; }}
  /* Visionneuse plein écran : clic sur une capture pour l'agrandir. */
  #lightbox {{ display: none; position: fixed; inset: 0; z-index: 100;
              background: #000d; cursor: zoom-out; padding: 2vmin; }}
  #lightbox.open {{ display: flex; align-items: center; justify-content: center; }}
  #lightbox img {{ max-width: 96vw; max-height: 96vh; border-radius: .4rem;
                 box-shadow: 0 8px 40px #000; }}
</style>
</head>
<body>
<div class="topbar">
  <header>
    <h1>{title}</h1>
    <div class="meta">{count} {t_subtitle} {date}</div>
  </header>
  <div class="toolbar">
    <button id="replayBtn" aria-pressed="false" onclick="toggleReplay()">▶ {t_replay}</button>
    <button class="nav" onclick="step(-1)">◀ {t_prev}</button>
    <button class="nav" onclick="step(1)">{t_next} ▶</button>
    <span id="pos"></span>
  </div>
</div>
<main id="steps">
{intro}
{steps_html}
</main>
<div id="lightbox" onclick="this.classList.remove('open')"><img alt=""></div>
<script>
  // Visionneuse : clic sur une capture → plein écran ; clic ou Échap → fermer.
  function zoom(img) {{
    const lb = document.getElementById('lightbox');
    lb.querySelector('img').src = img.src;
    lb.classList.add('open');
  }}
  let replay = false, cur = 0;
  const steps = () => [...document.querySelectorAll('.step')];
  function toggleReplay() {{
    replay = !replay;
    document.body.classList.toggle('replay', replay);
    const btn = document.getElementById('replayBtn');
    btn.classList.toggle('active', replay);
    btn.setAttribute('aria-pressed', replay);
    cur = 0; show();
  }}
  function step(d) {{
    const n = steps().length;
    if (!replay || n === 0) return;
    cur = (cur + d + n) % n; show();
  }}
  function show() {{
    const all = steps();
    all.forEach((s, i) => s.classList.toggle('current', i === cur));
    document.getElementById('pos').textContent =
      replay && all.length ? `{t_step} ${{cur + 1}} / ${{all.length}}` : '';
    if (replay) all[cur]?.scrollIntoView({{behavior: 'smooth', block: 'start'}});
  }}
  document.addEventListener('keydown', e => {{
    if (e.key === 'Escape') document.getElementById('lightbox').classList.remove('open');
    if (e.key === 'ArrowRight') step(1);
    if (e.key === 'ArrowLeft') step(-1);
  }});
</script>
</body>
</html>
"#
    )
}

fn render_step(step: &Step, number: usize, lang: Lang, show_elapsed: bool) -> String {
    let typed = matches!(step.action, Action::Text { .. }) && step.kind == StepKind::Captured;
    let class = if typed { "step typed" } else { "step" };
    // Titre saisi dans l'éditeur, sinon description reconstruite dans la
    // langue du rapport (indépendante de la langue d'enregistrement).
    let desc = step.display_title(lang);

    let mut ctx_parts = Vec::new();
    if show_elapsed {
        ctx_parts.push(format!("⏱ {:.1}s", step.elapsed_ms as f64 / 1000.0));
    }
    if let Some(w) = &step.window {
        ctx_parts.push(format!(
            "{} · PID {} · {}×{}",
            escape(&w.app_name),
            w.pid,
            w.width,
            w.height
        ));
    }
    let ctx = if ctx_parts.is_empty() {
        String::new()
    } else {
        format!("<p class=\"ctx\">{}</p>", ctx_parts.join(" — "))
    };

    let notes = if step.notes.trim().is_empty() {
        String::new()
    } else {
        format!("<p class=\"notes\">{}</p>", escape(&step.notes))
    };

    let image = step
        .screenshot
        .as_ref()
        .map(|name| {
            format!(
                "<img src=\"{}\" alt=\"{} {}\" loading=\"lazy\" onclick=\"zoom(this)\">",
                escape(name),
                lang.pick("Capture de l'étape", "Screenshot of step"),
                number
            )
        })
        .unwrap_or_default();

    format!(
        r#"<section class="{class}">
  <h2><span class="num">{number}</span>{desc}</h2>
  {ctx}
  {notes}
  {image}
</section>
"#,
        desc = escape(&desc),
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use std::fs::File;

    /// The export had no test at all, which is how `zip` crossed six majors
    /// with nothing but a successful compile behind it. A zip writer that
    /// builds is not a zip anyone can open: this reads the archive back.
    #[test]
    fn export_zip_produces_an_archive_that_reads_back() {
        let dir = std::env::temp_dir().join("ssr-export-test");
        let _ = fs::remove_dir_all(&dir);
        let session = Session::new(&dir, Lang::En).expect("session");
        // `export_zip` writes the report itself; steps.json has to be there.
        fs::write(dir.join("steps.json"), b"[]").expect("steps.json");

        let zip_path = dir.join("out.zip");
        export_zip(&session, &zip_path, &Progress::new(), Lang::En).expect("export");

        let f = File::open(&zip_path).expect("open archive");
        let mut archive = zip::ZipArchive::new(f).expect("the archive must be readable");
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert!(names.contains(&"report.html".to_string()), "{names:?}");
        assert!(names.contains(&"steps.json".to_string()), "{names:?}");

        // And the bytes must survive the round trip, not merely be listed.
        let body = {
            let mut entry = archive.by_name("steps.json").expect("entry");
            let mut body = String::new();
            std::io::Read::read_to_string(&mut entry, &mut body).expect("read entry");
            body
        };
        assert_eq!(body, "[]");

        drop(archive);
        let _ = fs::remove_dir_all(&dir);
    }
}
