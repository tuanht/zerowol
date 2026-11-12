#!/bin/sh
# Rebuild the vendored, trimmed Framework7 bundle in crates/zerowol/web/vendor/.
# Usage: scripts/build-f7.sh [version]   (needs npm + npx; esbuild is fetched by npx)
set -eu
V="${1:-9.3.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/crates/zerowol/web/vendor"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# Component name -> import identifier.
COMPONENTS="dialog:Dialog toast:Toast toggle:Toggle input:Input fab:Fab preloader:Preloader accordion:Accordion card:Card"
CSS_ONLY="radio"
cd "$TMP"
echo '{"private":true}' > package.json
npm install --no-audit --no-fund --silent "framework7@$V" esbuild@0.24.0
P="$TMP/node_modules/framework7"
{
  echo "import Framework7 from 'framework7';"
  for pair in $COMPONENTS; do
    c="${pair%%:*}"; n="${pair##*:}"
    echo "import $n from 'framework7/components/$c';"
    echo "Framework7.use($n);"
  done
  echo "window.Framework7 = Framework7;"
} > entry.js
mkdir -p "$OUT"
npx esbuild entry.js --bundle --minify --format=iife --target=es2019 \
  --legal-comments=none --outfile=bundle.js --log-level=warning
{
  printf '/* Framework7 %s | MIT | https://framework7.io */\n' "$V"
  cat bundle.js
} > "$OUT/framework7.js"
{
  printf '/* Framework7 %s | MIT | https://framework7.io */\n' "$V"
  cat "$P/framework7.min.css"
  for pair in $COMPONENTS $CSS_ONLY; do
    c="${pair%%:*}"
    if [ -f "$P/components/$c/$c.css" ]; then cat "$P/components/$c/$c.css"; fi
  done
} > "$OUT/framework7.css"
cp "$P/LICENSE" "$OUT/FRAMEWORK7-LICENSE"
ls -l "$OUT"
