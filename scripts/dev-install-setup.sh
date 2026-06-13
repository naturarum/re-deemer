#!/usr/bin/env bash
#
# ONE-TIME setup for scripts/dev-install.sh.
#
# The system plugin folders (/Library/Audio/Plug-Ins/{CLAP,VST3}) are
# root-owned, so a normal user can't write to them. This grants your user
# ownership of just those two folders — nothing else — so future dev installs
# need no password. Asks for your admin password once.
#
#   ./scripts/dev-install-setup.sh
#
set -euo pipefail

CLAP_DIR="/Library/Audio/Plug-Ins/CLAP"
VST3_DIR="/Library/Audio/Plug-Ins/VST3"
me="$(whoami)"

echo "Granting '$me' write access to the system plugin folders (one sudo prompt)…"
# Create them if missing, then hand the directories (not their contents) to
# the user. Owning the directory is enough to add/replace entries inside it.
sudo mkdir -p "$CLAP_DIR" "$VST3_DIR"
sudo chown "$me" "$CLAP_DIR" "$VST3_DIR"

echo "✓ Done. You now own:"
ls -ld "$CLAP_DIR" "$VST3_DIR"
echo
echo "From now on, scripts/dev-install.sh installs without a password."
