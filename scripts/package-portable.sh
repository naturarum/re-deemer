#!/bin/bash
# Package the Windows or Linux build (run in CI after `cargo xtask bundle`):
#   bash scripts/package-portable.sh windows|linux
# Produces dist/RE-DEEMER-<platform>.zip (stable name) + versioned copy.
set -euo pipefail
cd "$(dirname "$0")/.."

PLATFORM="${1:?usage: package-portable.sh windows|linux}"
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')

STAGE="dist/RE-DEEMER"
rm -rf dist && mkdir -p "$STAGE"
cp -R target/bundled/RE-DEEMER.clap "$STAGE/"
cp -R target/bundled/RE-DEEMER.vst3 "$STAGE/"
cp MANUAL.md PRESETS.md LICENSE CHANGELOG.md "$STAGE/"

if [ "$PLATFORM" = "windows" ]; then
    cat > "$STAGE/INSTALL.txt" <<'TXT'
RE-DEEMER — Windows install
===========================
Copy the bundles into the standard plugin folders:

  RE-DEEMER.vst3  ->  C:\Program Files\Common Files\VST3\
  RE-DEEMER.clap  ->  C:\Program Files\Common Files\CLAP\

(Create the CLAP folder if it doesn't exist.) Then rescan plugins in
your DAW. These builds are unsigned: if SmartScreen objects to the
zip, choose "More info" -> "Run anyway" / unblock the file in its
Properties dialog.

Windows builds are new — if anything misbehaves, please open an issue:
https://github.com/naturarum/re-deemer/issues
TXT
else
    cat > "$STAGE/INSTALL.txt" <<'TXT'
RE-DEEMER — Linux install
=========================
Copy the bundles into your user plugin folders:

  RE-DEEMER.vst3  ->  ~/.vst3/
  RE-DEEMER.clap  ->  ~/.clap/

Then rescan plugins in your DAW (Bitwig, Reaper, Ardour, Qtractor...).

Linux builds are new — if anything misbehaves, please open an issue:
https://github.com/naturarum/re-deemer/issues
TXT
fi

echo "RE-DEEMER v${VERSION} (${PLATFORM}) — $(date +%Y-%m-%d)" > "$STAGE/VERSION.txt"

cd dist
if command -v zip >/dev/null 2>&1; then
    zip -qry "RE-DEEMER-${PLATFORM}.zip" "RE-DEEMER"
else
    7z a -tzip -bso0 "RE-DEEMER-${PLATFORM}.zip" "RE-DEEMER"
fi
cp "RE-DEEMER-${PLATFORM}.zip" "RE-DEEMER-${VERSION}-${PLATFORM}.zip"
ls -la ./*.zip
