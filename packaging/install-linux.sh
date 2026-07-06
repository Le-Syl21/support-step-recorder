#!/usr/bin/env bash
# Installe la règle udev permettant à Support Step Recorder de lire les
# périphériques /dev/input (backend evdev) sous Wayland, sans ajouter
# l'utilisateur au groupe `input`. À lancer en administrateur (sudo).
set -euo pipefail

RULE_DST=/etc/udev/rules.d/60-ssr-input.rules
RULE_SRC="$(cd "$(dirname "$0")" && pwd)/60-ssr-input.rules"

if [ "$(id -u)" -ne 0 ]; then
    echo "Ce script doit être lancé en root : sudo $0" >&2
    exit 1
fi

# Nettoie une éventuelle ancienne version (numérotée 99, qui posait le tag
# `uaccess` trop tard pour que systemd-logind applique l'ACL).
rm -f /etc/udev/rules.d/99-ssr-input.rules

install -m 0644 "$RULE_SRC" "$RULE_DST"
udevadm control --reload-rules
udevadm trigger --subsystem-match=input

echo "✓ Règle installée : $RULE_DST"
echo "  Si la capture ne fonctionne pas tout de suite, reconnectez votre session."
echo "  Pour désinstaller : sudo rm $RULE_DST && sudo udevadm control --reload-rules \\"
echo "                      && sudo udevadm trigger --subsystem-match=input"
