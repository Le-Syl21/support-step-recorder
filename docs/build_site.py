#!/usr/bin/env python3
"""Generate the GitHub Pages site (English at the root, French under fr/).

Run from anywhere: python3 docs/build_site.py
Pages are plain HTML so search engines index them without JavaScript.
Every statement on these pages must be backed by the repository (README,
crates/, packaging/, CI workflow, release assets): keep it that way.
"""
import html
import json
from pathlib import Path

DOCS = Path(__file__).resolve().parent
SITE = "https://le-syl21.github.io/support-step-recorder/"
REPO = "https://github.com/Le-Syl21/support-step-recorder"
RELEASES = REPO + "/releases"
# Release asset names carry no version number (Package steps of .github/workflows/ci.yml),
# so /releases/latest/download/<name> keeps pointing at the newest build.
DL = REPO + "/releases/latest/download/"
RAW = "https://raw.githubusercontent.com/Le-Syl21/support-step-recorder/main/"
DISCORD = "https://discord.gg/T37DYHmt2j"

PAGES = ["index", "download", "guide", "linux-wayland", "faq"]

UI = {
    "en": {
        "nav": {"index": "Home", "download": "Download", "guide": "How to use",
                "linux-wayland": "Linux & Wayland", "faq": "FAQ"},
        "other": ("fr", "Version française", "FR"),
        "footer_src": "Source code and issues on GitHub", "footer_chat": "Discord",
        "footer_note": "MIT licence. Not affiliated with Microsoft.",
    },
    "fr": {
        "nav": {"index": "Accueil", "download": "Télécharger", "guide": "Mode d'emploi",
                "linux-wayland": "Linux et Wayland", "faq": "FAQ"},
        "other": ("en", "English version", "GB"),
        "footer_src": "Code source et tickets sur GitHub", "footer_chat": "Discord",
        "footer_note": "Licence MIT. Sans lien avec Microsoft.",
    },
}

# Genuine steps.json entry written by ssr-core's exporter for the demo session
# whose report is shown on the site (first step of the English session).
STEPS_JSON = """[
  {
    "index": 1,
    "timestamp_ms": 1789397964643,
    "elapsed_ms": 4011,
    "action": {
      "type": "text",
      "content": "le-syl21.github.io/support-step-recorder\\n"
    },
    "window": {
      "title": "Support Step Recorder - Chromium",
      "app_name": "chromium",
      "pid": 2710879,
      "x": 0,
      "y": 0,
      "width": 1280,
      "height": 800
    },
    "description": "Typed “le-syl21.github.io/support-step-recorder⏎” in « Support Step Recorder - Chromium » (chromium)"
  }
]"""


def href(page, lang, from_lang):
    """Relative link from a page in `from_lang` to `page` in `lang`."""
    up = "../" if from_lang == "fr" else ""
    base = up + ("fr/" if lang == "fr" else "")
    return (base + ("" if page == "index" else page + ".html")) or "./"


def url(page, lang):
    return SITE + ("fr/" if lang == "fr" else "") + ("" if page == "index" else page + ".html")


def downloads(lang):
    en = lang == "en"
    rows = [
        ("Windows", "64-bit (x86_64)" if en else "64 bits (x86_64)", "ssr-windows-x86_64.zip"),
        ("macOS", "Apple silicon (M1, M2…)" if en else "Apple silicon (M1, M2…)", "ssr-macos-arm64.tar.gz"),
        ("macOS", "Intel", "ssr-macos-x86_64.tar.gz"),
        ("Linux", "64-bit PC (x86_64)" if en else "PC 64 bits (x86_64)", "ssr-linux-x86_64.tar.gz"),
        ("Linux", "64-bit ARM (aarch64)" if en else "ARM 64 bits (aarch64)", "ssr-linux-aarch64.tar.gz"),
    ]
    head = ("<th>System</th><th>Download</th>" if en else "<th>Système</th><th>Téléchargement</th>")
    body = "".join(
        f'<tr><td><strong>{system}</strong><br><small>{arch}</small></td>'
        f'<td><a class="btn" href="{DL}{name}">{name}</a></td></tr>'
        for system, arch, name in rows)
    return f'<div class="table"><table class="dl"><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>'


def report_figure(lang):
    img = ("../" if lang == "fr" else "") + "img/"
    if lang == "en":
        return (f'<figure class="shot"><img src="{img}report-en.webp" width="1200" height="900" loading="lazy" '
                'alt="HTML report written by Support Step Recorder: numbered steps with their description and screenshot">'
                '<figcaption>The HTML report as ssr writes it. The session is a short demo built with ssr\'s own '
                'code (three steps on this website), not a real support case.</figcaption></figure>')
    return (f'<figure class="shot"><img src="{img}report-fr.webp" width="1200" height="900" loading="lazy" '
            'alt="Rapport HTML écrit par Support Step Recorder : étapes numérotées avec leur description et leur capture">'
            '<figcaption>Le rapport HTML tel que ssr l\'écrit. La session est une courte démonstration construite avec '
            'le code de ssr (trois étapes sur ce site), pas un vrai cas de support.</figcaption></figure>')


# ---------------------------------------------------------------- content

