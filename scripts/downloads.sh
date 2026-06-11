#!/bin/bash
# Download counts per release asset — the website's download button serves
# the versioned asset, so these counts split by version.
#   ./scripts/downloads.sh
set -euo pipefail
gh api repos/naturarum/re-deemer/releases --paginate \
    --jq '.[] | .tag_name as $t | .assets[] | "\($t)\t\(.name)\t\(.download_count)"' \
    | column -t -s $'\t'
