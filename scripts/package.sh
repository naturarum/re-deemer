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

echo "== tests =="
cargo test --workspace --release --quiet

echo "== universal CLAP + VST3 =="
cargo xtask bundle-universal te2-plugin --release

echo "== AUv2 wrapper =="
if [ ! -d wrapper-au/clap-wrapper ]; then
    git clone --depth 1 https://github.com/free-audio/clap-wrapper wrapper-au/clap-wrapper
fi
cmake -B wrapper-au/build -S wrapper-au -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build wrapper-au/build >/dev/null

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
