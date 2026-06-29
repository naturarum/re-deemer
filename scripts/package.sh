#!/bin/bash
# Build everything and produce the release archive:
#   dist/RE-DEEMER-macos.zip      (stable name -> permanent download link)
#   dist/RE-DEEMER-<ver>-macos.zip (versioned copy for the archives)
#
# Run from the repo root: ./scripts/package.sh
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
echo "== RE-DEEMER v${VERSION} =="

# Keep the in-plugin update manifest in lockstep with this build's version, so
# the notifier points users at the latest release. Regenerated from VERSION on
# every package run — no manual edit, and it ships in the release commit.
printf '{\n  "latest": "%s",\n  "url": "https://naturarum.github.io/re-deemer/"\n}\n' \
    "$VERSION" > docs/version.json
echo "== update manifest -> docs/version.json (latest ${VERSION}) =="

echo "== tests =="
cargo test --workspace --release --quiet

echo "== universal CLAP + VST3 =="
cargo xtask bundle-universal te2-plugin --release

echo "== AUv2 wrapper =="
if [ ! -d wrapper-au/clap-wrapper ]; then
    git clone --depth 1 https://github.com/free-audio/clap-wrapper wrapper-au/clap-wrapper
fi
# The AUv2 build helper generates its registration plist by loading the
# INSTALLED CLAP — install the one we just built first, or the component
# inherits a stale version. And build the wrapper fresh: incremental builds
# have bitten before.
rm -rf ~/Library/Audio/Plug-Ins/CLAP/RE-DEEMER.clap
cp -R target/bundled/RE-DEEMER.clap ~/Library/Audio/Plug-Ins/CLAP/
rm -rf wrapper-au/build
cmake -B wrapper-au/build -S wrapper-au -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build wrapper-au/build >/dev/null

# The v1.0.0 zip shipped a component whose Info.plist had lost its
# AudioComponents block (clap-wrapper regenerates it only on first build).
# Never let that leave the building again — and check the version integer
# actually matches this release (the helper once shipped a stale one).
AU_PLIST="wrapper-au/build/RE-DEEMER.component/Contents/Info.plist"
if ! plutil -extract AudioComponents json -o /dev/null "$AU_PLIST" 2>/dev/null; then
    echo "ERROR: AU Info.plist is missing AudioComponents — the component" >&2
    echo "       would not register on user machines. Aborting." >&2
    exit 1
fi
EXPECTED_AUVER=$(echo "$VERSION" | awk -F. '{ print $1*65536 + $2*256 + $3 }')
ACTUAL_AUVER=$(plutil -extract AudioComponents.0.version raw -o - - < "$AU_PLIST")
if [ "$ACTUAL_AUVER" != "$EXPECTED_AUVER" ]; then
    echo "ERROR: AU component version is $ACTUAL_AUVER, expected $EXPECTED_AUVER" >&2
    echo "       (v${VERSION}). Stale build helper output? Aborting." >&2
    exit 1
fi

echo "== staging =="
STAGE="dist/RE-DEEMER"
rm -rf dist && mkdir -p "$STAGE"
cp -R "target/bundled/RE-DEEMER.clap" "$STAGE/"
cp -R "target/bundled/RE-DEEMER.vst3" "$STAGE/"
cp -R "wrapper-au/build/RE-DEEMER.component" "$STAGE/"
cp MANUAL.md PRESETS.md LICENSE "$STAGE/" 2>/dev/null || cp MANUAL.md PRESETS.md "$STAGE/"
cp scripts/install.sh "$STAGE/install.sh"
chmod +x "$STAGE/install.sh"
echo "RE-DEEMER v${VERSION} — $(date +%Y-%m-%d)" > "$STAGE/VERSION.txt"

echo "== zipping =="
(cd dist && zip -qry "RE-DEEMER-macos.zip" "RE-DEEMER")
cp "dist/RE-DEEMER-macos.zip" "dist/RE-DEEMER-${VERSION}-macos.zip"

echo
echo "done:"
ls -la dist/*.zip
