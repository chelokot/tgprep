#!/usr/bin/env bash
set -euo pipefail

web=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=${1:-$web/../../dist}
target=${CARGO_TARGET_DIR:-$web/../../target}
wasm=$target/wasm32-unknown-unknown/web

CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-Cdebuginfo=0 --cfg erase_components" \
  cargo build --profile web --target wasm32-unknown-unknown -p tgprep-web -p tgprep-worker --manifest-path "$web/Cargo.toml"
rm -rf "$out"
mkdir -p "$out/assets"
for bundle in tgprep-web:tgprep tgprep-worker:worker; do
  wasm-bindgen --target web --no-typescript --out-dir "$out/assets" --out-name "${bundle#*:}" "$wasm/${bundle%:*}.wasm"
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue \
    -o "$out/assets/${bundle#*:}_bg.wasm" "$out/assets/${bundle#*:}_bg.wasm"
done
(cd "$web" && tailwindcss --input style.css --output "$out/assets/tgprep.css" --minify --optimize 2>/dev/null)
cp "$web"/public/* "$out/"

hashed() {
  local stem=${1%.*} extension=${1##*.}
  local name=$stem-$(sha256sum "$out/assets/$1" | cut -c1-12).$extension
  mv "$out/assets/$1" "$out/assets/$name"
  printf '%s' "$name"
}

printf 'import start from "/assets/%s";\nstart({ module_or_path: "/assets/%s" });\n' \
  "$(hashed worker.js)" "$(hashed worker_bg.wasm)" > "$out/assets/worker-start.js"
html=$(<"$web/index.html")
for asset in tgprep.js tgprep_bg.wasm tgprep.css worker-start.js; do
  html=${html//\/assets\/$asset/\/assets\/$(hashed "$asset")}
done
printf '%s\n' "$html" > "$out/index.html"
