use crate::browser::{Listener, error_message, settle, wait, window};
use crate::export::copy_photo;
use crate::i18n::{fill, t};
use crate::icons::{self, svg};
use crate::renderer::{Canvas, Renderer};
use crate::store::{PhotoFile, Viewer, close_viewer, commit, commit_keyed, store, toast};
use crate::thumbs::{Thumb, base_thumb, display_thumb, spawn_worker};
use crate::ui::{IconButton, RangeInput, Tone};
use crate::virtual_list::ResizeWatch;
use js_sys::{Function, Promise};
use leptos::html::{Canvas as CanvasElement, Div};
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use tgprep_model::doc::{Adjustment, Aspect, Crop, Edits, PhotoId};
use tgprep_model::geometry::{Handle, Side, Size, aspect_ratio, fit_crop, frame_size, move_crop, output_size, resize_crop};
use tgprep_model::ops;
use tgprep_worker::{Job, Request, Response};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{File, ImageBitmap, KeyboardEvent, PointerEvent, Worker};

const MAX_VIEW_SIDE: f64 = 4096.0;
const BITMAP_CACHE_LIMIT: usize = 5;

#[derive(Default)]
struct Decoder {
    worker: Option<Worker>,
    ready: bool,
    waiting: Vec<JsValue>,
    decoded: HashMap<String, Function>,
    bitmaps: VecDeque<(String, Promise)>,
}

thread_local! {
    static DECODER: RefCell<Decoder> = RefCell::new(Decoder::default());
}

fn decoded(_: &Worker, response: Response) {
    match response {
        Response::Ready => {
            let waiting = DECODER.with_borrow_mut(|decoder| {
                decoder.ready = true;
                std::mem::take(&mut decoder.waiting).into_iter().map(|message| (decoder.worker.clone(), message)).collect::<Vec<_>>()
            });
            for (worker, message) in waiting {
                if let Some(worker) = worker {
                    let _ = worker.post_message(&message);
                }
            }
        }
        Response::View { key, bitmap } => {
            if let Some(resolve) = DECODER.with_borrow_mut(|decoder| decoder.decoded.remove(&key)) {
                let _ = resolve.call1(&JsValue::NULL, &bitmap);
            }
        }
        Response::Thumb { .. } => {}
    }
}

pub fn start_decoder() {
    let worker = spawn_worker(decoded);
    DECODER.with_borrow_mut(|decoder| decoder.worker = worker);
}

fn load_bitmap(file: &Rc<PhotoFile>) -> Promise {
    let key = format!("{}|{}", file.id, file.last_modified);
    if let Some(cached) = DECODER.with_borrow(|decoder| decoder.bitmaps.iter().find(|(candidate, _)| *candidate == key).map(|(_, promise)| promise.clone())) {
        return cached;
    }
    let requested = key.clone();
    let loading = Promise::new(&mut |resolve, _| {
        DECODER.with_borrow_mut(|decoder| decoder.decoded.insert(requested.clone(), resolve));
    });
    let handle = file.handle.clone();
    spawn_local(async move {
        let Ok(data) = wait::<File>(handle.get_file()).await else { return };
        let message = Request { job: Job::View, key, file: data, long_side: MAX_VIEW_SIDE }.to_js();
        let worker = DECODER.with_borrow_mut(|decoder| {
            if decoder.ready {
                decoder.worker.clone()
            } else {
                decoder.waiting.push(message.clone());
                None
            }
        });
        if let Some(worker) = worker {
            let _ = worker.post_message(&message);
        }
    });
    DECODER.with_borrow_mut(|decoder| {
        decoder.bitmaps.push_back((requested, loading.clone()));
        while decoder.bitmaps.len() > BITMAP_CACHE_LIMIT {
            decoder.bitmaps.pop_front();
        }
    });
    loading
}

fn set_edits(id: &PhotoId, key: &str, update: impl FnOnce(Edits) -> Edits) {
    let id = id.clone();
    commit_keyed(Some(format!("edit:{id}:{key}")), move |doc| {
        let edits = update(doc.meta(&id).edits);
        ops::update_meta(doc, &[id], |meta| meta.edits = edits);
    });
}

