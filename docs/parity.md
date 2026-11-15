# TS vs Rust measurements

Harness: `tests/parity/perf.spec.ts` (`bundle`, `load`, `interactions`). Both builds are served by the Rust server (TS: its production `dist`, kept at `~/.cache/tgprep-ts-dist`), headless Chromium on the Intel Iris Xe through ANGLE/Vulkan (`--enable-gpu --use-angle=vulkan`; the default SwiftShader would time software GL), 1600×1000, CPU 1x, 5 interleaved runs, min / median ms. The folder has 300 synthetic 6000×4000 JPEGs (about 280 KB each) in OPFS, 30 posts of 4 photos, persistent profiles in `~/.cache/tgprep-perf-profiles`. Every timing ends at the end of the frame that first shows the result: a `ResizeObserver` probe resized every frame checks the condition after the frame's `requestAnimationFrame` callbacks and layout, then a `user-blocking` task marks the time. Starts are the triggering event's `timeStamp` (pointer move, pointer up) or the moment the harness changes the page (open, scroll, slider).

```
TGPREP_BUILDS="ts=http://127.0.0.1:5181=$HOME/.cache/tgprep-ts-dist;rust=http://127.0.0.1:5180=$PWD/dist" \
TGPREP_RUNS=5 node node_modules/@playwright/test/cli.js test -c tests/parity/playwright.config.ts perf
```

Results land in `~/.cache/tgprep-parity/perf/*.md`.

## Interactions

Before: 2592a8e, quiet machine, 5 runs. Now: 014dc7b, two harness runs of 5 pooled (10 runs), load average 2.5–5.3 from other work.

| | TS before (quiet) | Rust before (quiet) | TS now | Rust now |
|---|---|---|---|---|
| open folder → visible thumbnails (not cached) | 7004 / 7855 | 1870 / 2280 | 7075 / 9822 | 1546 / 1677 |
| open folder → visible thumbnails (cached) | 210 / 263 | 176 / 244 | 215 / 227 | 161 / 221 |
| scroll one screen → thumbnails | 26 / 31 | 31 / 61 | 24 / 28 | 26 / 30 |
| drag pointer move → drop highlight | 11 / 16 | 11 / 17 | 9 / 16 | 10 / 16 |
| viewer open → full-resolution frame | 340 / 562 | 359 / 628 | 323 / 473 | 321 / 475 |
| edit slider → frame | 11 / 12 | 15 / 27 | 11 / 12 | 13 / 14 |
| send mode click → photo copied | 266 / 317 | 274 / 320 | 269 / 335 | 280 / 363 |

The send row per harness run: TS 269 / 326 and 269 / 351, Rust 280 / 378 and 285 / 350; see Send below.

How the causes were found: Chrome traces (`Tracing` with `devtools.timeline`, `cc`, `gpu`) and CPU profiles of the same harness steps, with the wasm built with symbol names (`CARGO_PROFILE_WEB_STRIP=none`, then `wasm-bindgen --keep-debug` without `wasm-opt`).

