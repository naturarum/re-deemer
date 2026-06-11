#!/bin/bash
# RE-DEEMER installer: copies the plugins into your user plugin folders and
# clears the macOS download-quarantine flag so your DAW will load them.
# Run from inside the unzipped RE-DEEMER folder:  ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

CLAP_DIR="$HOME/Library/Audio/Plug-Ins/CLAP"
VST3_DIR="$HOME/Library/Audio/Plug-Ins/VST3"
AU_DIR="$HOME/Library/Audio/Plug-Ins/Components"
mkdir -p "$CLAP_DIR" "$VST3_DIR" "$AU_DIR"

echo "Installing RE-DEEMER…"
rm -rf "$CLAP_DIR/RE-DEEMER.clap" "$VST3_DIR/RE-DEEMER.vst3" "$AU_DIR/RE-DEEMER.component"
cp -R "RE-DEEMER.clap" "$CLAP_DIR/"
cp -R "RE-DEEMER.vst3" "$VST3_DIR/"
cp -R "RE-DEEMER.component" "$AU_DIR/"

echo "Clearing macOS quarantine…"
xattr -dr com.apple.quarantine \
    "$CLAP_DIR/RE-DEEMER.clap" \
    "$VST3_DIR/RE-DEEMER.vst3" \
    "$AU_DIR/RE-DEEMER.component" 2>/dev/null || true

echo
echo "Done. Rescan plugins in your DAW (or restart it)."
echo "Note: the AU loads the CLAP at runtime — keep both installed."
echo "You waited long enough."