def content(page, lang):
    p = lambda name: href(name, lang, lang)  # noqa: E731
    en = lang == "en"

    if page == "index":
        if en:
            return ("Free Problem Steps Recorder (psr.exe) alternative for Windows, macOS and Linux",
                    "Support Step Recorder (ssr) takes a screenshot on every click, notes the text you type and saves "
                    "a ZIP with an HTML report. A free, open-source replacement for psr.exe on Windows, macOS and "
                    "Linux, Wayland included.",
                    f"""
<h1>Support Step Recorder</h1>
<p class="lead">A free, open-source replacement for Windows' Problem Steps Recorder (<code>psr.exe</code>), for
Windows, macOS and Linux. On every click, <code>ssr</code> captures the window, notes the mouse button and the text
typed since the previous click. When you stop, it saves a <code>.zip</code> with the screenshots and an HTML report
that you can send to support or keep as a step-by-step how-to.</p>
<div class="actions"><a class="btn" href="{p('download')}">Download</a>
<a class="btn ghost" href="{p('guide')}">How to use it</a>
<a class="btn ghost" href="{REPO}">Source code</a></div>

<h2>Why a new steps recorder</h2>
<p>Microsoft has deprecated Steps Recorder (<code>psr.exe</code>) and plans to remove it from a future version of
Windows. It only ever existed on Windows. <code>ssr</code> keeps the same idea, a screenshot at each click and a
readable list of steps in one file, and runs on Windows, macOS and Linux, under X11 as well as Wayland.</p>

<h2>What it does</h2>
<ul>
<li><strong>Capture on click</strong>: the screenshot shows the state <em>at</em> the click, like the real PSR, so a
filled form is captured before it is sent.</li>
<li><strong>Typed text</strong>: what you type between two clicks becomes a step of its own, placed before the click.</li>
<li><strong>Window details</strong>: the title and application of the window, and on Windows, macOS and X11 a red
ring where you clicked.</li>
<li><strong>Wayland support</strong>: screen capture through <code>xdg-desktop-portal</code> and PipeWire (GNOME, KDE,
wlroots…), with a consent dialog asked once and then remembered. <a href="{p('linux-wayland')}">Linux &amp; Wayland setup</a>.</li>
<li><strong>Small files</strong>: WebP screenshots, and window captures trimmed to their content, without black borders.</li>
<li><strong>Never blocks</strong>: image encoding and disk writes run in a background thread, so clicking stays instant.</li>
<li><strong>Replayable HTML report</strong>: step-by-step replay mode, click a screenshot to zoom it, keyboard navigation.</li>
<li><strong>English and French</strong> interface, picked from your locale and switchable with the <code>FR</code>/<code>EN</code> button.</li>
</ul>

<h2>The report</h2>
{report_figure(lang)}
<p>The <code>.zip</code> holds <code>report.html</code>, one <code>step-XXXX.webp</code> per click and a
<code>steps.json</code> for tools. Details in <a href="{p('guide')}#zip">what the ZIP contains</a>.</p>

<h2>Platforms</h2>
<div class="cards">
<div class="card"><h3>Windows</h3><p>64-bit. A single <code>ssr.exe</code>, signed, nothing to install.</p><a class="more" href="{p('download')}#windows">Windows download →</a></div>
<div class="card"><h3>macOS</h3><p>Apple silicon and Intel. Signed and notarized by Apple.</p><a class="more" href="{p('download')}#macos">macOS download →</a></div>
<div class="card"><h3>Linux</h3><p>x86_64 and aarch64. X11 and Wayland (GNOME, KDE, wlroots…).</p><a class="more" href="{p('download')}#linux">Linux download →</a></div>
</div>

<h2>Record in three steps</h2>
<ol>
<li>Click <strong>Start</strong>.</li>
<li>Do what you want to show: clicks, typing.</li>
<li>Click <strong>Stop</strong>, choose where to save, and the <code>.zip</code> with the HTML report is written there.</li>
</ol>
<p>Questions, bug reports, beta testing: <a href="{DISCORD}">Discord</a> or <a href="{REPO}/issues">GitHub issues</a>.</p>
""")
        return ("Alternative gratuite à l'Enregistreur d'actions utilisateur (psr.exe) pour Windows, macOS et Linux",
                "Support Step Recorder (ssr) prend une capture à chaque clic, note le texte saisi et enregistre un ZIP "
                "avec un rapport HTML. Remplaçant libre et gratuit de psr.exe pour Windows, macOS et Linux, Wayland compris.",
                f"""
<h1>Support Step Recorder</h1>
<p class="lead">Un remplaçant libre et gratuit de l'Enregistreur d'actions utilisateur de Windows
(<code>psr.exe</code>, « Problem Steps Recorder »), pour Windows, macOS et Linux. À chaque clic, <code>ssr</code>
capture la fenêtre, note le bouton de la souris et le texte saisi depuis le clic précédent. À l'arrêt, il
enregistre un <code>.zip</code> avec les captures et un rapport HTML, à envoyer au support ou à garder comme tutoriel
pas à pas.</p>
<div class="actions"><a class="btn" href="{p('download')}">Télécharger</a>
<a class="btn ghost" href="{p('guide')}">Mode d'emploi</a>
<a class="btn ghost" href="{REPO}">Code source</a></div>

<h2>Pourquoi un nouvel enregistreur d'actions</h2>
<p>Microsoft a déprécié l'Enregistreur d'actions utilisateur (<code>psr.exe</code>) et prévoit de le retirer d'une
future version de Windows. Il n'a de toute façon jamais existé que sous Windows. <code>ssr</code> reprend le même
principe, une capture à chaque clic et une liste d'étapes lisible dans un seul fichier, et fonctionne sous Windows,
macOS et Linux, aussi bien sous X11 que sous Wayland.</p>

<h2>Ce qu'il fait</h2>
<ul>
<li><strong>Capture au clic</strong> : la capture montre l'état <em>au moment</em> du clic, comme le vrai PSR ; un
formulaire rempli est donc capturé avant son envoi.</li>
<li><strong>Texte saisi</strong> : ce que vous tapez entre deux clics devient une étape à part, placée avant le clic.</li>
<li><strong>Infos sur la fenêtre</strong> : titre et application de la fenêtre, et sous Windows, macOS et X11 un
cercle rouge à l'endroit du clic.</li>
<li><strong>Compatible Wayland</strong> : capture d'écran via <code>xdg-desktop-portal</code> et PipeWire (GNOME, KDE,
wlroots…), avec une demande d'autorisation posée une fois puis mémorisée. <a href="{p('linux-wayland')}">Mise en place sous Linux et Wayland</a>.</li>
<li><strong>Fichiers légers</strong> : captures en WebP, et captures de fenêtre recadrées sur leur contenu, sans bandes noires.</li>
<li><strong>Ne bloque jamais</strong> : l'encodage des images et l'écriture sur le disque se font dans un thread à part, le clic reste instantané.</li>
<li><strong>Rapport HTML rejouable</strong> : mode relecture pas à pas, zoom au clic sur une capture, navigation au clavier.</li>
<li>Interface en <strong>français et en anglais</strong>, choisie d'après la langue du système et modifiable avec le bouton <code>FR</code>/<code>EN</code>.</li>
</ul>

<h2>Le rapport</h2>
{report_figure(lang)}
<p>Le <code>.zip</code> contient <code>report.html</code>, une image <code>step-XXXX.webp</code> par clic et un
fichier <code>steps.json</code> destiné aux outils. Détails dans <a href="{p('guide')}#zip">le contenu du ZIP</a>.</p>

<h2>Systèmes</h2>
<div class="cards">
<div class="card"><h3>Windows</h3><p>64 bits. Un seul <code>ssr.exe</code>, signé, rien à installer.</p><a class="more" href="{p('download')}#windows">Téléchargement Windows →</a></div>
<div class="card"><h3>macOS</h3><p>Apple silicon et Intel. Signé et notarisé par Apple.</p><a class="more" href="{p('download')}#macos">Téléchargement macOS →</a></div>
<div class="card"><h3>Linux</h3><p>x86_64 et aarch64. X11 et Wayland (GNOME, KDE, wlroots…).</p><a class="more" href="{p('download')}#linux">Téléchargement Linux →</a></div>
</div>

<h2>Enregistrer en trois étapes</h2>
<ol>
<li>Cliquez sur <strong>Démarrer</strong>.</li>
<li>Faites ce que vous voulez montrer : clics, saisies.</li>
<li>Cliquez sur <strong>Arrêter</strong>, choisissez où enregistrer : le <code>.zip</code> et son rapport HTML y sont écrits.</li>
</ol>
<p>Questions, signalements de bugs, tests de versions bêta : <a href="{DISCORD}">Discord</a> ou <a href="{REPO}/issues">tickets GitHub</a>.</p>
""")

    if page == "download":
        if en:
            return ("Download Support Step Recorder for Windows, macOS and Linux – free psr.exe replacement",
                    "Download ssr, the free Problem Steps Recorder alternative: Windows (signed .exe), macOS Apple silicon "
                    "and Intel (signed, notarized), Linux x86_64 and aarch64. Or install it with cargo.",
                    f"""
<h1>Download Support Step Recorder</h1>
<p class="lead">One program, <code>ssr</code>, with no installer. Pick the file for your system: the links always
point to the latest release.</p>
{downloads(lang)}
<p>Older versions and release notes: <a href="{RELEASES}">all releases on GitHub</a>.</p>

<h2 id="windows">Windows</h2>
<ol>
<li>Download <a href="{DL}ssr-windows-x86_64.zip">ssr-windows-x86_64.zip</a> and extract it.</li>
<li>Run <code>ssr.exe</code>.</li>
</ol>
<p><code>ssr.exe</code> carries an Authenticode signature, applied and checked by the release workflow.</p>

<h2 id="macos">macOS</h2>
<ol>
<li>Download <a href="{DL}ssr-macos-arm64.tar.gz">ssr-macos-arm64.tar.gz</a> for Apple silicon, or
<a href="{DL}ssr-macos-x86_64.tar.gz">ssr-macos-x86_64.tar.gz</a> for Intel.</li>
<li>Extract it and run <code>ssr</code>:
<pre><code>tar xzf ssr-macos-arm64.tar.gz
./ssr</code></pre></li>
</ol>
<p>Both binaries are signed with an Apple Developer ID and notarized by Apple.</p>
<div class="note">The macOS window capture (ScreenCaptureKit) was written without a Mac to test on: it builds in the
release workflow, but its behaviour on a real Mac still has to be confirmed. Feedback is welcome on
<a href="{DISCORD}">Discord</a> or in <a href="{REPO}/issues">GitHub issues</a>.</div>

<h2 id="linux">Linux</h2>
<ol>
<li>Download <a href="{DL}ssr-linux-x86_64.tar.gz">ssr-linux-x86_64.tar.gz</a> (PC) or
<a href="{DL}ssr-linux-aarch64.tar.gz">ssr-linux-aarch64.tar.gz</a> (64-bit ARM).</li>
<li>Extract it and run <code>ssr</code>:
<pre><code>tar xzf ssr-linux-x86_64.tar.gz
./ssr</code></pre></li>
<li>On a <strong>Wayland</strong> session, install the <code>udev</code> rule first:
<a href="{p('linux-wayland')}">Linux &amp; Wayland setup</a>. Nothing to set up under X11.</li>
</ol>
<p>The Linux binaries are built on Ubuntu 24.04. They need glibc 2.39 or newer and the PipeWire and X11 client
libraries (<code>libpipewire-0.3</code>, <code>libX11</code>).</p>

<h2 id="cargo">Install with Cargo</h2>
<p>The two crates are published on crates.io. With a Rust toolchain:</p>
<pre><code>cargo install support-step-recorder-gui</code></pre>
<p>This installs the <code>ssr</code> command. On Linux, the build needs the PipeWire headers and clang first. On
Debian or Ubuntu:</p>
<pre><code>sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang</code></pre>
<p>The release workflow also installs <code>libxkbcommon-dev libwayland-dev libgl1-mesa-dev libdbus-1-dev libx11-dev
libxext-dev libxi-dev libxtst-dev libxrandr-dev libxcursor-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev
libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shm0-dev libgbm-dev</code> on Ubuntu 24.04; add them if the build stops
on a missing library.</p>

<h2 id="source">Build from source</h2>
<pre><code>git clone {REPO}.git
cd support-step-recorder
cargo run --release --bin ssr</code></pre>
<p>Run the tests with <code>cargo test --workspace</code>.</p>
""")
        return ("Télécharger Support Step Recorder pour Windows, macOS et Linux – remplaçant gratuit de psr.exe",
                "Téléchargez ssr, l'alternative gratuite à l'Enregistreur d'actions utilisateur : Windows (.exe signé), "
                "macOS Apple silicon et Intel (signé, notarisé), Linux x86_64 et aarch64. Ou installez-le avec cargo.",
                f"""
<h1>Télécharger Support Step Recorder</h1>
<p class="lead">Un seul programme, <code>ssr</code>, sans installateur. Choisissez le fichier de votre système : les
liens pointent toujours vers la dernière version.</p>
{downloads(lang)}
<p>Anciennes versions et notes de version : <a href="{RELEASES}">toutes les versions sur GitHub</a>.</p>

<h2 id="windows">Windows</h2>
<ol>
<li>Téléchargez <a href="{DL}ssr-windows-x86_64.zip">ssr-windows-x86_64.zip</a> et décompressez-le.</li>
<li>Lancez <code>ssr.exe</code>.</li>
</ol>
<p><code>ssr.exe</code> porte une signature Authenticode, apposée et vérifiée par le processus de publication.</p>

<h2 id="macos">macOS</h2>
<ol>
<li>Téléchargez <a href="{DL}ssr-macos-arm64.tar.gz">ssr-macos-arm64.tar.gz</a> pour Apple silicon, ou
<a href="{DL}ssr-macos-x86_64.tar.gz">ssr-macos-x86_64.tar.gz</a> pour Intel.</li>
<li>Décompressez-le et lancez <code>ssr</code> :
<pre><code>tar xzf ssr-macos-arm64.tar.gz
./ssr</code></pre></li>
</ol>
<p>Les deux binaires sont signés avec un Developer ID Apple et notarisés par Apple.</p>
<div class="note">La capture de fenêtre sous macOS (ScreenCaptureKit) a été écrite sans Mac pour la tester : elle
compile dans le processus de publication, mais son fonctionnement sur un vrai Mac reste à confirmer. Vos retours sont
les bienvenus sur <a href="{DISCORD}">Discord</a> ou dans les <a href="{REPO}/issues">tickets GitHub</a>.</div>

<h2 id="linux">Linux</h2>
<ol>
<li>Téléchargez <a href="{DL}ssr-linux-x86_64.tar.gz">ssr-linux-x86_64.tar.gz</a> (PC) ou
<a href="{DL}ssr-linux-aarch64.tar.gz">ssr-linux-aarch64.tar.gz</a> (ARM 64 bits).</li>
<li>Décompressez-le et lancez <code>ssr</code> :
<pre><code>tar xzf ssr-linux-x86_64.tar.gz
./ssr</code></pre></li>
<li>Sur une session <strong>Wayland</strong>, installez d'abord la règle <code>udev</code> :
<a href="{p('linux-wayland')}">mise en place sous Linux et Wayland</a>. Rien à faire sous X11.</li>
</ol>
<p>Les binaires Linux sont compilés sous Ubuntu 24.04. Ils demandent la glibc 2.39 ou plus récente, ainsi que les
bibliothèques clientes PipeWire et X11 (<code>libpipewire-0.3</code>, <code>libX11</code>).</p>

<h2 id="cargo">Installer avec Cargo</h2>
<p>Les deux crates sont publiées sur crates.io. Avec une chaîne de compilation Rust :</p>
<pre><code>cargo install support-step-recorder-gui</code></pre>
<p>La commande installe <code>ssr</code>. Sous Linux, la compilation demande d'abord les en-têtes PipeWire et clang.
Sous Debian ou Ubuntu :</p>
<pre><code>sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang</code></pre>
<p>Le processus de publication installe aussi <code>libxkbcommon-dev libwayland-dev libgl1-mesa-dev libdbus-1-dev
libx11-dev libxext-dev libxi-dev libxtst-dev libxrandr-dev libxcursor-dev libxcb1-dev libxcb-render0-dev
libxcb-shape0-dev libxcb-xfixes0-dev libxcb-randr0-dev libxcb-shm0-dev libgbm-dev</code> sous Ubuntu 24.04 ;
ajoutez-les si la compilation s'arrête sur une bibliothèque manquante.</p>

<h2 id="source">Compiler depuis les sources</h2>
<pre><code>git clone {REPO}.git
cd support-step-recorder
cargo run --release --bin ssr</code></pre>
<p>Les tests se lancent avec <code>cargo test --workspace</code>.</p>
""")

    if page == "guide":
        steps_json = html.escape(STEPS_JSON)
        if en:
            return ("How to record the steps to reproduce a problem – Support Step Recorder guide",
                    "Record your clicks and typed text with Support Step Recorder, save them as a ZIP, and read the HTML "
                    "report: replay mode, zoom, steps.json. The psr.exe workflow, on Windows, macOS and Linux.",
                    f"""
<h1>How to record your steps</h1>
<p class="lead">Start, do what you want to show, stop and save. This page goes through a recording and what ends up
in the <code>.zip</code>.</p>

<h2>1. Start</h2>
<p>Open <code>ssr</code> and click <strong>Start</strong>. The status bar at the bottom shows
<em>Recording</em>, followed by the input backend in use.</p>
<ul>
<li>Screenshots first go to a temporary folder (<code>support-step-recorder</code> inside the system's temporary
directory). They are deleted when you start the next recording, so save the session if you want to keep it.</li>
<li>On a Wayland session, your desktop now asks which window or screen to share: see
<a href="{p('linux-wayland')}#portal">the consent dialog</a>.</li>
</ul>

<h2>2. Do the actions to document</h2>
<ul>
<li><strong>Each click</strong>, when the mouse button is released, takes a screenshot of the active window. On
Windows, macOS and X11, a red ring marks the spot you clicked, and clicks on <code>ssr</code>'s own window are not
recorded.</li>
<li><strong>Typed text</strong> is collected until the next click, then saved as a separate step just before that
click. Letters, digits, punctuation, Space, Enter, Tab and Backspace are taken into account; arrows, function keys
and shortcuts are not turned into text.</li>
<li>Steps appear in the list on the left as you go. Select one, or use the Up and Down arrow keys, to see its
screenshot, date, time since the start, action and window.</li>
</ul>
<div class="note">Everything typed between two clicks goes into the report, passwords included. Stop the recording, or
leave secrets out, before typing anything private.</div>

<h2>3. Stop and save</h2>
<ol>
<li>Click <strong>Stop</strong>. A <strong>Save as</strong> dialog opens, with a name such as
<code>session-20260914-153000.zip</code>.</li>
<li>Pick a folder: the report is generated and the status bar shows where it was exported.</li>
<li>If you cancel, <code>ssr</code> warns that the session will not be saved and offers <strong>Choose a folder</strong>
or <strong>Discard session</strong>.</li>
</ol>
<p>The <strong>FR</strong>/<strong>EN</strong> button switches the interface language; the report is written in the
language selected when you save. The round button next to it cycles the theme: light, dark, system.</p>

<h2 id="zip">What the ZIP contains</h2>
<div class="table"><table><thead><tr><th>File</th><th>Content</th></tr></thead><tbody>
<tr><td><code>report.html</code></td><td>The report, to open in any web browser.</td></tr>
<tr><td><code>step-0001.webp</code>, <code>step-0002.webp</code>…</td><td>One screenshot per click, in WebP.</td></tr>
<tr><td><code>steps.json</code></td><td>The same steps in JSON, for scripts and ticketing tools.</td></tr>
</tbody></table></div>
<p>The HTML page loads the WebP files placed next to it: extract the whole archive before opening
<code>report.html</code>.</p>

<h2>Reading the report</h2>
{report_figure(lang)}
<ul>
<li>The header gives the number of steps and the recording date.</li>
<li>Each step shows its number, a description such as <em>Left click on « window title » (application)</em>, the time since the
start, the application, its process ID and the window size. Typed-text steps are marked with a yellow bar.</li>
<li>Click a screenshot to view it full screen; click again or press Escape to close it.</li>
<li><strong>Replay mode</strong> shows one step at a time. Move with the <strong>Previous</strong> and
<strong>Next</strong> buttons or the Left and Right arrow keys.</li>
</ul>

<h2>The steps.json format</h2>
<p>An array of steps. Each has an <code>index</code>, a <code>timestamp_ms</code> (milliseconds since the Unix epoch),
an <code>elapsed_ms</code> since the start, an <code>action</code> (<code>"type": "click"</code> with
<code>button</code> and, when known, <code>x</code>, <code>y</code>, <code>rel_x</code>, <code>rel_y</code>; or
<code>"type": "text"</code> with <code>content</code>), the <code>window</code> when known, the
<code>screenshot</code> file name for clicks, and the <code>description</code>. A typed-text step from the demo
session above:</p>
<pre><code>{steps_json}</code></pre>
""")
        return ("Comment enregistrer les étapes pour reproduire un problème – mode d'emploi de Support Step Recorder",
                "Enregistrez vos clics et votre saisie avec Support Step Recorder, sauvegardez-les en ZIP et lisez le "
                "rapport HTML : mode relecture, zoom, steps.json. Le principe de psr.exe, sous Windows, macOS et Linux.",
                f"""
<h1>Enregistrer vos étapes</h1>
<p class="lead">Démarrer, faire ce que vous voulez montrer, arrêter et enregistrer. Cette page détaille un
enregistrement et ce que contient le <code>.zip</code> obtenu.</p>

<h2>1. Démarrer</h2>
<p>Ouvrez <code>ssr</code> et cliquez sur <strong>Démarrer</strong>. La barre d'état, en bas, affiche
<em>Enregistrement</em> suivi du mode de capture des entrées utilisé.</p>
<ul>
<li>Les captures vont d'abord dans un dossier temporaire (<code>support-step-recorder</code>, dans le dossier
temporaire du système). Elles sont effacées au démarrage de l'enregistrement suivant : enregistrez la session si vous
voulez la garder.</li>
<li>Sur une session Wayland, le bureau demande à ce moment quelle fenêtre ou quel écran partager : voir
<a href="{p('linux-wayland')}#portal">la demande d'autorisation</a>.</li>
</ul>

<h2>2. Faire les actions à documenter</h2>
<ul>
<li><strong>Chaque clic</strong>, au relâchement du bouton, déclenche une capture de la fenêtre active. Sous Windows,
macOS et X11, un cercle rouge marque l'endroit cliqué, et les clics dans la fenêtre de <code>ssr</code> ne sont pas
enregistrés.</li>
<li><strong>Le texte saisi</strong> est mis de côté jusqu'au clic suivant, puis enregistré comme une étape à part,
juste avant ce clic. Lettres, chiffres, ponctuation, Espace, Entrée, Tab et Retour arrière sont pris en compte ; les
flèches, les touches de fonction et les raccourcis ne deviennent pas du texte.</li>
<li>Les étapes s'ajoutent au fur et à mesure dans la liste de gauche. Sélectionnez-en une, ou utilisez les flèches
haut et bas, pour voir sa capture, sa date, le temps écoulé depuis le début, l'action et la fenêtre.</li>
</ul>
<div class="note">Tout ce qui est tapé entre deux clics part dans le rapport, mots de passe compris. Arrêtez
l'enregistrement, ou ne tapez rien de confidentiel, avant de saisir une information privée.</div>

<h2>3. Arrêter et enregistrer</h2>
<ol>
<li>Cliquez sur <strong>Arrêter</strong>. Une fenêtre <strong>Enregistrer sous</strong> s'ouvre, avec un nom du type
<code>session-20260914-153000.zip</code>.</li>
<li>Choisissez un dossier : le rapport est généré et la barre d'état indique où il a été exporté.</li>
<li>Si vous annulez, <code>ssr</code> prévient que la session ne sera pas sauvegardée et propose <strong>Choisir un
dossier</strong> ou <strong>Abandonner la session</strong>.</li>
</ol>
<p>Le bouton <strong>FR</strong>/<strong>EN</strong> change la langue de l'interface ; le rapport est écrit dans la
langue choisie au moment de l'enregistrement. Le bouton rond d'à côté fait passer le thème de clair à sombre, puis à
celui du système.</p>

<h2 id="zip">Le contenu du ZIP</h2>
<div class="table"><table><thead><tr><th>Fichier</th><th>Contenu</th></tr></thead><tbody>
<tr><td><code>report.html</code></td><td>Le rapport, à ouvrir dans n'importe quel navigateur.</td></tr>
<tr><td><code>step-0001.webp</code>, <code>step-0002.webp</code>…</td><td>Une capture par clic, au format WebP.</td></tr>
<tr><td><code>steps.json</code></td><td>Les mêmes étapes en JSON, pour les scripts et les outils de tickets.</td></tr>
</tbody></table></div>
<p>La page HTML charge les images WebP posées à côté d'elle : décompressez toute l'archive avant d'ouvrir
<code>report.html</code>.</p>

<h2>Lire le rapport</h2>
{report_figure(lang)}
<ul>
<li>L'en-tête indique le nombre d'étapes et la date de l'enregistrement.</li>
<li>Chaque étape affiche son numéro, une description du type <em>Clic gauche sur « titre de la fenêtre » (application)</em>, le temps
écoulé depuis le début, l'application, son identifiant de processus (PID) et la taille de la fenêtre. Les étapes de
saisie sont repérées par une barre jaune.</li>
<li>Cliquez sur une capture pour l'afficher en plein écran ; un nouveau clic ou la touche Échap la referme.</li>
<li>Le <strong>mode relecture</strong> montre une étape à la fois. Passez de l'une à l'autre avec les boutons
<strong>Précédent</strong> et <strong>Suivant</strong> ou les flèches gauche et droite.</li>
</ul>

<h2>Le format de steps.json</h2>
<p>Un tableau d'étapes. Chacune a un <code>index</code>, un <code>timestamp_ms</code> (millisecondes depuis l'epoch
Unix), un <code>elapsed_ms</code> depuis le début, une <code>action</code> (<code>"type": "click"</code> avec
<code>button</code> et, quand elles sont connues, <code>x</code>, <code>y</code>, <code>rel_x</code>,
<code>rel_y</code> ; ou <code>"type": "text"</code> avec <code>content</code>), la fenêtre (<code>window</code>)
quand elle est connue, le nom du fichier de capture (<code>screenshot</code>) pour les clics, et la
<code>description</code>. Une étape de saisie de la session de démonstration (enregistrée en anglais) :</p>
<pre><code>{steps_json}</code></pre>
""")

    if page == "linux-wayland":
        rule = "KERNEL==\"event*\", SUBSYSTEM==\"input\", TAG+=\"uaccess\""
        clone = f"""git clone {REPO}.git
cd support-step-recorder
sudo ./packaging/install-linux.sh"""
        curl = f"""curl -fLO {RAW}packaging/install-linux.sh
curl -fLO {RAW}packaging/60-ssr-input.rules
sudo bash install-linux.sh"""
        uninstall = """sudo rm /etc/udev/rules.d/60-ssr-input.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=input"""
        if en:
            return ("Steps recorder for Linux and Wayland (GNOME, KDE) – Support Step Recorder setup",
                    "Record steps on Linux under X11 or Wayland with Support Step Recorder: the udev rule for /dev/input "
                    "(60-ssr-input.rules, install-linux.sh), the xdg-desktop-portal consent dialog and PipeWire capture.",
                    f"""
<h1>Linux and Wayland</h1>
<p class="lead">Under X11, <code>ssr</code> works like on Windows and macOS, with nothing to set up. Under Wayland it
takes a dedicated path, because Wayland does not let an ordinary application watch the clicks and keys of other
windows or capture them freely.</p>

<div class="table"><table><thead><tr><th></th><th>X11</th><th>Wayland</th></tr></thead><tbody>
<tr><td><strong>Clicks and keys</strong></td><td><code>device_query</code></td><td><code>evdev</code>, reading <code>/dev/input</code> (needs the <code>udev</code> rule)</td></tr>
<tr><td><strong>Screenshots</strong></td><td><code>xcap</code>, active window</td><td><code>xdg-desktop-portal</code> + PipeWire, window or screen chosen by you</td></tr>
</tbody></table></div>
<p>The Wayland path is picked when <code>XDG_SESSION_TYPE</code> is <code>wayland</code> or <code>WAYLAND_DISPLAY</code>
is set. The portal is used on every Wayland desktop: <code>xcap</code>'s own Wayland support only works on wlroots
compositors, not on GNOME or KDE.</p>

<h2 id="udev">1. Allow reading input devices</h2>
<p>The <code>evdev</code> backend reads <code>/dev/input</code>. To give access <strong>without</strong> adding your
user to the <code>input</code> group, install the <code>udev</code> rule shipped in the repository:</p>
<pre><code>{html.escape(clone)}</code></pre>
<p>Without git, fetch the two files into the same folder and run the script from there:</p>
<pre><code>{html.escape(curl)}</code></pre>
<p>The script:</p>
<ul>
<li>installs <code>/etc/udev/rules.d/60-ssr-input.rules</code>, which contains a single rule:
<code>{html.escape(rule)}</code>. It gives the user of the active session read access to input devices;</li>
<li>removes an older <code>99-ssr-input.rules</code> if present;</li>
<li>reloads the <code>udev</code> rules and applies them to input devices right away.</li>
</ul>
<p>If capture does not work straight away, log out and back in.</p>
<p>The <code>60-</code> prefix matters: the <code>uaccess</code> tag has to be set <em>before</em> systemd's
<code>73-seat-late.rules</code>, which applies the access rights only to devices already tagged. A rule numbered 99
comes too late and has no effect.</p>
<div class="note"><strong>Security</strong>: this rule lets applications run by the active user read keyboard and
mouse input. That is the trade-off for a PSR-style recorder under Wayland. Screen capture still goes through the
portal and its consent dialog.</div>
<p>Without the rule (or membership of the <code>input</code> group), <strong>Start</strong> fails: the status bar
reports that no readable <code>/dev/input</code> device was found.</p>
<p>To uninstall:</p>
<pre><code>{uninstall}</code></pre>

<h2 id="portal">2. Accept the screen sharing request</h2>
<p>When you click <strong>Start</strong>, your desktop (GNOME, KDE, wlroots…) shows its screen sharing dialog. Pick
<strong>one window or one screen</strong>: every screenshot of the session comes from it, mouse pointer included.</p>
<p><code>ssr</code> keeps the token returned by the portal in <code>~/.local/state/ssr/screencast.token</code> (or
<code>$XDG_STATE_HOME/ssr/screencast.token</code>), so later recordings do not ask again. If the portal is not
available, <code>ssr</code> falls back to <code>xcap</code>.</p>

<h2>What changes under Wayland</h2>
<ul>
<li>The screenshot is the latest image of the shared window or screen. Window captures are trimmed of the black
borders some compositors add around them.</li>
<li>The click position is not known: no red ring on the screenshots, and no coordinates in <code>steps.json</code>.</li>
<li>The title and application of the window are not exposed; the step list shows
<em>Window: unknown (not exposed under Wayland)</em>.</li>
<li>Left, right and middle mouse buttons are recorded.</li>
</ul>

<h2>Building on Linux</h2>
<p>The Wayland capture path needs the PipeWire headers and clang (for bindgen). On Debian or Ubuntu:</p>
<pre><code>sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang</code></pre>
<p>See <a href="{p('download')}#cargo">install with Cargo</a> for the full list used by the release workflow.</p>
""")
        return ("Enregistreur d'actions pour Linux et Wayland (GNOME, KDE) – mise en place de Support Step Recorder",
                "Enregistrer ses actions sous Linux, X11 ou Wayland, avec Support Step Recorder : la règle udev pour "
                "/dev/input (60-ssr-input.rules, install-linux.sh), l'autorisation xdg-desktop-portal et la capture PipeWire.",
                f"""
<h1>Linux et Wayland</h1>
<p class="lead">Sous X11, <code>ssr</code> fonctionne comme sous Windows et macOS, sans rien à régler. Sous Wayland,
il passe par un chemin dédié, car Wayland ne laisse pas une application ordinaire observer les clics et les touches
des autres fenêtres, ni les capturer librement.</p>

<div class="table"><table><thead><tr><th></th><th>X11</th><th>Wayland</th></tr></thead><tbody>
<tr><td><strong>Clics et clavier</strong></td><td><code>device_query</code></td><td><code>evdev</code>, lecture de <code>/dev/input</code> (règle <code>udev</code> nécessaire)</td></tr>
<tr><td><strong>Captures</strong></td><td><code>xcap</code>, fenêtre active</td><td><code>xdg-desktop-portal</code> + PipeWire, fenêtre ou écran choisi par vous</td></tr>
</tbody></table></div>
<p>Le chemin Wayland est choisi quand <code>XDG_SESSION_TYPE</code> vaut <code>wayland</code> ou que
<code>WAYLAND_DISPLAY</code> est défini. Le portail sert sur tous les bureaux Wayland : la prise en charge de Wayland
par <code>xcap</code> ne fonctionne qu'avec les compositeurs wlroots, pas avec GNOME ni KDE.</p>

<h2 id="udev">1. Autoriser la lecture des périphériques d'entrée</h2>
<p>Le backend <code>evdev</code> lit <code>/dev/input</code>. Pour y donner accès <strong>sans</strong> ajouter
l'utilisateur au groupe <code>input</code>, installez la règle <code>udev</code> fournie dans le dépôt :</p>
<pre><code>{html.escape(clone)}</code></pre>
<p>Sans git, récupérez les deux fichiers dans un même dossier et lancez le script depuis celui-ci :</p>
<pre><code>{html.escape(curl)}</code></pre>
<p>Le script :</p>
<ul>
<li>installe <code>/etc/udev/rules.d/60-ssr-input.rules</code>, qui contient une seule règle :
<code>{html.escape(rule)}</code>. Elle donne à l'utilisateur de la session active un accès en lecture aux
périphériques d'entrée ;</li>
<li>supprime une ancienne <code>99-ssr-input.rules</code> si elle existe ;</li>
<li>recharge les règles <code>udev</code> et les applique tout de suite aux périphériques d'entrée.</li>
</ul>
<p>Si la capture ne fonctionne pas tout de suite, fermez puis rouvrez votre session.</p>
<p>Le préfixe <code>60-</code> compte : le tag <code>uaccess</code> doit être posé <em>avant</em>
<code>73-seat-late.rules</code> de systemd, qui n'applique les droits d'accès qu'aux périphériques déjà marqués. Une
règle numérotée 99 arrive trop tard et reste sans effet.</p>
<div class="note"><strong>Sécurité</strong> : cette règle permet aux applications lancées par l'utilisateur actif de
lire les entrées du clavier et de la souris. C'est le compromis assumé pour un enregistreur de type PSR sous Wayland.
La capture d'écran, elle, passe toujours par le portail et sa demande d'autorisation.</div>
<p>Sans la règle (ni appartenance au groupe <code>input</code>), <strong>Démarrer</strong> échoue : la barre d'état
indique qu'aucun périphérique <code>/dev/input</code> lisible n'a été trouvé.</p>
<p>Pour désinstaller :</p>
<pre><code>{uninstall}</code></pre>

<h2 id="portal">2. Accepter le partage d'écran</h2>
<p>Au clic sur <strong>Démarrer</strong>, le bureau (GNOME, KDE, wlroots…) affiche sa fenêtre de partage d'écran.
Choisissez <strong>une fenêtre ou un écran</strong> : toutes les captures de la session en proviennent, pointeur de la
souris compris.</p>
<p><code>ssr</code> garde le jeton renvoyé par le portail dans <code>~/.local/state/ssr/screencast.token</code> (ou
<code>$XDG_STATE_HOME/ssr/screencast.token</code>) : les enregistrements suivants ne redemandent rien. Si le portail
n'est pas disponible, <code>ssr</code> se rabat sur <code>xcap</code>.</p>

<h2>Ce qui change sous Wayland</h2>
<ul>
<li>La capture est la dernière image de la fenêtre ou de l'écran partagé. Les captures de fenêtre sont débarrassées
des bandes noires que certains compositeurs ajoutent autour.</li>
<li>La position du clic n'est pas connue : pas de cercle rouge sur les captures, pas de coordonnées dans
<code>steps.json</code>.</li>
<li>Le titre et l'application de la fenêtre ne sont pas exposés ; la liste des étapes affiche
<em>Fenêtre : inconnue (non exposée sous Wayland)</em>.</li>
<li>Les boutons gauche, droit et du milieu de la souris sont enregistrés.</li>
</ul>

<h2>Compiler sous Linux</h2>
<p>Le chemin de capture Wayland demande les en-têtes PipeWire et clang (pour bindgen). Sous Debian ou Ubuntu :</p>
<pre><code>sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang</code></pre>
<p>Voir <a href="{p('download')}#cargo">installer avec Cargo</a> pour la liste complète utilisée par le processus de
publication.</p>
""")

    if page == "faq":
        return faq_page(lang)
    raise KeyError(page)


