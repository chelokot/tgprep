# TS vs Rust measurements

Harness: `tests/parity/perf.spec.ts` (`bundle`, `load`, `interactions`). Both builds are served by the Rust server (TS: its production `dist`, kept at `~/.cache/tgprep-ts-dist`), headless Chromium on the Intel Iris Xe through ANGLE/Vulkan (`--enable-gpu --use-angle=vulkan`; the default SwiftShader would time software GL), 1600×1000, CPU 1x, 5 interleaved runs, min / median ms. The folder has 300 synthetic 6000×4000 JPEGs (about 280 KB each) in OPFS, 30 posts of 4 photos, persistent profiles in `~/.cache/tgprep-perf-profiles`. Every timing ends at the end of the frame that first shows the result: a `ResizeObserver` probe resized every frame checks the condition after the frame's `requestAnimationFrame` callbacks and layout, then a `user-blocking` task marks the time. Starts are the triggering event's `timeStamp` (pointer move, pointer up) or the moment the harness changes the page (open, scroll, slider).

```
TGPREP_BUILDS="ts=http://127.0.0.1:5181=$HOME/.cache/tgprep-ts-dist;rust=http://127.0.0.1:5180=$PWD/dist" \
TGPREP_RUNS=5 node node_modules/@playwright/test/cli.js test -c tests/parity/playwright.config.ts perf
```

Results land in `~/.cache/tgprep-parity/perf/*.md`.

## Interactions

Before: 2592a8e, quiet machine, 5 runs. After: 5ebb927, 4 runs; other work kept the load average at 4–7, which is why TS's own numbers moved (its uncached open swung from 7 s to 27 s).

| | TS before (quiet) | Rust before (quiet) | TS after (loaded) | Rust after (loaded) |
|---|---|---|---|---|
| open folder → visible thumbnails (not cached) | 7004 / 7855 | 1870 / 2280 | 8419 / 27322 | 1638 / 2069 |
| open folder → visible thumbnails (cached) | 210 / 263 | 176 / 244 | 190 / 255 | 158 / 226 |
| scroll one screen → thumbnails | 26 / 31 | 31 / 61 | 25 / 33 | 27 / 31 |
| drag pointer move → drop highlight | 11 / 16 | 11 / 17 | 12 / 17 | 11 / 16 |
| viewer open → full-resolution frame | 340 / 562 | 359 / 628 | 372 / 462 | 373 / 578 |
| edit slider → frame | 11 / 12 | 15 / 27 | 11 / 12 | 13 / 14 |
| send mode click → photo copied | 266 / 317 | 274 / 320 | 267 / 324 | 339 / 441 |

How the causes were found: Chrome traces (`Tracing` with `devtools.timeline`, `cc`, `gpu`) and CPU profiles of the same harness steps, with the wasm built with symbol names (`CARGO_PROFILE_WEB_STRIP=none`, then `wasm-bindgen --keep-debug` without `wasm-opt`).

- Slider (fixed). A tile whose photo had just been edited showed an `animate-pulse` placeholder until the edited thumbnail rendered. While that animation ran, the compositor stayed subscribed to vsync, so each input waited for the next vsync before its frame (`BeginMainFrame` 7–13 ms after the input instead of 1–3 ms). The tile now keeps the base thumbnail until the edited one is ready. The 300–565 ms samples came from read-ahead (below). About 2 ms is left. The input task is 2.3 ms against React's 1.3 ms. `Layerize` measured 8–14 ms in Rust and 9–10 ms in TS across separate traced runs, so the rest is not attributed. The DOM is the same: 2341 against 2370 nodes, with the same layer and backdrop-filter counts.
- Read-ahead contention (scroll, slider spikes, viewer). The thumbnail workers' `createImageBitmap` decodes of 6000×4000 JPEGs (50–110 ms each) run on the renderer's shared `ThreadPoolForegroundWorker` threads (7 here), the same threads that decode visible thumbnails and rasterize tiles. TS had no background work by then. Read-ahead now uses at most two workers, waits while the viewer is open and runs only after a second without input (pointer press or drag, wheel, key, scroll). With that, the scroll row moved from 31 / 61 to 27 / 31. Under that load, one 86 ms `createObjectURL` call for an arriving read-ahead thumbnail also blocked a scroll frame.
- Viewer open. With read-ahead out of the way, the same work remains in both builds. `texImage2D` of the 4096 px bitmap takes 64–105 ms in Rust and 84–214 ms in TS, and both show multi-second GPU-contention outliers (TS 2.7 s, Rust 1.6 s in one profiled run). Minimums are equal; the medians differ inside the noise of a loaded machine.
- Send: a regression introduced by these scheduling changes, cause not verified. Before them the row matched TS (274 / 320 against 266 / 317). Since the viewer pause and the idle gate it has been 330–374 / 441–671. The suspected cause: the harness waits about 1.8 s for the toast between copies, long enough for read-ahead to resume, so up to two 6000×4000 decodes are in flight when the next click decodes its own full-resolution photo. That is unproven. Read-ahead was also unfinished under the old scheduling (243 of 300 thumbnails at the end of the slider steps), and `erase_components` changed in the same span without being isolated on interactions. Next step: log the cached thumbnail count before each copy click and add a variant that waits for all 300 before send mode.
- Rejected: decoding thumbnails with WebCodecs `ImageDecoder` at a DCT-scaled size. It decodes a smooth 280 KB synthetic JPEG 7× faster (10 ms against 72 ms) and applies EXIF orientation. It snaps desired sizes to a DCT scale (800×600 and 1125×750 gave 750×500) and fails below 1/8. On a noisy 11.7 MB JPEG, closer to a camera file, four workers made 14–19 thumbnails a second against 14.8 with `createImageBitmap`, because Huffman decoding dominates.

