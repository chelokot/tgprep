# Rust port

The app is Rust: a Leptos (CSR) UI compiled to WebAssembly, a second small wasm bundle as the image Web Worker, and an axum server for the built files. The TypeScript app (`c954212`) was the specification; it still lives in `src/` until the parity gate passes.

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
- e2e: `TGPREP_URL=http://localhost:5180 node node_modules/@playwright/test/cli.js test` runs `tests/e2e` against the Rust server; without `TGPREP_URL` it starts vite (the TS app).
- Screens: `TGPREP_URL=… TGPREP_LABEL=rust … test -c tests/parity/playwright.config.ts screens`, then `… compare` writes side-by-side sheets (TS, Rust, diff) and `report.tsv` to `~/.cache/tgprep-parity/screens/compare`; `TGPREP_CROP=name,x,y,w,h … crop` zooms a region. Kept sheets are in `docs/parity/`.

## Deliberate differences

- A commit that changes nothing adds no undo step (immer made a new doc for every `detach`).
- An edited thumbnail that is still rendering shows the loading pulse instead of the broken-image icon.
- A worker joins the pool only after it posts `ready`; thumbnail responses from a folder opened before are dropped.
- The drop target is resolved on every pointer move as well as every frame (the TS app resolved it only in the frame callback).
- Geometry uses exact doubles; Chromium's DOMMatrix has about 1e-8 of float noise (fixture tolerance 1e-6).

## Gotchas

- `view!` attribute values with `<`, `>`, `>=` or a bare `if` must be braced (`disabled=move || { a >= b }`, `class={if … }`), or the macro silently splits the tag.
- JSX `{a} text {b}` is several text nodes; mirror that (`"#"{n}`) for pixel parity.
- Closures handed to the browser are dropped in a later task (`browser::retire`), never inside their own call.
- `on_cleanup` needs `Send`; hold non-Send resources (listeners, observers) in `StoredValue::new_local` so they drop with the owner.
- wasm builds need `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS=-Cdebuginfo=0` (build.sh sets it).

## Status (2026-11-15)

Done: model, server, worker, full UI; e2e 6/6 on both builds; screens identical on 8 of 10 states, drag-1600 0.04% (ghost subpixels), send-820 0.28% (extent of a stray text selection both builds leave after the drag step).

Next:
1. Measurement harness `tests/parity/perf.spec.ts` and `docs/parity.md` (300 synthetic 6000x4000 JPEGs in OPFS; open → visible thumbs, scroll fill, drag move → highlight frame, viewer open → full frame, slider → frame, send click → clipboard; bundle sizes). Serve both dists through the Rust server (`DIR=~/.cache/tgprep-ts-dist` holds the TS production build).
2. Bundle size: wasm 1,025,013 B raw / 406,226 gzip vs TS JS 429,407 / 131,572 gzip. Look at twiggy (`twiggy top`), serde_json/format! use, view generics.
3. Stress pass (300 photos → 30 posts), fix bugs with failing-then-passing tests. Known: pressing a tile and moving under the 5 px threshold starts a native text selection (TS too).
4. Then delete the TS app (src/, vite/tsconfig/oxlint/vitest configs, React deps), keep tests/e2e running against the Rust build, update README.