def faq_items(lang):
    """(question, answer HTML) pairs; answers stay short and sourced."""
    p = lambda name: href(name, lang, lang)  # noqa: E731
    if lang == "en":
        return [
            ("Is Support Step Recorder a replacement for psr.exe?",
             "<p>It follows the same idea: a screenshot at each click, a readable description of every step and a "
             "single archive to send. It differs on the points below: it runs on Windows, macOS and Linux, it records "
             "the text typed between clicks as steps, and it writes an HTML report with WebP screenshots and a "
             "<code>steps.json</code>.</p>"),
            ("Is it free?",
             f"<p>Yes. It is open source under the MIT licence, and the <a href=\"{REPO}\">source code</a> is on GitHub.</p>"),
            ("Which systems does it run on?",
             f"<p>Windows 64-bit, macOS on Apple silicon and Intel, Linux on x86_64 and aarch64, under X11 or Wayland "
             f"(GNOME, KDE, wlroots…). See <a href=\"{p('download')}\">Download</a>.</p>"),
            ("Does it work on Wayland?",
             f"<p>Yes, with a <code>udev</code> rule to read clicks and keys, and your desktop's screen sharing "
             f"dialog for screenshots. See <a href=\"{p('linux-wayland')}\">Linux &amp; Wayland</a>.</p>"),
            ("What does the person receiving the report need?",
             "<p>A web browser. They extract the <code>.zip</code> and open <code>report.html</code>; the screenshots "
             "are the WebP files next to it.</p>"),
            ("Does ssr send anything over the internet?",
             "<p>No. It has no upload feature: the <code>.zip</code> is written to the folder you choose, and you "
             "decide who gets it.</p>"),
            ("Are passwords recorded?",
             "<p>Text typed between two clicks is recorded as it is typed, with no filtering. Stop the recording before "
             "typing anything private.</p>"),
            ("Why is some typed text wrong in the report?",
             "<p>Text is rebuilt from the keys pressed, based on a US keyboard layout. With another layout (AZERTY, "
             "for instance), some characters can come out different.</p>"),
            ("Why no red ring and no window title under Wayland?",
             "<p>Wayland does not give the click position or the active window to applications. The screenshot then "
             "comes from the window or screen you chose in the sharing dialog.</p>"),
            ("Can I add comments or edit steps?",
             "<p>No. <code>ssr</code> records the steps as they happen and exports them as they are.</p>"),
            ("Does it record video?",
             "<p>No. It takes one screenshot per click, saved as a WebP image.</p>"),
            ("Where are screenshots kept during a recording?",
             "<p>In a <code>support-step-recorder</code> folder inside the system's temporary directory, until you "
             "save. That folder is emptied when the next recording starts.</p>"),
            ("Which languages are available?",
             "<p>English and French, for the interface and the report. The language follows your locale and the "
             "<code>FR</code>/<code>EN</code> button switches it.</p>"),
            ("Where can I get help?",
             f"<p>On <a href=\"{DISCORD}\">Discord</a> or in <a href=\"{REPO}/issues\">GitHub issues</a>.</p>"),
        ]
    return [
        ("Support Step Recorder remplace-t-il psr.exe ?",
         "<p>Il suit le même principe : une capture à chaque clic, une description lisible de chaque étape et une seule "
         "archive à envoyer. Il s'en distingue sur les points ci-dessous : il fonctionne sous Windows, macOS et Linux, "
         "il enregistre le texte saisi entre les clics comme des étapes, et il écrit un rapport HTML avec des captures "
         "WebP et un fichier <code>steps.json</code>.</p>"),
        ("Est-il gratuit ?",
         f"<p>Oui. C'est un logiciel libre sous licence MIT, et le <a href=\"{REPO}\">code source</a> est sur GitHub.</p>"),
        ("Sous quels systèmes fonctionne-t-il ?",
         f"<p>Windows 64 bits, macOS sur Apple silicon et Intel, Linux sur x86_64 et aarch64, sous X11 ou Wayland "
         f"(GNOME, KDE, wlroots…). Voir <a href=\"{p('download')}\">Télécharger</a>.</p>"),
        ("Fonctionne-t-il sous Wayland ?",
         f"<p>Oui, avec une règle <code>udev</code> pour lire les clics et le clavier, et la fenêtre de partage d'écran "
         f"du bureau pour les captures. Voir <a href=\"{p('linux-wayland')}\">Linux et Wayland</a>.</p>"),
        ("De quoi a besoin la personne qui reçoit le rapport ?",
         "<p>D'un navigateur web. Elle décompresse le <code>.zip</code> et ouvre <code>report.html</code> ; les "
         "captures sont les fichiers WebP posés à côté.</p>"),
        ("ssr envoie-t-il quelque chose sur Internet ?",
         "<p>Non. Il n'a aucune fonction d'envoi : le <code>.zip</code> est écrit dans le dossier que vous choisissez, "
         "et c'est vous qui décidez à qui le transmettre.</p>"),
        ("Les mots de passe sont-ils enregistrés ?",
         "<p>Le texte tapé entre deux clics est enregistré tel quel, sans aucun filtre. Arrêtez l'enregistrement avant "
         "de saisir une information privée.</p>"),
        ("Pourquoi une partie du texte saisi est-elle fausse dans le rapport ?",
         "<p>Le texte est reconstitué à partir des touches pressées, d'après une disposition de clavier américaine "
         "(QWERTY US). Avec une autre disposition, AZERTY par exemple, certains caractères peuvent différer.</p>"),
        ("Pourquoi ni cercle rouge ni titre de fenêtre sous Wayland ?",
         "<p>Wayland ne communique pas aux applications la position du clic ni la fenêtre active. La capture provient "
         "alors de la fenêtre ou de l'écran choisi dans la fenêtre de partage.</p>"),
        ("Peut-on ajouter des commentaires ou modifier les étapes ?",
         "<p>Non. <code>ssr</code> enregistre les étapes telles qu'elles se déroulent et les exporte telles quelles.</p>"),
        ("Enregistre-t-il une vidéo ?",
         "<p>Non. Il prend une capture par clic, enregistrée en image WebP.</p>"),
        ("Où sont gardées les captures pendant l'enregistrement ?",
         "<p>Dans un dossier <code>support-step-recorder</code> du dossier temporaire du système, jusqu'à ce que vous "
         "enregistriez. Ce dossier est vidé au démarrage de l'enregistrement suivant.</p>"),
        ("Quelles langues sont disponibles ?",
         "<p>Le français et l'anglais, pour l'interface comme pour le rapport. La langue suit celle du système et le "
         "bouton <code>FR</code>/<code>EN</code> permet d'en changer.</p>"),
        ("Où trouver de l'aide ?",
         f"<p>Sur <a href=\"{DISCORD}\">Discord</a> ou dans les <a href=\"{REPO}/issues\">tickets GitHub</a>.</p>"),
    ]