## Load

The welcome screen painted, measured from navigation start. First visit is a fresh context; second and third reload it (immutable assets from the HTTP cache, wasm compiled from Chrome's code cache). 5ebb927, 5 runs:

| | TS | Rust |
|---|---|---|
| first visit | 229 / 265 | 199 / 214 |
| second visit | 21 / 51 | 19 / 37 |
| third visit | 21 / 29 | 26 / 62 |
| first visit, 9 Mbit/s 60 ms | 320 / 348 | 435 / 468 |
| second visit, 9 Mbit/s 60 ms | 84 / 378 | 93 / 96 |
| third visit, 9 Mbit/s 60 ms | 81 / 87 | 85 / 97 |

Locally Rust is faster, because the wasm compiles while it streams. Over a 9 Mbit/s link a first visit is about 120 ms slower. That is the 198 KB of extra brotli bytes (about 175 ms at 9 Mbit/s), partly hidden by the TS bundle's longer parse.

## Bundle (bytes as the server sends them, 5ebb927)

| | TS | Rust |
|---|---:|---:|
| wasm br | 0 | 300,768 |
| js br | 114,458 | 13,740 |
| total br | 123,438 | 321,643 |
| total gzip | 142,905 | 412,902 |
| total identity | 478,477 | 1,113,144 |

`build.sh` now builds with `--cfg erase_components`, which makes the wasm about 10% smaller. Its effect on the interaction rows was not isolated; see the send row and `--remap-path-prefix` for the cargo registry. Before the remap, the bundle carried absolute `/var/home/…/.cargo/registry` paths in panic locations.

Main wasm (`tgprep_bg`) variants, raw / gzip 9 / brotli 11 (measured at 2592a8e):

| build | raw | gzip | brotli |
|---|---:|---:|---:|
| `opt-level = "z"`, fat LTO, 1 CGU, `panic = "abort"`, `wasm-opt -Oz` | 1,032,434 | 409,303 | 316,354 |
| same with `opt-level = "s"` | 1,223,167 | 456,501 | 337,873 |
| `z` + `--cfg erase_components` (kept) | 938,186 | 369,512 | 285,915 |
| `z` + `erase_components` + nightly `-Zbuild-std` `-Cpanic=immediate-abort` | 831,754 | 328,980 | 258,139 |
| `wasm-opt` flag variants (`--converge`, `-Oz -Oz`, `--gufa`, `-Os`) | ±0.5% | | |

Where the optimized code goes (twiggy on `wasm-opt -Oz -g`, 1.02 MB of items): tgprep_web 22%, tachys 16%, core 15% (mostly `drop_in_place` and closure shims generated for Leptos view types), reactive_graph 13%, data 12% (Tailwind classes, SVG icons, both locales), serde_json 4% (needed for byte-exact `.tgprep.json`), alloc 4%, leptos 3%, hashbrown 2%. Leptos has no optional features left to switch off for CSR.

Why the slow-link gap stays open:

- Trimming dependencies buys single-digit percent.
- Matching TS needs about 180 KB less brotli. That is roughly the whole Leptos runtime plus its view monomorphizations, so it means dropping the framework.
- Splitting the bundle so the welcome screen paints before the rest loads needs Leptos `#[lazy]` with `wasm-split` post-processing (`cargo-leptos --split`). That pipeline is experimental. The first chunk would still carry the shared runtime (tachys, reactive_graph, core), so the estimated gain is 60–100 KB brotli, about 50–90 ms on this link.
- Nightly `build-std` buys another 10% but pins a nightly toolchain.

## Stress pass

`tests/parity/stress.spec.ts` prepares 300 photos into 30 posts of 10 at 1366×768, 1600×1000 and 820×1180. Each run selects with Ctrl and Shift click, makes a new post from the selection, writes captions, reorders, edits in the viewer (rotate, exposure, straighten, crop), archives, then uses send mode and marks a post sent. After each state it saves a screenshot and fails on any of these:

- page scroll;
- a control outside the window that no scroller can reach;
- a control clipped by its container;
- cut text that is not an ellipsis;
- a scroller that scrolls sideways.

Scrollbars are shown (`--hide-scrollbars` off). Posts are built with New post from selection. The only drag is one reorder inside a post; drags from the library and post moves are covered by the e2e tests at 1600 and 820, not at 1366. It found two bugs, both also in TS, both fixed with failing-then-passing e2e tests:

- At 820 px the post header was wider than the post. Delete post sat outside the card and the posts pane scrolled sideways. The actions now wrap as a group onto their own row.
- After touching a slider in the edit panel, → moved to the next photo and the still-focused range input stepped too. The next photo got a 0.1° straighten and a crop.

```
TGPREP_URL=http://127.0.0.1:5179 node node_modules/@playwright/test/cli.js test -c tests/parity/playwright.config.ts stress
```

Screenshots land in `~/.cache/tgprep-parity/stress/<TGPREP_LABEL or rust>/`.

## Screens

`docs/parity/*.png` are TS | Rust | diff sheets, and `screens.tsv` the differing pixels. The 820 sheets differ by about 23% because the post actions wrap. The 1600 sheets differ by about 1% at glyph edges inside thumbnails, which are resampled from 512 px in Rust and 720 px in TS.
