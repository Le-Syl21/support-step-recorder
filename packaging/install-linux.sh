#!/usr/bin/env bash
# Installe la règle udev permettant à Support Step Recorder de lire les
# périphériques /dev/input (backend evdev) sous Wayland, sans ajouter
# l'utilisateur au groupe `input`. À lancer en administrateur (sudo).
#
# Depuis une archive de release (./ssr à côté de ./packaging/), installe aussi
# l'application : /usr/local/bin/ssr + l'entrée du menu des applications.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
RULE_DST=/etc/udev/rules.d/60-ssr-input.rules
RULE_SRC="$HERE/60-ssr-input.rules"
BIN_SRC="$HERE/../ssr"
BIN_DST=/usr/local/bin/ssr
DESKTOP_DST=/usr/local/share/applications/support-step-recorder.desktop
ICON_DST=/usr/local/share/icons/hicolor/scalable/apps/support-step-recorder.svg

if [ "$(id -u)" -ne 0 ]; then
    echo "Ce script doit être lancé en root : sudo $0" >&2
    exit 1
fi

# ── L'application (archive de release seulement) ────────────────────
if [ -f "$BIN_SRC" ]; then
    install -D -m 0755 "$BIN_SRC" "$BIN_DST"
    install -D -m 0644 "$HERE/support-step-recorder.desktop" "$DESKTOP_DST"
    install -D -m 0644 "$HERE/support-step-recorder.svg" "$ICON_DST"
    if command -v restorecon >/dev/null 2>&1; then
        restorecon "$BIN_DST" "$DESKTOP_DST" "$ICON_DST" || true
    fi
    command -v update-desktop-database >/dev/null 2>&1 \
        && update-desktop-database -q /usr/local/share/applications || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 \
        && gtk-update-icon-cache -q -t /usr/local/share/icons/hicolor || true
    echo "✓ Application installée / app installed: $BIN_DST"
    echo "  Lancer / launch: Activités → « Support Step Recorder », ou / or: ssr"
fi

# Nettoie une éventuelle ancienne version (numérotée 99, qui posait le tag
# `uaccess` trop tard pour que systemd-logind applique l'ACL).
rm -f /etc/udev/rules.d/99-ssr-input.rules

install -m 0644 "$RULE_SRC" "$RULE_DST"
# SELinux (RHEL, Fedora…) : remet le contexte par défaut du dossier des
# règles, au cas où le fichier source portait un autre label.
if command -v restorecon >/dev/null 2>&1; then
    restorecon "$RULE_DST" || true
fi
udevadm control --reload-rules
udevadm trigger --subsystem-match=input

echo "✓ Règle installée / rule installed: $RULE_DST"


echo
echo "  Reconnectez votre session pour activer la capture clavier/souris."
echo "  Log out and back in to enable keyboard/mouse capture."
echo "  Désinstaller / uninstall:"
echo "    sudo rm -f $RULE_DST $BIN_DST $DESKTOP_DST $ICON_DST"
echo "    sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=input"