def faq_page(lang):
    items = "".join(f"<h3>{html.escape(q)}</h3>\n{a}\n" for q, a in faq_items(lang))
    if lang == "en":
        return ("Problem Steps Recorder (psr.exe) replacement: FAQ and comparison – Support Step Recorder",
                "How Support Step Recorder compares with Windows Steps Recorder (psr.exe), and answers about "
                "platforms, Wayland, privacy, typed text and the HTML report.",
                f"""
<h1>FAQ and comparison with psr.exe</h1>
<p class="lead">Steps Recorder (<code>psr.exe</code>) and Support Step Recorder side by side, then the questions that
come up most.</p>

<div class="table"><table><thead><tr><th></th><th>Steps Recorder (psr.exe)</th><th>Support Step Recorder (ssr)</th></tr></thead><tbody>
<tr><td><strong>Systems</strong></td><td>Windows only</td><td>Windows, macOS, Linux (X11 and Wayland)</td></tr>
<tr><td><strong>Status</strong></td><td>Deprecated by Microsoft, to be removed from Windows</td><td>Open source, MIT licence</td></tr>
<tr><td><strong>Capture</strong></td><td>Screenshot at each click</td><td>Screenshot at each click, plus typed text as steps</td></tr>
<tr><td><strong>Output</strong></td><td>ZIP with an MHT report</td><td>ZIP with an HTML report, WebP screenshots and <code>steps.json</code></td></tr>
</tbody></table></div>

<div class="faq">
{items}</div>
""")
    return ("Remplacer l'Enregistreur d'actions utilisateur (psr.exe) : FAQ et comparatif – Support Step Recorder",
            "Support Step Recorder comparé à l'Enregistreur d'actions utilisateur de Windows (psr.exe), et réponses sur "
            "les systèmes, Wayland, la confidentialité, le texte saisi et le rapport HTML.",
            f"""
<h1>FAQ et comparaison avec psr.exe</h1>
<p class="lead">L'Enregistreur d'actions utilisateur (<code>psr.exe</code>) et Support Step Recorder côte à côte,
puis les questions qui reviennent le plus.</p>

<div class="table"><table><thead><tr><th></th><th>Enregistreur d'actions (psr.exe)</th><th>Support Step Recorder (ssr)</th></tr></thead><tbody>
<tr><td><strong>Systèmes</strong></td><td>Windows uniquement</td><td>Windows, macOS, Linux (X11 et Wayland)</td></tr>
<tr><td><strong>Statut</strong></td><td>Déprécié par Microsoft, appelé à disparaître de Windows</td><td>Logiciel libre, licence MIT</td></tr>
<tr><td><strong>Capture</strong></td><td>Capture à chaque clic</td><td>Capture à chaque clic, et texte saisi enregistré comme étape</td></tr>
<tr><td><strong>Résultat</strong></td><td>ZIP avec un rapport MHT</td><td>ZIP avec un rapport HTML, des captures WebP et <code>steps.json</code></td></tr>
</tbody></table></div>

<div class="faq">
{items}</div>
""")


