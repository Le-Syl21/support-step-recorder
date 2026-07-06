//! Export d'une session : JSON, rapport HTML rejouable et archive ZIP.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::i18n::Lang;
use crate::model::{Action, Step};
use crate::progress::Progress;
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
    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    add_file(&mut zip, session, "report.html", opts)?;
    add_file(&mut zip, session, "steps.json", opts)?;
    for step in &session.steps {
        if let Some(name) = &step.screenshot {
            add_file(&mut zip, session, name, opts)?;
        }
    }

    zip.finish().map_err(io::Error::other)?;
    Ok(())
}

fn add_file(
    zip: &mut ZipWriter<File>,
    session: &Session,
    name: &str,
    opts: SimpleFileOptions,
) -> io::Result<()> {
    let path = session.dir.join(name);
    if !path.exists() {
        return Ok(());
    }
    let data = fs::read(&path)?;
    zip.start_file(name, opts).map_err(io::Error::other)?;
    zip.write_all(&data)?;
    Ok(())
}

/// Construit le rapport HTML autoportant (steps + lecteur de relecture).
pub fn render_html(session: &Session, lang: Lang) -> String {
    let mut steps_html = String::new();
    for step in &session.steps {
        steps_html.push_str(&render_step(step, lang));
    }

    let date = session.started_at.format("%Y-%m-%d %H:%M:%S");
    let count = session.steps.len();

    // Chaînes traduites injectées dans le gabarit.
    let lang_code = lang.code();
    let t_subtitle = lang.pick("étape(s) — enregistré le", "step(s) — recorded on");
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
<title>Support Step Recorder</title>
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
    <h1>Support Step Recorder</h1>
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
{steps_html}
</main>
<div id="lightbox" onclick="this.classList.remove('open')"><img alt="capture agrandie"></div>
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
    if (!replay) return;
    const n = steps().length;
    cur = (cur + d + n) % n; show();
  }}
  function show() {{
    const all = steps();
    all.forEach((s, i) => s.classList.toggle('current', i === cur));
    document.getElementById('pos').textContent =
      replay ? `{t_step} ${{cur + 1}} / ${{all.length}}` : '';
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

fn render_step(step: &Step, lang: Lang) -> String {
    let typed = matches!(step.action, Action::Text { .. });
    let class = if typed { "step typed" } else { "step" };
    let elapsed = format!("{:.1}s", step.elapsed_ms as f64 / 1000.0);
    // Description reconstruite dans la langue du rapport (indépendante de la
    // langue d'enregistrement).
    let desc = Step::build_description(&step.action, step.window.as_ref(), lang);

    let ctx = step
        .window
        .as_ref()
        .map(|w| {
            format!(
                "{} · PID {} · {}×{}",
                escape(&w.app_name),
                w.pid,
                w.width,
                w.height
            )
        })
        .unwrap_or_default();

    let image = step
        .screenshot
        .as_ref()
        .map(|name| {
            format!(
                "<img src=\"{}\" alt=\"capture étape {}\" onclick=\"zoom(this)\">",
                escape(name),
                step.index
            )
        })
        .unwrap_or_default();

    format!(
        r#"<section class="{class}">
  <h2><span class="num">{idx}</span>{desc}</h2>
  <p class="ctx">⏱ {elapsed} — {ctx}</p>
  {image}
</section>
"#,
        idx = step.index,
        desc = escape(&desc),
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
