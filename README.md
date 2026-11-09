# Support Step Recorder (`ssr`)

> Cross-platform user-action recorder written in Rust — an open clone of Windows'
> *Problem Steps Recorder* (`psr.exe`) to document a bug or a step-by-step how-to.

**[🇬🇧 English](#-english) · [🇫🇷 Français](#-français)**

## Community & support

Questions, bug reports, beta testing, or just want to chat? Join the Discord:

[![Discord](https://img.shields.io/badge/Discord-Le--Syl21%20Tools-5865F2?logo=discord&logoColor=white)](https://discord.gg/T37DYHmt2j)

---

## 🇬🇧 English

**Website**: [le-syl21.github.io/support-step-recorder](https://le-syl21.github.io/support-step-recorder/)
(download, how-to, Linux/Wayland setup, FAQ).

On every click, `ssr` captures the relevant window, records the **mouse button**,
aggregates **typed text** between clicks, and — when available — the **active
window** info. When you stop, you land in the **guide editor**: rename and
describe steps, reorder or delete them, combine several recordings, then save an
editable **project** (`.ssrproj`) and export a `.zip` bundling the screenshots, a
`steps.json`, and a self-contained, replayable **HTML report**.

The interface is available in **English and French** (auto-detected from your
locale, switchable with the `FR`/`EN` button).

### Features

- **Capture on click**: grabs the state *at* the click (like a real PSR), so a
  filled form is captured *before* it is submitted.
- **Wayland-native**: screen capture via `xdg-desktop-portal` + **PipeWire**
  (works on GNOME, KDE, wlroots…), with a one-time **consent dialog** and a
  reusable token so it isn't asked again.
- **Content-cropped** screenshots: window captures are trimmed to their content
  (no black padding), keeping files small.
- **Never blocks**: WebP encoding and disk writes happen on a background writer
  thread — clicking stays instant.
- **Replayable HTML report**: step-by-step replay mode, click any screenshot to
  **zoom** it, keyboard navigation, printable.
- **Guide editor**: custom step titles and multi-line instructions, drag and
  drop (or Alt+↑/↓) to reorder, delete a step or only its screenshot, add
  PNG/JPEG/WebP images or written steps, redact typed text, undo/redo.
- **Reuse earlier recordings**: open a ZIP exported by v0.2.0 or later, or import
  chosen steps from several recordings into one master guide. Source archives
  are never modified; same-named screenshots never collide.
- **Projects**: save the guide as `.ssrproj`, reopen it later, keep editing;
  exporting a ZIP is a separate action. Unsaved work is asked about before it is
  replaced, and recovered after a crash.
- **Multi-OS by design**: Windows, macOS, Linux (X11 & Wayland). The macOS window capture
  builds in CI but has not been tried on a real Mac yet: reports welcome.

### Capture backends

Input and screen capture are abstracted behind backends, chosen at compile time
(`cfg`) and at runtime (Wayland vs. the rest):

| Component | Windows / macOS / Linux X11 | Linux Wayland |
|---|---|---|
| **Input** (clicks / keys) | [`device_query`](https://crates.io/crates/device_query) | **`evdev`** (`/dev/input`) |
| **Screen capture** | [`xcap`](https://crates.io/crates/xcap) (active window) | **`xdg-desktop-portal` + PipeWire** (user picks a window or screen) |

> **Why a dedicated Wayland path?** Wayland forbids passive global input capture
> by a normal client, so we read `/dev/input` directly (needs a `udev` rule, see
> [Linux/Wayland setup](#linuxwayland-setup)). And screen capture must go through
> the desktop portal + PipeWire — `xcap`'s Wayland path only works on wlroots
> compositors, not GNOME/KDE, so we use the portal for **all** Wayland.

### Workflow

1. **Start** — screenshots are written to a **private** working folder
   (`~/.cache/ssr` on Linux, mode 0700).
2. Do the actions you want to document (clicks, typing).
3. **Stop** — you land in the **editor**, nothing is written elsewhere yet.
   Rename steps, add instructions, reorder, delete, import steps from other
   recordings (**Import…**), add images.
4. **Save project** (Ctrl+S) writes an editable `.ssrproj`; **Export guide…**
   (Ctrl+E) writes the shareable `.zip`. Both write a temporary file first and
   replace the target only once complete.

| File | Contents | Use |
|---|---|---|
| `.ssrproj` | `project.json`, `steps.json`, `assets/<sha256>.<ext>` | Reopen and keep editing |
| `.zip` (export) | `report.html`, `steps.json`, the referenced images | Send to support / colleagues |

`ssr file.ssrproj` or `ssr recording.zip` opens a file at start. Imported
archives are treated as untrusted: only `steps.json` and the images it names are
read (never `report.html`), paths, sizes and image formats are checked.

### Build & run

```bash
cargo run -p support-step-recorder-gui      # or: cargo run --bin ssr
cargo test --workspace    # run the tests
```

**Linux build dependencies** (for the Wayland capture path): `libpipewire-0.3-dev`,
`libspa-0.2-dev`, `clang` (bindgen). On Debian/Ubuntu:

```bash
sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang
```

### Linux/Wayland setup

The `evdev` backend reads `/dev/input`. To grant access **without** adding your
user to the `input` group, install the provided `udev` rule:

```bash
sudo ./packaging/install-linux.sh
```

It installs `/etc/udev/rules.d/60-ssr-input.rules` (`TAG+="uaccess"`), granting
the active-session user read access to input devices. The `60-` prefix is
required: the `uaccess` tag must be set *before* systemd's seat rules apply.

> ⚠️ **Security**: this lets apps run by the active user read keyboard/mouse
> input — the assumed trade-off for a PSR-style recorder on Wayland. Screen
> capture additionally goes through the portal (consent popup, reusable token).

The rule is only needed to **record**: opening, editing, merging, saving and
exporting guides work without it.

### RHEL 10

Release builds include `ssr-<version>-rhel10-<arch>.tar.gz`, built against RHEL
10's glibc and PipeWire. Same install flow:

```bash
tar xzf ssr-<version>-rhel10-x86_64.tar.gz && cd ssr-<version>-rhel10-x86_64
sudo ./packaging/install-linux.sh   # for recording on Wayland
./ssr
```

To build it yourself (RHEL 10 with CRB, or any EL10 rebuild), or from any
machine with podman/docker, including air-gapped builds, see
[`packaging/rhel/README-RHEL10.md`](packaging/rhel/README-RHEL10.md).

### License

MIT.

---

## 🇫🇷 Français

**Site web** : [le-syl21.github.io/support-step-recorder/fr](https://le-syl21.github.io/support-step-recorder/fr/)
(téléchargement, mode d'emploi, mise en place Linux/Wayland, FAQ).

À chaque clic, `ssr` capture la fenêtre concernée, relève le **bouton de souris**,
agrège le **texte saisi** entre deux clics et — quand c'est disponible — les
infos de la **fenêtre active**. À l'arrêt, vous arrivez dans l'**éditeur de
guide** : renommez et décrivez les étapes, réordonnez-les ou supprimez-les,
combinez plusieurs enregistrements, puis enregistrez un **projet** éditable
(`.ssrproj`) et exportez un `.zip` regroupant les captures, un `steps.json` et un
**rapport HTML** autoportant et rejouable.

L'interface est disponible en **français et anglais** (détectée depuis la locale,
basculable via le bouton `FR`/`EN`).

### Fonctionnalités

- **Capture au clic** : saisit l'état *au moment* du clic (comme un vrai PSR) —
  un formulaire rempli est donc capturé *avant* son envoi.
- **Natif Wayland** : capture d'écran via `xdg-desktop-portal` + **PipeWire**
  (GNOME, KDE, wlroots…), avec un **dialogue de consentement** unique et un jeton
  réutilisable pour ne plus le redemander.
- **Recadrage au contenu** : les captures de fenêtre sont rognées sur leur
  contenu (pas de bandes noires), fichiers plus légers.
- **Ne bloque jamais** : l'encodage WebP et l'écriture disque se font dans un
  thread d'écriture dédié — le clic reste instantané.
- **Rapport HTML rejouable** : mode relecture pas à pas, **zoom** au clic sur une
  capture, navigation clavier, imprimable.
- **Éditeur de guide** : titres d'étape personnalisés et consignes sur plusieurs
  lignes, glisser-déposer (ou Alt+↑/↓) pour réordonner, suppression d'une étape
  ou de sa seule capture, ajout d'images PNG/JPEG/WebP ou d'étapes rédigées,
  masquage du texte saisi, annuler/rétablir.
- **Réutiliser d'anciens enregistrements** : ouvrir un ZIP exporté par la v0.2.0
  ou plus récente, ou importer des étapes choisies de plusieurs enregistrements
  dans un guide unique. Les archives d'origine ne sont jamais modifiées ; deux
  captures de même nom ne s'écrasent jamais.
- **Projets** : enregistrer le guide en `.ssrproj`, le rouvrir plus tard et
  continuer ; l'export ZIP est une action distincte. Un travail non enregistré
  est signalé avant d'être remplacé, et récupéré après un plantage.
- **Multi-OS par conception** : Windows, macOS, Linux (X11 et Wayland). La capture de
  fenêtre macOS compile en CI mais n'a pas encore été essayée sur un vrai Mac : vos retours sont bienvenus.

### Backends de capture

Les entrées et la capture d'écran sont abstraites par backend, choisi à la
compilation (`cfg`) et à l'exécution (Wayland ou non) :

| Brique | Windows / macOS / Linux X11 | Linux Wayland |
|---|---|---|
| **Entrées** (clics / clavier) | [`device_query`](https://crates.io/crates/device_query) | **`evdev`** (`/dev/input`) |
| **Capture écran** | [`xcap`](https://crates.io/crates/xcap) (fenêtre active) | **`xdg-desktop-portal` + PipeWire** (l'utilisateur choisit une fenêtre ou l'écran) |

> **Pourquoi un chemin Wayland dédié ?** Wayland interdit la capture passive
> globale des entrées par un client classique : on lit donc directement
> `/dev/input` (règle `udev` requise, cf. [Mise en place](#mise-en-place-linuxwayland)).
> Et la capture d'écran passe obligatoirement par le portail + PipeWire — le
> chemin Wayland de `xcap` ne marche que sur les compositeurs wlroots, pas
> GNOME/KDE ; on utilise donc le portail pour **tout** Wayland.

### Déroulé

1. **Démarrer** — les captures sont écrites dans un dossier de travail **privé**
   (`~/.cache/ssr` sous Linux, mode 0700).
2. Réaliser les actions à documenter (clics, saisies).
3. **Arrêter** — vous arrivez dans l'**éditeur**, rien n'est encore écrit
   ailleurs. Renommez, ajoutez des consignes, réordonnez, supprimez, importez des
   étapes d'autres enregistrements (**Importer…**), ajoutez des images.
4. **Enregistrer le projet** (Ctrl+S) écrit un `.ssrproj` éditable ; **Exporter le
   guide…** (Ctrl+E) écrit le `.zip` à partager. Les deux écrivent d'abord un
   fichier temporaire et ne remplacent la cible qu'une fois complet.

| Fichier | Contenu | Usage |
|---|---|---|
| `.ssrproj` | `project.json`, `steps.json`, `assets/<sha256>.<ext>` | Rouvrir et continuer l'édition |
| `.zip` (export) | `report.html`, `steps.json`, les images référencées | Envoyer au support / à des collègues |

`ssr fichier.ssrproj` ou `ssr enregistrement.zip` ouvre un fichier au démarrage.
Les archives importées sont considérées comme non fiables : seuls `steps.json` et
les images qu'il nomme sont lus (jamais `report.html`) ; chemins, tailles et
formats d'image sont vérifiés.

### Compilation & lancement

```bash
cargo run -p support-step-recorder-gui      # ou : cargo run --bin ssr
cargo test --workspace    # lancer les tests
```

**Dépendances de build Linux** (chemin de capture Wayland) : `libpipewire-0.3-dev`,
`libspa-0.2-dev`, `clang` (bindgen). Sur Debian/Ubuntu :

```bash
sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang
```

### Mise en place Linux/Wayland

Le backend `evdev` lit `/dev/input`. Pour y donner accès **sans** ajouter
l'utilisateur au groupe `input`, installez la règle `udev` fournie :

```bash
sudo ./packaging/install-linux.sh
```

Elle pose `/etc/udev/rules.d/60-ssr-input.rules` (`TAG+="uaccess"`), qui accorde à
l'utilisateur de la session active un accès en lecture aux périphériques
d'entrée. Le préfixe `60-` est indispensable : le tag `uaccess` doit être posé
*avant* les règles de seat de systemd.

> ⚠️ **Sécurité** : cela autorise les applications de l'utilisateur actif à lire
> les entrées clavier/souris — le compromis assumé pour un enregistreur de type
> PSR sous Wayland. La capture d'**écran** passe en plus par le portail (popup de
> consentement, jeton réutilisable).

La règle n'est nécessaire que pour **enregistrer** : ouvrir, éditer, combiner,
enregistrer et exporter des guides fonctionne sans elle.

### RHEL 10

Les releases incluent `ssr-<version>-rhel10-<arch>.tar.gz`, compilé contre la
glibc et PipeWire de RHEL 10. Même installation :

```bash
tar xzf ssr-<version>-rhel10-x86_64.tar.gz && cd ssr-<version>-rhel10-x86_64
sudo ./packaging/install-linux.sh   # pour enregistrer sous Wayland
./ssr
```

Pour le compiler soi-même (RHEL 10 avec CRB, ou une recompilation EL10), ou
depuis n'importe quelle machine avec podman/docker, y compris hors ligne, voir
[`packaging/rhel/README-RHEL10.md`](packaging/rhel/README-RHEL10.md).

### Licence

MIT.