# ---------------------------------------------------------------- layout

def json_ld(page, lang, title, description):
    app = {
        "@type": "SoftwareApplication",
        "name": "Support Step Recorder",
        "alternateName": "ssr",
        "description": description if page == "index" else (
            "Free, open-source alternative to Windows' Problem Steps Recorder (psr.exe)." if lang == "en" else
            "Alternative libre et gratuite à l'Enregistreur d'actions utilisateur de Windows (psr.exe)."),
        "operatingSystem": "Windows, macOS, Linux",
        "applicationCategory": "UtilitiesApplication",
        "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
        "isAccessibleForFree": True,
        "license": "https://opensource.org/licenses/MIT",
        "downloadUrl": url("download", lang),
        "codeRepository": REPO,
        "url": url("index", lang),
        "inLanguage": ["en", "fr"],
    }
    if page in ("index", "download"):
        return {"@context": "https://schema.org", **app}
    if page == "faq":
        return {"@context": "https://schema.org", "@type": "FAQPage", "name": title, "inLanguage": lang,
                "url": url(page, lang), "about": app,
                "mainEntity": [{"@type": "Question", "name": q,
                                "acceptedAnswer": {"@type": "Answer", "text": a}}
                               for q, a in faq_items(lang)]}
    return {"@context": "https://schema.org", "@type": "WebPage", "name": title, "description": description,
            "url": url(page, lang), "inLanguage": lang, "about": app}


