# Support Step Recorder (`ssr`)

> Cross-platform user-action recorder written in Rust — an open clone of Windows'
> *Problem Steps Recorder* (`psr.exe`) to document a bug or a step-by-step how-to.

**[🇬🇧 English](#-english) · [🇫🇷 Français](#-français)**

---

## 🇬🇧 English

On every click, `ssr` captures the relevant window, records the **mouse button**,
aggregates **typed text** between clicks, and — when available — the **active
window** info. When you stop, it asks **where to save** and writes a `.zip`
bundling the WebP screenshots, a `steps.json`, and a self-contained, replayable
**HTML report**.

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
  **zoom** it, keyboard navigation.
- **Multi-OS by design**: Windows, macOS, Linux (X11 & Wayland).

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

1. **Start** — screenshots are written to a **temporary** folder (volatile).
2. Do the actions you want to document (clicks, typing).
3. **Stop** — a native **“Save as…”** dialog opens; pick a location and the
   `ZIP + HTML` report is generated there. (Cancel → confirm discard or retry.)

The `.zip` contains `report.html`, `steps.json` and all `step-XXXX.webp`. The
HTML references the WebP files (they travel together in the zip).

### Build & run

```bash
cargo run -p ssr-gui      # or: cargo run --bin ssr
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

### License

MIT.

---

## 🇫🇷 Français

À chaque clic, `ssr` capture la fenêtre concernée, relève le **bouton de souris**,
agrège le **texte saisi** entre deux clics et — quand c'est disponible — les
infos de la **fenêtre active**. À l'arrêt, il demande **où enregistrer** et écrit
un `.zip` regroupant les captures WebP, un `steps.json` et un **rapport HTML**
autoportant et rejouable.

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
  capture, navigation clavier.
- **Multi-OS par conception** : Windows, macOS, Linux (X11 et Wayland).

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

1. **Démarrer** — les captures sont écrites dans un dossier **temporaire**
   (volatile).
2. Réaliser les actions à documenter (clics, saisies).
3. **Arrêter** — un dialogue natif **« Enregistrer sous… »** s'ouvre ; choisis un
   emplacement et le rapport `ZIP + HTML` y est généré. (Annuler → confirmer
   l'abandon ou réessayer.)

Le `.zip` contient `report.html`, `steps.json` et toutes les `step-XXXX.webp`. Le
HTML référence les WebP (ils voyagent ensemble dans le zip).

### Compilation & lancement

```bash
cargo run -p ssr-gui      # ou : cargo run --bin ssr
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

### Licence

MIT.
