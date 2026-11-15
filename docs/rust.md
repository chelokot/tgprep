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
crates/web/build.sh            # writes dist/: wasm32 profile `web` (opt z, fat LTO, panic abort), --cfg erase_components, wasm-bindgen, wasm-opt -Oz, Tailwind, hashed names
cargo build --release -p tgprep-server
TGPREP_LISTEN=127.0.0.1:5180 TGPREP_CLIENT_DIR=dist $CARGO_TARGET_DIR/release/tgprep-server
```

Server environment:

| variable | meaning |
|---|---|
| `TGPREP_LISTEN` | socket address to listen on, e.g. `0.0.0.0:443` |
| `TGPREP_CLIENT_DIR` | the built `dist` directory, read once at start (restart after a rebuild) |
| `TGPREP_TLS_CERT`, `TGPREP_TLS_KEY` | PEM certificate chain and private key; with both set the server speaks HTTPS, without them plain HTTP |

The release binary is copied to `~/.cache/tgprep-release/tgprep-server` (built at the commit that added this line, together with `dist`). Run it with `dist` next to it:

```
TGPREP_LISTEN=0.0.0.0:443 TGPREP_CLIENT_DIR=/path/to/dist TGPREP_TLS_CERT=cert.pem TGPREP_TLS_KEY=key.pem ~/.cache/tgprep-release/tgprep-server
```

Profiling the wasm with function names: build with `CARGO_PROFILE_WEB_STRIP=none crates/web/build.sh`, then replace `dist/assets/tgprep_bg-*.wasm` with `wasm-bindgen --target web --no-typescript --keep-debug` output of `$CARGO_TARGET_DIR/wasm32-unknown-unknown/web/tgprep-web.wasm` (no `wasm-opt`); the JS glue is identical.

## Tests

- `cargo test --workspace`: model ops (the TS unit tests plus more), `.tgprep.json` compatibility (`crates/model/tests/fixtures`: a v1 input, its zod-parsed form and a file the TS app wrote after an edit session, all rewritten byte for byte), geometry against 216 cases evaluated by the TS code in Chromium, server routes.
- e2e: build `dist` first, then `node node_modules/@playwright/test/cli.js test` starts the server with `cargo run --release -p tgprep-server` on 127.0.0.1:5179 and runs `tests/e2e`; `TGPREP_URL=…` points it at a running server instead.
- Perf: `tests/parity/perf.spec.ts`, see `docs/parity.md`.
- Screens: `TGPREP_URL=… TGPREP_LABEL=rust … test -c tests/parity/playwright.config.ts screens`, then `… compare` writes side-by-side sheets (TS, Rust, diff) and `report.tsv` to `~/.cache/tgprep-parity/screens/compare`; `TGPREP_CROP=name,x,y,w,h … crop` zooms a region. Kept sheets are in `docs/parity/`.

## Deliberate differences

- A commit that changes nothing adds no undo step (immer made a new doc for every `detach`).
- An edited thumbnail that is still rendering shows the photo's previous thumbnail (TS showed the broken-image icon on the first edit). A running pulse there also kept Chromium's compositor on vsync and delayed every slider frame.
- A worker joins the pool only after it posts `ready`; thumbnail responses from a folder opened before are dropped.
- The drop target is resolved on every pointer move as well as every frame (the TS app resolved it only in the frame callback).
- Thumbnails: visible tiles first (newest request first, as before), on all four workers. Then read-ahead, nearest first: 8 screens below and 2 above the library window, on at most two workers, not while the viewer is open, and only after a second without input. Read-ahead decodes share the renderer's image decode threads with what is on screen. TS read ahead only by accident (about 144 tiles mounted in its first render).
- When the post header does not fit (820 px panes), its actions wrap to a row of their own; TS pushed Delete post outside the post and scrolled the pane sideways.
- ←/→ in the viewer do not also step a focused slider; in TS the next photo got a 0.1° straighten after touching a slider.
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

Done: model, server, worker, full UI, TS app removed. Measured against TS (see `docs/parity.md`) and stress-tested with 300 photos into 30 posts at 1366×768, 1600×1000 and 820×1180 (`tests/parity/stress.spec.ts`). At 5ebb927, fmt, host and wasm clippy, `cargo test --workspace` and the e2e suite (15 tests) pass.

Open:
1. Send-mode copy is slower than TS when read-ahead resumes during the pause before a click (see `docs/parity.md`).
2. The edit slider frame is about 2 ms slower than TS.
3. A first visit over a slow link is about 120 ms slower, because of the wasm size.
