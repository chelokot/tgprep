# TS vs Rust measurements

Harness: `tests/parity/perf.spec.ts` (`bundle`, `load`, `interactions`). Both builds are served by the Rust server (TS: its production `dist`, kept at `~/.cache/tgprep-ts-dist`), headless Chromium on the Intel Iris Xe through ANGLE/Vulkan (`--enable-gpu --use-angle=vulkan`; the default SwiftShader would time software GL), 1600×1000, CPU 1x, 5 interleaved runs, min / median ms. The folder has 300 synthetic 6000×4000 JPEGs (about 280 KB each) in OPFS, 30 posts of 4 photos, persistent profiles in `~/.cache/tgprep-perf-profiles`. Every timing ends at the end of the frame that first shows the result: a `ResizeObserver` probe resized every frame checks the condition after the frame's `requestAnimationFrame` callbacks and layout, then a `user-blocking` task marks the time. Starts are the triggering event's `timeStamp` (pointer move, pointer up) or the moment the harness changes the page (open, scroll, slider).

```
TGPREP_BUILDS="ts=http://127.0.0.1:5181=$HOME/.cache/tgprep-ts-dist;rust=http://127.0.0.1:5180=$PWD/dist" \
TGPREP_RUNS=5 node node_modules/@playwright/test/cli.js test -c tests/parity/playwright.config.ts perf
```

Results land in `~/.cache/tgprep-parity/perf/*.md`.

## Interactions (2592a8e)

| | TS | Rust |
|---|---|---|
| open folder → visible thumbnails (not cached) | 7004 / 7855 | 1870 / 2280 |
| open folder → visible thumbnails (cached) | 210 / 263 | 176 / 244 |
| scroll one screen → thumbnails | 26 / 31 | 31 / 61 |
| drag pointer move → drop highlight | 11 / 16 | 11 / 17 |
| viewer open → full-resolution frame | 340 / 562 | 359 / 628 |
| edit slider → frame | 11 / 12 | 15 / 27 |
| send mode click → photo copied | 266 / 317 | 274 / 320 |

- Open: TS mounts about 144 tiles in its first render (the virtualizer has no scroller size yet), queues all their thumbnails and pops them last-in first-out, so the visible ones finish last. Rust asks only for the tiles on screen, then reads ahead (8 screens below, 2 above, nearest first) with idle workers.
- Scroll: TS's 144 accidental thumbnails cover the five scrolled screens; Rust has to have read them ahead (the harness waits 8 s after opening). Scrolling past what is prepared costs both builds the decode time (about 20 thumbnails a second on 4 workers).
- Edit slider: the open question. Outside the harness (same profile, same sequence: scrolls, five viewer opens, then 12 slider steps) Rust finishes the input task in 0.5 ms and the next frame 5–17 ms later, like TS; inside the harness Rust's median is a frame longer with occasional 300–565 ms samples. Not explained yet.

## Load

The welcome screen painted, measured from navigation start. First visit is a fresh context; second and third reload it (immutable assets from the HTTP cache, wasm compiled from Chrome's code cache).

| | TS | Rust | Rust `erase_components` |
|---|---|---|---|
| first visit | 222 / 239 | 190 / 200 | 193 / 205 |
| second visit | 21 / 32 | 25 / 30 | 25 / 30 |
| third visit | 28 / 30 | 29 / 31 | 20 / 30 |
| first visit, 9 Mbit/s 60 ms | 291 / 299 | 452 / 458 | 426 / 432 |
| second visit, 9 Mbit/s 60 ms | 82 / 88 | 85 / 91 | 83 / 84 |
| third visit, 9 Mbit/s 60 ms | 77 / 84 | 85 / 93 | 94 / 96 |

Locally and on every cached visit Rust is as fast or faster (streaming compilation, immutable caching). A first visit over a 9 Mbit/s link is about 160 ms slower: the extra 200–230 KB of compressed wasm.

## Bundle (bytes as the server sends them)

| | TS | Rust | Rust `erase_components` |
|---|---:|---:|---:|
| wasm br | 0 | 329,474 | 299,195 |
| js br | 114,458 | 13,716 | 13,716 |
| total br | 123,438 | 350,317 | 320,039 |
| total gzip | 142,905 | 449,985 | 410,012 |
| total identity | 478,477 | 1,201,325 | 1,107,084 |

Main wasm (`tgprep_bg`) variants, raw / gzip 9 / brotli 11:

| build | raw | gzip | brotli |
|---|---:|---:|---:|
| `opt-level = "z"`, fat LTO, 1 CGU, `panic = "abort"`, `wasm-opt -Oz` (kept) | 1,032,434 | 409,303 | 316,354 |
| same with `opt-level = "s"` | 1,223,167 | 456,501 | 337,873 |
| `z` + `--cfg erase_components` | 938,186 | 369,512 | 285,915 |
| `z` + `erase_components` + nightly `-Zbuild-std` `-Cpanic=immediate-abort` | 831,754 | 328,980 | 258,139 |
| `wasm-opt` flag variants (`--converge`, `-Oz -Oz`, `--gufa`, `-Os`) | ±0.5% | | |

twiggy: of the code, about 420 KB is Leptos/tachys/reactive_graph monomorphizations (view builders, `drop_in_place` for 881 types, closures), 140 KB the app, 37 KB serde_json (needed for byte-exact `.tgprep.json`), about 15 KB float formatting. Trimming dependencies would buy single-digit percent; `erase_components` buys 10% with no measurable cost here; the nightly build-std another 10% but pins a nightly toolchain.
