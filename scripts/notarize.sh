#!/bin/bash
# Sign + notarize + staple the staged release, then rebuild the zips.
# Run AFTER ./scripts/package.sh, from the repo root:
#
#   SIGN_ID="Developer ID Application: Your Name (TEAMID)" \
#   NOTARY_PROFILE="redeemer-notary" \
#   ./scripts/notarize.sh
#
# One-time setup (see README-RELEASING.md):
#   1. Create a "Developer ID Application" certificate in Xcode
#      (Settings -> Accounts -> Manage Certificates -> + ).
#   2. Create an app-specific password at appleid.apple.com, then:
#      xcrun notarytool store-credentials "redeemer-notary" \
#          --apple-id you@example.com --team-id TEAMID
set -euo pipefail
cd "$(dirname "$0")/.."

STAGE="dist/RE-DEEMER"
: "${SIGN_ID:?Set SIGN_ID to your 'Developer ID Application: ...' identity}"
: "${NOTARY_PROFILE:?Set NOTARY_PROFILE to your notarytool keychain profile}"
[ -d "$STAGE" ] || { echo "Run ./scripts/package.sh first."; exit 1; }

BUNDLES=("$STAGE/RE-DEEMER.clap" "$STAGE/RE-DEEMER.vst3" "$STAGE/RE-DEEMER.component")

echo "== codesign (hardened runtime, timestamped) =="
for b in "${BUNDLES[@]}"; do
    codesign --force --options runtime --timestamp -s "$SIGN_ID" "$b"
    codesign --verify --strict "$b" && echo "  signed: $b"
done

echo "== notarize =="
ditto -c -k --keepParent "$STAGE" dist/notarize-upload.zip
xcrun notarytool submit dist/notarize-upload.zip \
    --keychain-profile "$NOTARY_PROFILE" --wait
rm dist/notarize-upload.zip

echo "== staple tickets =="
for b in "${BUNDLES[@]}"; do
    xcrun stapler staple "$b"
done

echo "== rebuild distribution zips (now notarized) =="
# Use Info-ZIP (`zip`), NOT `ditto`: ditto stores macOS extended attributes
# (incl. the un-strippable com.apple.provenance) as AppleDouble ._* entries,
# and plain `unzip` materializes those as junk files INSIDE the signed
# bundles — codesign then reports "a sealed resource is missing or invalid".
# Nothing in the bundles needs xattrs (signature and stapler ticket are
# regular files), and zip stores none, so the archive extracts clean with
# Archive Utility, ditto and unzip alike.
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
rm -f dist/RE-DEEMER-macos.zip "dist/RE-DEEMER-${VERSION}-macos.zip"
(cd dist && zip -qry "RE-DEEMER-macos.zip" "RE-DEEMER")
cp dist/RE-DEEMER-macos.zip "dist/RE-DEEMER-${VERSION}-macos.zip"

echo "== verify the zip round-trips with a clean seal =="
rm -rf dist/.verify && mkdir dist/.verify
(cd dist/.verify && unzip -qq ../RE-DEEMER-macos.zip)
for b in dist/.verify/RE-DEEMER/RE-DEEMER.{clap,vst3,component}; do
    codesign --verify --strict "$b" || { echo "ERROR: seal broken after zip: $b"; exit 1; }
    xcrun stapler validate "$b" >/dev/null || { echo "ERROR: ticket missing after zip: $b"; exit 1; }
    echo "  zip round-trip OK: $b"
done
rm -rf dist/.verify

echo
echo "done — notarized archives:"
ls -la dist/*.zip
echo
echo "Verify on a clean account with:  spctl -a -t open --context context:primary-signature -v <bundle>"