fn fit_into(container: Size, aspect: f64) -> Size {
    if container.w / container.h > aspect { Size { w: container.h * aspect, h: container.h } } else { Size { w: container.w, h: container.w / aspect } }
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 { format!("{}", value.trunc()) } else { value.to_string() }
}

fn photo_file(id: &PhotoId) -> Option<Rc<PhotoFile>> {
    store().library.with_untracked(|library| library.as_ref().and_then(|library| library.by_id.get(id).cloned()))
}

fn edits_of(id: PhotoId) -> Memo<Edits> {
    Memo::new(move |_| store().doc.with(|doc| doc.meta(&id).edits))
}

#[component]
fn CropOverlay(id: PhotoId, size: Size) -> impl IntoView {
    let edits = edits_of(id.clone());
    let overlay = NodeRef::<Div>::new();
    let dragging = StoredValue::new_local(Vec::<Listener>::new());
    let start = move |event: PointerEvent, handle: Option<Handle>| {
        event.stop_propagation();
        if let Some(target) = event.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) {
            let _ = target.set_pointer_capture(event.pointer_id());
        }
        let Some(overlay) = overlay.get_untracked() else { return };
        let rect = overlay.get_bounding_client_rect();
        let origin = (f64::from(event.client_x()), f64::from(event.client_y()));
        let initial = edits.get_untracked();
        let ratio = aspect_ratio(initial.aspect, frame_size(size, initial.quarter));
        let id = id.clone();
        let target: web_sys::EventTarget = window().into();
        let moving = Listener::new(&target, "pointermove", false, move |event| {
            let Some(event) = event.dyn_ref::<PointerEvent>() else { return };
            let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
            let next = match handle {
                Some(handle) => resize_crop(size, &initial, initial.crop, handle, (x - rect.left()) / rect.width(), (y - rect.top()) / rect.height(), ratio),
                None => move_crop(size, &initial, initial.crop, (x - origin.0) / rect.width(), (y - origin.1) / rect.height()),
            };
            set_edits(&id, "crop", |current| Edits { crop: next, ..current });
        });
        let ending = Listener::new(&target, "pointerup", false, move |_| dragging.update_value(Vec::clear));
        dragging.set_value(vec![moving, ending]);
    };
    let box_style = move || {
        let crop = edits.get().crop;
        format!("left: {}%; top: {}%; width: {}%; height: {}%;", crop.x * 100.0, crop.y * 100.0, crop.w * 100.0, crop.h * 100.0)
    };
    let mover = start.clone();
    view! {
        <div node_ref=overlay class="absolute inset-0 overflow-hidden">
            <div on:pointerdown=move |event| mover(event, None) class="absolute cursor-move shadow-[0_0_0_200vmax_rgba(0,0,0,0.6)] ring-1 ring-white" style=box_style>
                <div class="pointer-events-none absolute inset-0 grid grid-cols-3 grid-rows-3">
                    {(0..9).map(|_| view! { <div class="border-[0.5px] border-white/30"></div> }).collect_view()}
                </div>
                {Handle::ALL
                    .into_iter()
                    .map(|handle| {
                        let start = start.clone();
                        let position = |side| match side {
                            Side::Start => 0,
                            Side::Middle => 50,
                            Side::End => 100,
                        };
                        let cursor = match (handle.x, handle.y) {
                            (Side::Middle, _) => "ns-resize",
                            (_, Side::Middle) => "ew-resize",
                            (x, y) if x == y => "nwse-resize",
                            _ => "nesw-resize",
                        };
                        view! {
                            <div
                                on:pointerdown=move |event| start(event, Some(handle))
                                class="absolute size-4 -translate-1/2 rounded-sm border-2 border-white bg-black/40"
                                style=format!("left: {}%; top: {}%; cursor: {cursor};", position(handle.x), position(handle.y))
                            ></div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
fn Stage(id: PhotoId, file: Rc<PhotoFile>, size: Size, cropping: Memo<bool>, comparing: RwSignal<bool>) -> impl IntoView {
    let edits = edits_of(id.clone());
    let container = NodeRef::<Div>::new();
    let canvas = NodeRef::<CanvasElement>::new();
    let bounds = RwSignal::new(Size { w: 0.0, h: 0.0 });
    let bitmap: RwSignal<Option<ImageBitmap>, LocalStorage> = RwSignal::new_local(None);
    let renderer = StoredValue::new_local(None::<Renderer>);
    let thumb = display_thumb(id.clone());
    let watch = StoredValue::new_local(None::<ResizeWatch>);
    Effect::new(move |_| {
        if let Some(element) = container.get() {
            watch.set_value(ResizeWatch::new(&element, move |entry| {
                let rect = entry.content_rect();
                bounds.set(Size { w: rect.width(), h: rect.height() });
            }));
        }
    });
    let loading = load_bitmap(&file);
    spawn_local(async move {
        if let Ok(loaded) = wait::<ImageBitmap>(loading).await {
            let _ = bitmap.try_set(Some(loaded));
        }
    });
    let shown = Memo::new(move |_| if comparing.get() { Edits::default() } else { edits.get() });
    let display = Memo::new(move |_| {
        let shown = shown.get();
        let output = if cropping.get() { frame_size(size, shown.quarter) } else { output_size(size, &shown) };
        let container = bounds.get();
        if container.w > 0.0 { fit_into(container, output.w / output.h) } else { Size { w: 0.0, h: 0.0 } }
    });
    Effect::new(move |_| {
        let (shown, cropping, display) = (shown.get(), cropping.get(), display.get());
        bitmap.with(|bitmap| {
            let Some(bitmap) = bitmap else { return };
            if display.w == 0.0 {
                return;
            }
            let Some(element) = canvas.get() else { return };
            let ratio = window().device_pixel_ratio();
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "canvas pixel sizes")]
            let output = ((display.w * ratio).round() as u32, (display.h * ratio).round() as u32);
            renderer.update_value(|renderer| {
                if renderer.is_none() {
                    *renderer = Renderer::new(Canvas::Element(element)).ok();
                }
                if let Some(renderer) = renderer.as_mut() {
                    let _ = renderer.render(bitmap, &shown, if cropping { Crop::FULL } else { shown.crop }, output);
                }
            });
        });
    });
    let loaded = move || bitmap.with(Option::is_some);
    view! {
        <div node_ref=container class=move || if cropping.get() { "relative min-h-0 min-w-0 flex-1 m-10" } else { "relative min-h-0 min-w-0 flex-1 m-4" }>
            <div class="absolute top-1/2 left-1/2 -translate-1/2" style=move || format!("width: {}px; height: {}px;", display.get().w, display.get().h)>
                {move || {
                    (!loaded())
                        .then(|| thumb.get().and_then(|thumb| thumb.url))
                        .flatten()
                        .map(|url| view! { <img src=url alt="" class="absolute inset-0 size-full object-contain opacity-80 blur-[1px]" /> })
                }}
                <canvas node_ref=canvas class="absolute inset-0 size-full" style=move || if loaded() { "visibility: visible;" } else { "visibility: hidden;" }></canvas>
                {move || cropping.get().then(|| view! { <CropOverlay id=id.clone() size=size /> })}
            </div>
        </div>
    }
}

struct Group {
    title: fn() -> &'static str,
    keys: &'static [(Adjustment, f64)],
}

const GROUPS: [Group; 3] = [
    Group {
        title: || t().light,
        keys: &[(Adjustment::Exposure, -100.0), (Adjustment::Brightness, -100.0), (Adjustment::Contrast, -100.0), (Adjustment::Highlights, -100.0), (Adjustment::Shadows, -100.0)],
    },
    Group { title: || t().color, keys: &[(Adjustment::Temperature, -100.0), (Adjustment::Tint, -100.0), (Adjustment::Saturation, -100.0), (Adjustment::Vibrance, -100.0)] },
    Group { title: || t().effects, keys: &[(Adjustment::Sharpness, 0.0), (Adjustment::Vignette, -100.0), (Adjustment::Fade, 0.0), (Adjustment::Grain, 0.0)] },
];

fn adjustment_label(adjustment: Adjustment) -> &'static str {
    let strings = t();
    match adjustment {
        Adjustment::Exposure => strings.exposure,
        Adjustment::Brightness => strings.brightness,
        Adjustment::Contrast => strings.contrast,
        Adjustment::Highlights => strings.highlights,
        Adjustment::Shadows => strings.shadows,
        Adjustment::Temperature => strings.temperature,
        Adjustment::Tint => strings.tint,
        Adjustment::Saturation => strings.saturation,
        Adjustment::Vibrance => strings.vibrance,
        Adjustment::Sharpness => strings.sharpness,
        Adjustment::Vignette => strings.vignette,
        Adjustment::Fade => strings.fade,
        Adjustment::Grain => strings.grain,
    }
}

fn aspect_label(aspect: Aspect) -> &'static str {
    match aspect {
        Aspect::Free => t().aspect_free,
        Aspect::Original => t().aspect_original,
        Aspect::Square => "1:1",
        Aspect::Portrait4x5 => "4:5",
        Aspect::Landscape5x4 => "5:4",
        Aspect::Portrait3x4 => "3:4",
        Aspect::Landscape4x3 => "4:3",
        Aspect::Portrait2x3 => "2:3",
        Aspect::Landscape3x2 => "3:2",
        Aspect::Portrait9x16 => "9:16",
        Aspect::Landscape16x9 => "16:9",
    }
}

#[component]
fn EditPanel(id: PhotoId, size: Size, cropping: RwSignal<bool>) -> impl IntoView {
    let store = store();
    let edits = edits_of(id.clone());
    let base_crop = StoredValue::new(edits.get_untracked().crop);
    let turn = {
        let id = id.clone();
        move |clockwise: bool| {
            set_edits(&id, "quarter", |current| {
                let quarter = current.quarter.turned(clockwise);
                let next = Edits { quarter, ..current };
                Edits { crop: fit_crop(size, &next, Crop::FULL, aspect_ratio(current.aspect, frame_size(size, quarter))), ..next }
            });
        }
    };
    let turn_left = turn.clone();
    let (flipped, reset_crop, angle_reset, angle_change) = (id.clone(), id.clone(), id.clone(), id.clone());
    let (pasted, reset_all) = (id.clone(), id.clone());
    let aspects = id.clone();
    let adjusted = id;
    view! {
        <aside class="flex w-80 shrink-0 flex-col overflow-y-auto border-l border-white/10 bg-[#121317] text-white">
            <section class="space-y-3 border-b border-white/10 p-4">
                <div class="flex items-center gap-1">
                    <IconButton tone=Tone::Overlay icon=icons::CROP label=t().crop active=cropping on_click=move |()| cropping.update(|cropping| *cropping = !*cropping) />
                    <IconButton tone=Tone::Overlay icon=icons::ROTATE_CCW label=t().rotate_left on_click=move |()| turn_left(false) />
                    <IconButton tone=Tone::Overlay icon=icons::ROTATE_CW label=t().rotate_right on_click=move |()| turn(true) />
                    <IconButton
                        tone=Tone::Overlay
                        icon=icons::FLIP_HORIZONTAL_2
                        label=t().flip
                        active=Signal::derive(move || edits.get().flip)
                        on_click=move |()| set_edits(&flipped, "flip", |current| Edits { flip: !current.flip, ..current })
                    />
                    <div class="flex-1"></div>
                    <IconButton
                        tone=Tone::Overlay
                        icon=icons::UNDO_2
                        label=t().reset_crop
                        on_click=move |()| set_edits(&reset_crop, "reset-crop", |current| Edits { crop: Crop::FULL, angle: 0.0, quarter: Default::default(), flip: false, aspect: Aspect::Free, ..current })
                    />
                </div>
                <label class="block text-xs text-white/60">
                    <span class="flex justify-between">
                        {t().angle}
                        " "
                        <span class="tabular-nums">{move || format!("{:.1}", edits.get().angle)}"°"</span>
                    </span>
                    <RangeInput
                        label=t().angle
                        min=-45.0
                        max=45.0
                        step=0.1
                        value=Signal::derive(move || edits.get().angle)
                        on_start=move |()| base_crop.set_value(edits.get_untracked().crop)
                        on_reset=move |()| {
                            let base = base_crop.get_value();
                            set_edits(&angle_reset, "angle", |current| Edits { angle: 0.0, crop: fit_crop(size, &Edits { angle: 0.0, ..current }, base, None), ..current });
                        }
                        on_change=move |angle| {
                            let base = base_crop.get_value();
                            set_edits(&angle_change, "angle", |current| Edits { angle, crop: fit_crop(size, &Edits { angle, ..current }, base, None), ..current });
                        }
                        class="h-1 cursor-pointer accent-accent mt-1 w-full"
                    />
                </label>
                <div>
                    <div class="mb-1.5 text-xs text-white/60">{t().aspect}</div>
                    <div class="flex flex-wrap gap-1">
                        {Aspect::ALL
                            .into_iter()
                            .map(|aspect| {
                                let id = aspects.clone();
                                let pressed = move || edits.get().aspect == aspect;
                                view! {
                                    <button
                                        type="button"
                                        aria-pressed=move || pressed().to_string()
                                        on:click=move |_| set_edits(&id, "aspect", |current| Edits { aspect, crop: fit_crop(size, &current, current.crop, aspect_ratio(aspect, frame_size(size, current.quarter))), ..current })
                                        class=move || if pressed() { "rounded-md px-2 py-1 text-xs bg-accent text-white" } else { "rounded-md px-2 py-1 text-xs bg-white/8 text-white/70 hover:bg-white/15" }
                                    >
                                        {aspect_label(aspect)}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
            </section>
            {GROUPS
                .iter()
                .map(|group| {
                    let id = adjusted.clone();
                    view! {
                        <section class="space-y-2.5 border-b border-white/10 p-4">
                            <h3 class="text-xs font-semibold tracking-wide text-white/50 uppercase">{(group.title)()}</h3>
                            {group
                                .keys
                                .iter()
                                .map(|(adjustment, min)| {
                                    let adjustment = *adjustment;
                                    let (reset, change) = (id.clone(), id.clone());
                                    let label = adjustment_label(adjustment);
                                    view! {
                                        <label class="block text-xs text-white/75" title=t().double_click_reset>
                                            <span class="flex justify-between">
                                                {label}
                                                " "
                                                <span class="text-white/50 tabular-nums">{move || number(edits.get().adjust[adjustment])}</span>
                                            </span>
                                            <RangeInput
                                                label=label
                                                min=*min
                                                max=100.0
                                                step=1.0
                                                value=Signal::derive(move || edits.get().adjust[adjustment])
                                                on_reset=move |()| set_edits(&reset, adjustment.key(), |mut current| {
                                                    current.adjust[adjustment] = 0.0;
                                                    current
                                                })
                                                on_change=move |value| set_edits(&change, adjustment.key(), |mut current| {
                                                    current.adjust[adjustment] = value;
                                                    current
                                                })
                                                class="h-1 cursor-pointer accent-accent mt-1 w-full"
                                            />
                                        </label>
                                    }
                                })
                                .collect_view()}
                        </section>
                    }
                })
                .collect_view()}
            <section class="flex gap-1 p-4">
                <IconButton tone=Tone::Overlay icon=icons::CLIPBOARD_COPY label=t().copy_adjustments on_click=move |()| store.copied_adjustments.set(Some(edits.get_untracked().adjust)) />
                <IconButton
                    tone=Tone::Overlay
                    icon=icons::CLIPBOARD_PASTE
                    label=t().paste_adjustments
                    disabled=Signal::derive(move || store.copied_adjustments.with(Option::is_none))
                    on_click=move |()| {
                        if let Some(copied) = store.copied_adjustments.get_untracked() {
                            set_edits(&pasted, "paste", |current| Edits { adjust: copied, ..current });
                        }
                    }
                />
                <div class="flex-1"></div>
                <IconButton tone=Tone::Overlay icon=icons::UNDO_2 label=t().reset_all on_click=move |()| set_edits(&reset_all, "reset", |_| Edits::default()) />
            </section>
        </aside>
    }
}

#[component]
fn CategoryPicker(id: PhotoId) -> impl IntoView {
    let store = store();
    let categories = Memo::new(move |_| store.doc.with(|doc| doc.categories.clone()));
    let current = Memo::new({
        let id = id.clone();
        move |_| store.doc.with(|doc| doc.photo_category(&id).clone())
    });
    view! {
        <div class="mr-2 flex gap-1">
            {move || {
                categories
                    .get()
                    .into_iter()
                    .enumerate()
                    .map(|(index, category)| {
                        let chosen = category.id.clone();
                        let pressed = category.id == current.get();
                        let photo = id.clone();
                        view! {
                            <button
                                type="button"
                                title=fill(t().photo_category_hint, &[("name", &category.name), ("key", &(index + 1).to_string())])
                                aria-pressed=pressed.to_string()
                                on:click=move |_| {
                                    let (photo, chosen) = (photo.clone(), chosen.clone());
                                    commit(move |doc| ops::set_photo_category(doc, &[photo], &chosen));
                                }
                                style:background-color=pressed.then(|| category.color.clone())
                                class={if pressed {
                                    "flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition-colors text-white"
                                } else {
                                    "flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition-colors text-white/70 hover:bg-white/15 hover:text-white"
                                }}
                            >
                                {(!pressed).then(|| view! { <span class="size-2 rounded-full" style:background-color=category.color.clone()></span> })}
                                {category.name.clone()}
                            </button>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

fn viewer_state() -> Option<Viewer> {
    store().viewer.get_untracked()
}

fn go(delta: isize) {
    let Some(viewer) = viewer_state() else { return };
    let Some(index) = viewer.ids.iter().position(|candidate| *candidate == viewer.id) else { return };
    if let Some(next) = index.checked_add_signed(delta).and_then(|next| viewer.ids.get(next)) {
        store().viewer.set(Some(Viewer { id: next.clone(), ..viewer }));
    }
}

fn archive_and_advance() {
    let Some(viewer) = viewer_state() else { return };
    let id = viewer.id.clone();
    let archived = !store().doc.with_untracked(|doc| doc.meta(&id).archived);
    let index = viewer.ids.iter().position(|candidate| *candidate == id).unwrap_or_default();
    commit({
        let id = id.clone();
        move |doc| ops::set_archived(doc, &[id], archived)
    });
    let remaining: Vec<PhotoId> = viewer.ids.iter().filter(|candidate| **candidate != id).cloned().collect();
    match remaining.get(index.min(remaining.len().saturating_sub(1))).cloned() {
        Some(next) => store().viewer.set(Some(Viewer { ids: Arc::new(remaining), id: next, ..viewer })),
        None => close_viewer(),
    }
}

fn copy_current() {
    let Some(viewer) = viewer_state() else { return };
    let Some(file) = photo_file(&viewer.id) else { return };
    let store = store();
    let edits = store.doc.with_untracked(|doc| doc.meta(&viewer.id).edits);
    let number = viewer.ids.iter().position(|candidate| *candidate == viewer.id).unwrap_or_default() + 1;
    let copying = copy_photo(&file, &edits, store.settings.with_untracked(|settings| settings.export_long_side));
    spawn_local(async move {
        match settle(copying).await {
            Ok(_) => toast(fill(t().copied_photo, &[("number", &number.to_string())])),
            Err(error) => toast(fill(t().copy_failed, &[("reason", &error_message(&error))])),
        }
    });
}

fn toggle_checked(id: PhotoId) {
    commit(move |doc| {
        let checked = !doc.meta(&id).checked;
        ops::update_meta(doc, &[id], |meta| meta.checked = checked);
    });
}

fn is_text_entry(target: Option<web_sys::EventTarget>) -> bool {
    target.is_some_and(|target| target.has_type::<web_sys::HtmlTextAreaElement>() || target.dyn_ref::<web_sys::HtmlInputElement>().is_some_and(|input| input.type_() != "range"))
}

#[component]
fn ViewerScreen() -> impl IntoView {
    let store = store();
    let id = Memo::new(move |_| store.viewer.with(|viewer| viewer.as_ref().map(|viewer| viewer.id.clone()).unwrap_or_else(|| PhotoId(String::new()))));
    let position = Memo::new(move |_| store.viewer.with(|viewer| viewer.as_ref().map_or((0, 0), |viewer| (viewer.ids.iter().position(|candidate| *candidate == viewer.id).unwrap_or_default(), viewer.ids.len()))));
    let meta = Memo::new(move |_| {
        let id = id.get();
        store.doc.with(|doc| {
            let meta = doc.meta(&id);
            (meta.checked, meta.archived)
        })
    });
    let editing = Memo::new(move |_| store.settings.with(|settings| settings.edit_panel_open));
    let set_editing = move |open: bool| store.settings.update(|settings| settings.edit_panel_open = open);
    let cropping = RwSignal::new(false);
    let comparing = RwSignal::new(false);
    let size = Memo::new(move |_| {
        let id = id.get();
        match base_thumb(&id).get() {
            Some(Thumb::Ready { size, .. }) => Some(size),
            _ => None,
        }
    });
    let cropping_shown = Memo::new(move |_| cropping.get() && editing.get());

    Effect::new(move |_| {
        let Some(viewer) = store.viewer.get() else { return };
        let index = viewer.ids.iter().position(|candidate| *candidate == viewer.id).unwrap_or_default();
        for neighbour in [index.checked_sub(1), Some(index + 1)].into_iter().flatten().filter_map(|neighbour| viewer.ids.get(neighbour)) {
            if let Some(file) = photo_file(neighbour) {
                let _ = load_bitmap(&file);
            }
        }
    });

    let target: web_sys::EventTarget = window().into();
    let keys = StoredValue::new_local(vec![
        Listener::new(&target, "keydown", false, move |event| {
            let Some(event) = event.dyn_ref::<KeyboardEvent>() else { return };
            if is_text_entry(event.target()) {
                return;
            }
            let key = event.key();
            let current = id.get_untracked();
            match key.as_str() {
                "Escape" if cropping.get_untracked() => cropping.set(false),
                "Escape" => close_viewer(),
                "Enter" if cropping.get_untracked() => cropping.set(false),
                _ if event.ctrl_key() || event.meta_key() => {
                    if key.to_lowercase() == "c" {
                        event.prevent_default();
                        copy_current();
                    }
                }
                "ArrowLeft" => go(-1),
                "ArrowRight" => go(1),
                "a" | "Delete" => archive_and_advance(),
                "c" | " " => {
                    event.prevent_default();
                    toggle_checked(current);
                }
                "e" => set_editing(!editing.get_untracked()),
                "r" => {
                    set_editing(true);
                    cropping.update(|cropping| *cropping = !*cropping);
                }
                "\\" => comparing.set(true),
                digit if digit.len() == 1 && ("1"..="9").contains(&digit) => {
                    let category = store.doc.with_untracked(|doc| digit.parse::<usize>().ok().and_then(|number| doc.categories.get(number - 1)).map(|category| category.id.clone()));
                    if let Some(category) = category {
                        commit(move |doc| ops::set_photo_category(doc, &[current], &category));
                    }
                }
                _ => {}
            }
        }),
        Listener::new(&target, "keyup", false, move |event| {
            if event.dyn_ref::<KeyboardEvent>().is_some_and(|event| event.key() == "\\") {
                comparing.set(false);
            }
        }),
    ]);
    keys.with_value(|_| ());

    view! {
        <div role="dialog" aria-modal="true" class="fixed inset-0 z-50 flex flex-col bg-[#08090b] text-white">
            <header class="flex h-14 shrink-0 items-center gap-1 px-3">
                <IconButton tone=Tone::Overlay icon=icons::X label=t().close on_click=move |()| close_viewer() />
                <div class="ml-2 min-w-0 flex-1">
                    <div class="truncate text-sm font-medium">{move || id.get().0}</div>
                    <div class="text-xs text-white/50 tabular-nums">{move || position.get().0 + 1}" / "{move || position.get().1}</div>
                </div>
                {move || view! { <CategoryPicker id=id.get() /> }}
                <IconButton
                    tone=Tone::Overlay
                    icon=icons::CHECK
                    label=Signal::derive(move || if meta.get().0 { t().uncheck } else { t().check })
                    active=Signal::derive(move || meta.get().0)
                    on_click=move |()| toggle_checked(id.get_untracked())
                />
                <IconButton
                    tone=Tone::Overlay
                    icon=Signal::derive(move || if meta.get().1 { icons::ARCHIVE_RESTORE } else { icons::ARCHIVE })
                    label=Signal::derive(move || if meta.get().1 { t().restore } else { t().archive })
                    on_click=move |()| archive_and_advance()
                />
                <IconButton tone=Tone::Overlay icon=icons::COPY label=t().copy_photo on_click=move |()| copy_current() />
                <button
                    type="button"
                    title=t().compare
                    aria-label=t().compare
                    on:pointerdown=move |_| comparing.set(true)
                    on:pointerup=move |_| comparing.set(false)
                    on:pointerleave=move |_| comparing.set(false)
                    class="grid size-8 place-items-center rounded-lg text-white/80 hover:bg-white/15 hover:text-white"
                >
                    {svg(icons::EYE, "size-4")}
                </button>
                <IconButton tone=Tone::Overlay icon=icons::SLIDERS_HORIZONTAL label=t().edit_panel active=editing on_click=move |()| set_editing(!editing.get_untracked()) />
            </header>
            <div class="relative flex min-h-0 flex-1">
                <button
                    type="button"
                    aria-label=t().previous
                    title=t().previous
                    disabled=move || position.get().0 == 0
                    on:click=move |_| go(-1)
                    class="absolute top-1/2 left-3 z-10 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white backdrop-blur transition hover:bg-white/20 disabled:opacity-0"
                >
                    {svg(icons::CHEVRON_LEFT, "size-6")}
                </button>
                {move || {
                    let id = id.get();
                    match (photo_file(&id), size.get()) {
                        (Some(file), Some(size)) => view! { <Stage id=id file=file size=size cropping=cropping_shown comparing=comparing /> }.into_any(),
                        _ => view! { <div class="grid flex-1 place-items-center text-sm text-white/50">{t().loading}</div> }.into_any(),
                    }
                }}
                <button
                    type="button"
                    aria-label=t().next
                    title=t().next
                    disabled=move || position.get().0 + 1 >= position.get().1
                    on:click=move |_| go(1)
                    class=move || {
                        if editing.get() {
                            "absolute top-1/2 z-10 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white backdrop-blur transition hover:bg-white/20 disabled:opacity-0 right-[21.75rem]"
                        } else {
                            "absolute top-1/2 z-10 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white backdrop-blur transition hover:bg-white/20 disabled:opacity-0 right-3"
                        }
                    }
                >
                    {svg(icons::CHEVRON_RIGHT, "size-6")}
                </button>
                {move || {
                    let id = id.get();
                    match (editing.get(), size.get()) {
                        (true, Some(size)) => Some(view! { <EditPanel id=id size=size cropping=cropping /> }),
                        _ => None,
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn ViewerLayer() -> impl IntoView {
    let open = Memo::new(move |_| store().viewer.with(Option::is_some));
    move || open.get().then(|| view! { <ViewerScreen /> })
}
