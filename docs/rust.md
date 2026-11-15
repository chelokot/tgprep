# Rust port

The app is Rust: a Leptos (CSR) UI compiled to WebAssembly, a second small wasm bundle as the image Web Worker, and an axum server for the built files. The TypeScript app (`c954212` on `main`) was the specification; it is deleted on this branch. Its production build is kept at `~/.cache/tgprep-ts-dist` for measurements (serve it with the Rust server, `TGPREP_CLIENT_DIR=~/.cache/tgprep-ts-dist`).

## Layout

```
crates/model   doc v1 types (serde, IndexMap key order, JS number output), ops, crop geometry; pure, tested
crates/web     the UI (Leptos 0.8, csr): store, persistence, thumbs, renderer (WebGL2), export, dnd, panes, viewer
crates/worker  lib: the worker message protocol; bin: the worker (createImageBitmap, OffscreenCanvas webp thumbs, view bitmaps)
crates/server  axum + rustls; serves dist from memory with brotli/gzip, immutable /assets, no-cache pages
```

CSR only: there is no server data (everything comes from the folder the browser opens), so SSR and hydration would only add weight.

## Build and run

```
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$HOME/.cache/tgprep-target RUSTC_WRAPPER=$HOME/.cargo/bin/sccache
crates/web/build.sh            # writes dist/
cargo build --release -p tgprep-server
TGPREP_LISTEN=127.0.0.1:5180 TGPREP_CLIENT_DIR=dist target/release/tgprep-server
```

`TGPREP_TLS_CERT` and `TGPREP_TLS_KEY` (PEM) switch the server to HTTPS; without them it serves plain HTTP. The server reads `dist` once at start, so restart it after a rebuild.

## Tests

- `cargo test --workspace`: model ops (the TS unit tests plus more), `.tgprep.json` compatibility (`crates/model/tests/fixtures`: a v1 input, its zod-parsed form and a file the TS app wrote after an edit session, all rewritten byte for byte), geometry against 216 cases evaluated by the TS code in Chromium, server routes.
- e2e: build `dist` first, then `node node_modules/@playwright/test/cli.js test` starts the server with `cargo run --release -p tgprep-server` on 127.0.0.1:5179 and runs `tests/e2e`; `TGPREP_URL=…` points it at a running server instead.
- Perf: `tests/parity/perf.spec.ts`, see `docs/parity.md`.
- Screens: `TGPREP_URL=… TGPREP_LABEL=rust … test -c tests/parity/playwright.config.ts screens`, then `… compare` writes side-by-side sheets (TS, Rust, diff) and `report.tsv` to `~/.cache/tgprep-parity/screens/compare`; `TGPREP_CROP=name,x,y,w,h … crop` zooms a region. Kept sheets are in `docs/parity/`.

## Deliberate differences

- A commit that changes nothing adds no undo step (immer made a new doc for every `detach`).
- An edited thumbnail that is still rendering shows the loading pulse instead of the broken-image icon.
- A worker joins the pool only after it posts `ready`; thumbnail responses from a folder opened before are dropped.
- The drop target is resolved on every pointer move as well as every frame (the TS app resolved it only in the frame callback).
- Thumbnails: visible tiles first (newest request first, as before); idle workers then read ahead 8 screens below and 2 above the library window, nearest first. TS read ahead only by accident (about 144 tiles mounted in its first render).
- Thumbnail size follows the largest tile on screen: 1.5 × the tile's short side (library cell, post tile height) × `devicePixelRatio`, rounded up to 256 px, at least 512, and it only grows; mounted tiles ask again when it grows. TS used a fixed 720 px, which a 24 rem library cell or a 20 rem post tile upscales on a 2x screen. The cache key includes the size.
- A photo whose file cannot be read any more shows the broken-image icon; in TS its worker never came back to the pool, so after four such files no thumbnail was made.
- Geometry uses exact doubles; Chromium's DOMMatrix has about 1e-8 of float noise (fixture tolerance 1e-6).

## Gotchas

- `view!` attribute values with `<`, `>`, `>=` or a bare `if` must be braced (`disabled=move || { a >= b }`, `class={if … }`), or the macro silently splits the tag.
- JSX `{a} text {b}` is several text nodes; mirror that (`"#"{n}`) for pixel parity.
- Closures handed to the browser are dropped in a later task (`browser::retire`), never inside their own call.
- `on_cleanup` needs `Send`; hold non-Send resources (listeners, observers) in `StoredValue::new_local` so they drop with the owner.
- wasm builds need `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS=-Cdebuginfo=0` (build.sh sets it).

## Status (2026-11-15)

Done: model, server, worker, full UI, TS app removed; fmt, host and wasm clippy and `cargo test --workspace` clean at 607a6c6 (rerun them after later changes); e2e 10/10 (×3); perf harness and `docs/parity.md`.

The sub-threshold text-selection bug reported earlier does not exist: the parity script pressed `g.jpg` at y = 1734 in an 1180 px tall viewport (on `<html>`) and dragged a plain text selection; `screens.spec.ts` now scrolls the tile into view, and an e2e test guards that dragging a photo over text selects nothing.

Next:
1. Edit slider is a frame slower than TS inside the perf harness only (see `docs/parity.md`); find out why before deciding anything on it.
2. Decide on `--cfg erase_components` (10% smaller wasm, no measured cost) and wire it into `build.sh` if kept; the nightly build-std option is listed in `docs/parity.md`.
3. Stress pass: script 300 photos into 30 posts (drag, reorder, captions, edits, archive, send) and check fit at 1366×768, 1600×1000, 820×1180.
4. Rerun the parity screens for both builds (`screens.spec.ts` changed: the 820 drag is now a real drag).
5. Release `dist`, release server binary copied out of the target dir, delete the target dir.
