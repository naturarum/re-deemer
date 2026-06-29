# Releasing RE-DEEMER — the runbook

Everything from a clean checkout to a published, notarized release.

## One-time setup (do once, ~10 minutes)

### A. Apple notarization credentials

1. **Developer ID certificate** — open Xcode → Settings → Accounts → select
   your team → Manage Certificates → **+** → **Developer ID Application**.
   (Your keychain currently only has *Apple Development* certs; those can't
   notarize.) Confirm with:
   ```
   security find-identity -v -p codesigning | grep "Developer ID"
   ```
2. **App-specific password** — at <https://account.apple.com> →
   Sign-In and Security → App-Specific Passwords → create one named
   "redeemer notary".
3. **Store it for notarytool** (password is kept in your keychain, never in
   the repo):
   ```
   xcrun notarytool store-credentials "redeemer-notary" \
       --apple-id YOUR-APPLE-ID-EMAIL --team-id YOURTEAMID
   ```
   (Team ID is the 10-character code shown next to your team in Xcode.)

### B. GitHub

1. Log in (browser flow — credentials never touch this repo or any agent):
   ```
   gh auth login --web
   ```
2. Create the repo and push:
   ```
   gh repo create re-deemer --public --source . --push
   ```
3. Enable the website: repo → Settings → Pages → Source: **Deploy from a
   branch** → branch `main`, folder **/docs**.
4. Fill the placeholders in `site/index.html` (GitHub repo path ×3 and the
   PayPal handle), commit, push.

## Every release

```
# 1. bump version in Cargo.toml ([workspace.package]) AND in
#    wrapper-au/CMakeLists.txt (project VERSION + BUNDLE_VERSION — these feed the
#    AU component version, and package.sh aborts if they disagree with the crate
#    version). Add a CHANGELOG entry. (docs/version.json — the manifest the
#    in-plugin update notifier polls — is regenerated from the version by
#    package.sh in step 2, so there's nothing to edit there.)
# 2. build, test, package:
./scripts/package.sh

# 3. sign + notarize + staple (uses your keychain):
SIGN_ID="Developer ID Application: Your Name (TEAMID)" \
NOTARY_PROFILE="redeemer-notary" ./scripts/notarize.sh

# 4. tag + release with the notarized archive:
git add -A && git commit -m "vX.Y.Z"
git tag vX.Y.Z && git push origin main vX.Y.Z
gh release create vX.Y.Z dist/RE-DEEMER-macos.zip \
    "dist/RE-DEEMER-X.Y.Z-macos.zip" --notes-file CHANGELOG.md
```

The website's download button points at
`releases/latest/download/RE-DEEMER-macos.zip`, so publishing the release is
all it takes — the site updates itself.

> The GitHub Actions workflow (`.github/workflows/release.yml`) can build a
> draft release automatically on tag push, but it produces *unsigned*
> builds — CI has no access to your certificate. Until signing secrets are
> added to CI, prefer the local flow above and upload the notarized zip
> (`gh release create` / `gh release upload --clobber`). If a CI draft for
> the same tag exists, replace its assets with the notarized ones before
> publishing.

## Sanity checks before publishing

- `spctl -a -t open --context context:primary-signature -v ~/Library/Audio/Plug-Ins/CLAP/RE-DEEMER.clap`
  should say `accepted`.
- Download the zip from the draft release in a browser (so it gets the
  quarantine flag), unzip, install, and load it in a DAW — the true
  end-user path.