- Slider (fixed). A tile whose photo had just been edited showed an `animate-pulse` placeholder until the edited thumbnail rendered. While that animation ran, the compositor stayed subscribed to vsync, so each input waited for the next vsync before its frame (`BeginMainFrame` 7–13 ms after the input instead of 1–3 ms). The tile now keeps the base thumbnail until the edited one is ready. The 300–565 ms samples came from read-ahead (below). About 2 ms is left. The input task is 2.3 ms against React's 1.3 ms. `Layerize` measured 8–14 ms in Rust and 9–10 ms in TS across separate traced runs, so the rest is not attributed. The DOM is the same: 2341 against 2370 nodes, with the same layer and backdrop-filter counts.
- Read-ahead contention (scroll, slider spikes, viewer). The thumbnail workers' `createImageBitmap` decodes of 6000×4000 JPEGs (50–110 ms each) run on the renderer's shared `ThreadPoolForegroundWorker` threads (7 here), the same threads that decode visible thumbnails and rasterize tiles. TS had no background work by then. Read-ahead now uses at most two workers, waits while the viewer is open and runs only after a second without input (pointer press or drag, wheel, key, scroll). With that, the scroll row moved from 31 / 61 to 27 / 31. Under that load, one 86 ms `createObjectURL` call for an arriving read-ahead thumbnail also blocked a scroll frame.
- Viewer open. With read-ahead out of the way, the same work remains in both builds. `texImage2D` of the 4096 px bitmap takes 64–105 ms in Rust and 84–214 ms in TS, and both show multi-second GPU-contention outliers (TS 2.7 s, Rust 1.6 s in one profiled run). Minimums are equal; the medians differ inside the noise of a loaded machine.
- Send: not read-ahead; the rest is not localised. Read-ahead does resume in the 1.8 s toast pause, so later clicks can start with two thumbnail decodes in flight (the click's press stops further dispatch). A copy of the harness held read-ahead off from entering send mode with synthetic input on alternate Rust runs and clicked 10 posts per run, 6 runs (load about 2.5–3): TS 313 ms median with 4 of 60 clicks over 500 ms, Rust 325 with 2 of 30, Rust without read-ahead 320 with 3 of 30. A probe timing `createImageBitmap`, `convertToBlob` and `clipboard.write` at each click found the same work in both builds after the click, at the same cost per stage: decode 60–80 ms, resize to 2560 px 60–70, WebGL render 12–50, PNG encode 75–95, clipboard 45–55, toast frame 15. The plain harness runs above had more slow Rust clicks (8 of 30 against TS 3 of 30); the 10-click runs did not reproduce that.
- Rejected: decoding thumbnails with WebCodecs `ImageDecoder` at a DCT-scaled size. It decodes a smooth 280 KB synthetic JPEG 7× faster (10 ms against 72 ms) and applies EXIF orientation. It snaps desired sizes to a DCT scale (800×600 and 1125×750 gave 750×500) and fails below 1/8. On a noisy 11.7 MB JPEG, closer to a camera file, four workers made 14–19 thumbnails a second against 14.8 with `createImageBitmap`, because Huffman decoding dominates.

## Load

The welcome screen painted, measured from navigation start. First visit is a fresh context; second and third reload it (immutable assets from the HTTP cache, wasm compiled from Chrome's code cache). 014dc7b, 5 runs:

| | TS | Rust |
|---|---|---|
| first visit | 225 / 256 | 196 / 209 |
| second visit | 26 / 30 | 24 / 30 |
| third visit | 21 / 27 | 24 / 30 |
| first visit, 9 Mbit/s 60 ms | 287 / 292 | 425 / 429 |
| second visit, 9 Mbit/s 60 ms | 85 / 90 | 84 / 88 |
| third visit, 9 Mbit/s 60 ms | 83 / 89 | 83 / 95 |

Locally Rust is faster, because the wasm compiles while it streams. Over a 9 Mbit/s link a first visit is about 140 ms slower. That is the 198 KB of extra brotli bytes (about 175 ms at 9 Mbit/s), partly hidden by the TS bundle's longer parse.

## Bundle (bytes as the server sends them, 014dc7b)

| | TS | Rust |
|---|---:|---:|
| wasm br | 0 | 300,419 |
| js br | 114,458 | 13,740 |
| total br | 123,438 | 321,299 |
| total gzip | 142,905 | 412,761 |
| total identity | 478,477 | 1,112,923 |

`build.sh` now builds with `--cfg erase_components`, which makes the wasm about 10% smaller. Its effect on the interaction rows was not isolated. It also passes `--remap-path-prefix` for the cargo registry. Before the remap, the bundle carried absolute `/var/home/…/.cargo/registry` paths in panic locations.

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

Scrollbars are shown (`--hide-scrollbars` off). The first three posts are built by dragging from the library: single photos into the empty pane and onto the last tile's edge, a ten-photo selection into the gap below a post, a photo between two posts, and a selection before a post's first tile. The checks also run while a drag is held over a post and over a gap. The other 27 posts come from New post from selection, and one reorder drags inside a post. The library drags found nothing at any size. Bugs found, all also in TS, all fixed with failing-then-passing e2e tests:

- At 820 px the post header was wider than the post. Delete post sat outside the card and the posts pane scrolled sideways. The actions now wrap as a group onto their own row.
- After touching a slider in the edit panel, → moved to the next photo and the still-focused range input stepped too. The next photo got a 0.1° straighten and a crop.
- At 820×1180 in crop mode, where the photo fills the stage's width, the side handles sat under Previous and Next. The arrows now sit at the edges of the photo area, also with the edit panel open (TS put Next 1.75 rem from the panel), and the crop stage keeps 4.5 rem side margins.

```
TGPREP_URL=http://127.0.0.1:5179 node node_modules/@playwright/test/cli.js test -c tests/parity/playwright.config.ts stress
```

Screenshots land in `~/.cache/tgprep-parity/stress/<TGPREP_LABEL or rust>/`.

## Screens

`docs/parity/*.png` are TS | Rust | diff sheets, and `screens.tsv` the differing pixels. The 820 sheets differ by about 23% because the post actions wrap. The 1600 sheets differ by about 1% at glyph edges inside thumbnails, which are resampled from 512 px in Rust and 720 px in TS.
