#!/usr/bin/env bash
#
# Dev install — build the RE-DEEMER bundle and drop the CLAP + VST3 into the
# system plugin folders so your host loads the fresh build on next rescan.
#
#   ./scripts/dev-install.sh                          # clean release build
#   ./scripts/dev-install.sh --features pointer_probe # forward extra cargo args
#
# Run ./scripts/dev-install-setup.sh ONCE first to grant write access to the
# system folders; after that this needs no password. (If you skip the setup it
# falls back to sudo per copy, which will prompt.)
#
set -euo pipefail

CLAP_DIR="/Library/Audio/Plug-Ins/CLAP"
VST3_DIR="/Library/Audio/Plug-Ins/VST3"

cd "$(dirname "$0")/.."
root="$(pwd)"

echo "▶ Building RE-DEEMER (universal, release) ${*:-}"
cargo xtask bundle-universal te2-plugin --release "$@"

src="$root/target/bundled"
[ -d "$src/RE-DEEMER.clap" ] || { echo "✗ no CLAP bundle in $src"; exit 1; }
[ -d "$src/RE-DEEMER.vst3" ] || { echo "✗ no VST3 bundle in $src"; exit 1; }

install_one() {
	local from="$1" to_dir="$2" name="$3"
	local dst="$to_dir/$name"
	if [ -w "$to_dir" ]; then
		rm -rf "$dst" && cp -R "$from" "$dst"
	else
		echo "  (no write access to $to_dir — using sudo; run dev-install-setup.sh once to skip this)"
		sudo rm -rf "$dst" && sudo cp -R "$from" "$dst"
	fi
	echo "  ✓ $dst"
}

echo "▶ Installing"
install_one "$src/RE-DEEMER.clap" "$CLAP_DIR" "RE-DEEMER.clap"
install_one "$src/RE-DEEMER.vst3" "$VST3_DIR" "RE-DEEMER.vst3"
echo "✓ Done — rescan plugins (or restart your host) to load it."