def render(page, lang):
    title, description, body = content(page, lang)
    u = UI[lang]
    up = "../" if lang == "fr" else ""
    other, other_label, other_code = u["other"]
    nav = "".join(
        f'<a href="{href(n, lang, lang)}"{" aria-current=\"page\"" if n == page else ""}>{u["nav"][n]}</a>'
        for n in PAGES)
    ld = json.dumps(json_ld(page, lang, title, description), ensure_ascii=False).replace("</", "<\\/")
    return f"""<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)}</title>
<meta name="description" content="{html.escape(description)}">
<link rel="canonical" href="{url(page, lang)}">
<link rel="alternate" hreflang="en" href="{url(page, 'en')}">
<link rel="alternate" hreflang="fr" href="{url(page, 'fr')}">
<link rel="alternate" hreflang="x-default" href="{url(page, 'en')}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Support Step Recorder">
<meta property="og:title" content="{html.escape(title)}">
<meta property="og:description" content="{html.escape(description)}">
<meta property="og:url" content="{url(page, lang)}">
<meta property="og:image" content="{SITE}img/og.jpg">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:locale" content="{'fr_FR' if lang == 'fr' else 'en_GB'}">
<meta property="og:locale:alternate" content="{'en_GB' if lang == 'fr' else 'fr_FR'}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{html.escape(title)}">
<meta name="twitter:description" content="{html.escape(description)}">
<meta name="twitter:image" content="{SITE}img/og.jpg">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><circle cx='8' cy='8' r='7' fill='%23dc4646'/></svg>">
<link rel="stylesheet" href="{up}style.css">
<script type="application/ld+json">{ld}</script>
</head>
<body>
<header class="site"><div class="wrap">
<a class="brand" href="{href('index', lang, lang)}">Support Step <span>Recorder</span></a>
<nav class="main">{nav}</nav>
<a class="lang" href="{href(page, other, lang)}" hreflang="{other}"><img src="{up}img/{other_code.lower()}.svg" alt="{other_code}" width="21" height="14">{other_label}</a>
</div></header>
<main><div class="wrap">
{body.strip()}
</div></main>
<footer class="site"><div class="wrap">
<a href="{REPO}">{u["footer_src"]}</a>
<a href="{DISCORD}">{u["footer_chat"]}</a>
<span>{u["footer_note"]}</span>
</div></footer>
</body>
</html>
"""


def main():
    (DOCS / "fr").mkdir(exist_ok=True)
    for lang in ("en", "fr"):
        out = DOCS / ("fr" if lang == "fr" else "")
        for page in PAGES:
            (out / ("index.html" if page == "index" else page + ".html")).write_text(render(page, lang), encoding="utf-8")
    urls = []
    for page in PAGES:
        alts = "".join(f'<xhtml:link rel="alternate" hreflang="{l}" href="{url(page, l)}"/>' for l in ("en", "fr"))
        alts += f'<xhtml:link rel="alternate" hreflang="x-default" href="{url(page, "en")}"/>'
        for lang in ("en", "fr"):
            urls.append(f"<url><loc>{url(page, lang)}</loc>{alts}</url>")
    (DOCS / "sitemap.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n'
        + "\n".join(urls) + "\n</urlset>\n", encoding="utf-8")
    (DOCS / ".nojekyll").write_text("", encoding="utf-8")


if __name__ == "__main__":
    main()
