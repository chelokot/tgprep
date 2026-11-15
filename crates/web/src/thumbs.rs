use crate::browser::{Frame, Serial, Timeout, document, wait, window};
use crate::idb::KeyValueStore;
use crate::renderer::{Canvas, Renderer};
use crate::store::{PhotoFile, store};
use js_sys::{Object, Reflect};
use leptos::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tgprep_model::doc::{Edits, PhotoId};
use tgprep_model::geometry::{Size, fit_within, output_size};
use tgprep_worker::{Job, Request, Response, Thumbnail};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Blob, File, ImageBitmap, ImageEncodeOptions, MessageEvent, OffscreenCanvas, Url, Worker,
    WorkerOptions, WorkerType,
};

const SHORT_TO_LONG_SIDE: f64 = 1.5;
const THUMB_SIDE_STEP: f64 = 256.0;
const EDITED_DELAY_MS: i32 = 250;
const EDITED_QUALITY: f64 = 0.85;

#[derive(Debug, Clone, PartialEq)]
pub enum Thumb {
    Ready { url: String, size: Size },
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayThumb {
    pub url: Option<String>,
    pub aspect: f64,
    pub edited: bool,
}

#[derive(Default)]
struct Pool {
    idle: Vec<Worker>,
    queue: Vec<Rc<PhotoFile>>,
    ahead: Vec<Rc<PhotoFile>>,
    ahead_rank: HashMap<PhotoId, usize>,
    promoted: HashSet<PhotoId>,
    in_flight: HashMap<String, (PhotoId, Worker, u32)>,
    requested: HashSet<PhotoId>,
    generation: u32,
}

impl Pool {
    fn nearest_ahead(&mut self) -> Option<Rc<PhotoFile>> {
        let position = self
            .ahead
            .iter()
            .enumerate()
            .min_by_key(|(_, file)| self.ahead_rank.get(&file.id))
            .map(|(position, _)| position)?;
        Some(self.ahead.swap_remove(position))
    }
}

struct Pending {
    updates: Vec<(PhotoId, Thumb)>,
    frame: Option<Frame>,
}

thread_local! {
    static POOL: RefCell<Pool> = RefCell::new(Pool::default());
    static THUMBS: RefCell<HashMap<PhotoId, ArcRwSignal<Option<Thumb>>>> = RefCell::new(HashMap::new());
    static BLOBS: RefCell<HashMap<PhotoId, Blob>> = RefCell::new(HashMap::new());
    static PENDING: RefCell<Pending> = const { RefCell::new(Pending { updates: Vec::new(), frame: None }) };
    static CACHE: KeyValueStore = KeyValueStore::new("tgprep-thumbs", "thumbs");
    static EDITED: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
    static RENDERING: Rc<Serial> = Rc::new(Serial::default());
    static THUMB_SIDE: ArcRwSignal<f64> = ArcRwSignal::new(0.0);
}

pub fn spawn_worker(on_message: impl Fn(&Worker, Response) + 'static) -> Option<Worker> {
    let script = document()
        .query_selector("meta[name=tgprep-worker]")
        .ok()
        .flatten()?
        .get_attribute("content")?;
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    let worker = Worker::new_with_options(&script, &options).ok()?;
    let receiver = worker.clone();
    let handler = Closure::<dyn Fn(MessageEvent)>::new(move |event: MessageEvent| {
        if let Ok(response) = Response::from_js(&event.data()) {
            on_message(&receiver, response);
        }
    });
    worker.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
    Some(worker)
}

fn thumb_long_side() -> f64 {
    THUMB_SIDE.with(|side| side.get_untracked())
}

pub fn require_thumb_short_side(css_pixels: f64) {
    let device = css_pixels * SHORT_TO_LONG_SIDE * window().device_pixel_ratio();
    let side = (device / THUMB_SIDE_STEP).ceil().max(2.0) * THUMB_SIDE_STEP;
    THUMB_SIDE.with(|current| {
        if side > current.get_untracked() {
            POOL.with_borrow_mut(|pool| {
                pool.requested = pool.ahead.iter().map(|file| file.id.clone()).collect();
                pool.promoted.clear();
            });
            current.set(side);
        }
    });
}

fn cache_key(file: &PhotoFile, long_side: f64) -> String {
    format!(
        "{}|{}|{}|{long_side}",
        file.id, file.size, file.last_modified
    )
}

fn signal(id: &PhotoId) -> ArcRwSignal<Option<Thumb>> {
    THUMBS.with_borrow_mut(|thumbs| {
        thumbs
            .entry(id.clone())
            .or_insert_with(|| ArcRwSignal::new(None))
            .clone()
    })
}

fn publish(id: PhotoId, thumb: Thumb) {
    PENDING.with_borrow_mut(|pending| {
        pending.updates.push((id, thumb));
        if pending.frame.is_none() {
            pending.frame = Some(Frame::request(|_| {
                let updates = PENDING.with_borrow_mut(|pending| {
                    pending.frame.take();
                    std::mem::take(&mut pending.updates)
                });
                for (id, thumb) in updates {
                    signal(&id).set(Some(thumb));
                }
            }));
        }
    });
}

fn store_thumb(id: PhotoId, thumbnail: Option<&Thumbnail>) {
    let thumb = thumbnail.and_then(|thumbnail| {
        let url = Url::create_object_url_with_blob(&thumbnail.blob).ok()?;
        BLOBS.with_borrow_mut(|blobs| blobs.insert(id.clone(), thumbnail.blob.clone()));
        Some(Thumb::Ready {
            url,
            size: Size {
                w: thumbnail.width,
                h: thumbnail.height,
            },
        })
    });
    publish(id, thumb.unwrap_or(Thumb::Failed));
}

fn cached_thumbnail(value: &JsValue) -> Option<Thumbnail> {
    let field = |name: &str| Reflect::get(value, &JsValue::from_str(name)).ok();
    Some(Thumbnail {
        blob: field("blob")?.dyn_into().ok()?,
        width: field("w")?.as_f64()?,
        height: field("h")?.as_f64()?,
    })
}

fn cache_entry(thumbnail: &Thumbnail) -> JsValue {
    let entry = Object::new();
    let _ = Reflect::set(&entry, &JsValue::from_str("blob"), &thumbnail.blob);
    let _ = Reflect::set(
        &entry,
        &JsValue::from_str("w"),
        &JsValue::from_f64(thumbnail.width),
    );
    let _ = Reflect::set(
        &entry,
        &JsValue::from_str("h"),
        &JsValue::from_f64(thumbnail.height),
    );
    entry.into()
}

fn pump() {
    loop {
        let next = POOL.with_borrow_mut(|pool| {
            if pool.idle.is_empty() {
                return None;
            }
            let file = pool.queue.pop().or_else(|| pool.nearest_ahead())?;
            let worker = pool.idle.pop()?;
            let long_side = thumb_long_side();
            let key = cache_key(&file, long_side);
            pool.in_flight.insert(
                key.clone(),
                (file.id.clone(), worker.clone(), pool.generation),
            );
            Some((file, worker, key, long_side))
        });
        let Some((file, worker, key, long_side)) = next else {
            return;
        };
        spawn_local(async move {
            match wait::<File>(file.handle.get_file()).await {
                Ok(data) => {
                    let _ = worker.post_message(
                        &Request {
                            job: Job::Thumb,
                            key,
                            file: data,
                            long_side,
                        }
                        .to_js(),
                    );
                }
                Err(_) => received(
                    &worker,
                    Response::Thumb {
                        key,
                        thumbnail: None,
                    },
                ),
            }
        });
    }
}

fn received(worker: &Worker, response: Response) {
    match response {
        Response::Ready => POOL.with_borrow_mut(|pool| pool.idle.push(worker.clone())),
        Response::Thumb { key, thumbnail } => {
            let finished = POOL.with_borrow_mut(|pool| {
                let (id, worker, generation) = pool.in_flight.remove(&key)?;
                pool.idle.push(worker);
                Some((id, generation == pool.generation))
            });
            if let Some((id, true)) = finished {
                store_thumb(id, thumbnail.as_ref());
                if let Some(thumbnail) = thumbnail {
                    spawn_local(async move {
                        let _ = CACHE
                            .with(Clone::clone)
                            .set(&key, &cache_entry(&thumbnail))
                            .await;
                    });
                }
            }
        }
        Response::View { .. } => {}
    }
    pump();
}

pub fn start_workers() {
    let available = window().navigator().hardware_concurrency() - 1.0;
    let count = available.clamp(1.0, 4.0);
    let mut started = 0.0;
    while started < count {
        spawn_worker(received);
        started += 1.0;
    }
}

#[derive(Clone, Copy)]
enum Priority {
    Shown,
    Ahead,
}

fn request_thumb(file: Rc<PhotoFile>, priority: Priority) {
    let requested = POOL.with_borrow_mut(|pool| {
        if pool.requested.insert(file.id.clone()) {
            return true;
        }
        if let Priority::Shown = priority {
            match pool.ahead.iter().position(|waiting| waiting.id == file.id) {
                Some(position) => {
                    let promoted = pool.ahead.swap_remove(position);
                    pool.queue.push(promoted);
                }
                None => {
                    pool.promoted.insert(file.id.clone());
                }
            }
        }
        false
    });
    if !requested {
        return;
    }
    spawn_local(async move {
        let cached = CACHE
            .with(Clone::clone)
            .get(&cache_key(&file, thumb_long_side()))
            .await
            .ok()
            .and_then(|value| cached_thumbnail(&value));
        if let Some(thumbnail) = cached {
            store_thumb(file.id.clone(), Some(&thumbnail));
            return;
        }
        POOL.with_borrow_mut(|pool| {
            let promoted = pool.promoted.remove(&file.id);
            match priority {
                Priority::Ahead if !promoted => {
                    if pool.ahead_rank.contains_key(&file.id) {
                        pool.ahead.push(file);
                    } else {
                        pool.requested.remove(&file.id);
                    }
                }
                _ => pool.queue.push(file),
            }
        });
        pump();
    });
}

pub fn read_ahead(ids: &[PhotoId]) {
    let Some(files) = store().library.with_untracked(|library| {
        library.as_ref().map(|library| {
            ids.iter()
                .filter_map(|id| library.by_id.get(id).cloned())
                .collect::<Vec<_>>()
        })
    }) else {
        return;
    };
    POOL.with_borrow_mut(|pool| {
        pool.ahead_rank = ids
            .iter()
            .enumerate()
            .map(|(rank, id)| (id.clone(), rank))
            .collect();
        let (kept, dropped): (Vec<_>, Vec<_>) = std::mem::take(&mut pool.ahead)
            .into_iter()
            .partition(|file| pool.ahead_rank.contains_key(&file.id));
        for file in dropped {
            pool.requested.remove(&file.id);
        }
        pool.ahead = kept;
    });
    for file in files {
        request_thumb(file, Priority::Ahead);
    }
}

pub fn reset_thumbs() {
    POOL.with_borrow_mut(|pool| {
        pool.requested.clear();
        pool.queue.clear();
        pool.ahead.clear();
        pool.ahead_rank.clear();
        pool.promoted.clear();
        pool.generation += 1;
    });
    THUMBS.with_borrow_mut(HashMap::clear);
    BLOBS.with_borrow_mut(HashMap::clear);
}

pub fn base_thumb(id: &PhotoId) -> ArcRwSignal<Option<Thumb>> {
    let file = store().library.with_untracked(|library| {
        library
            .as_ref()
            .and_then(|library| library.by_id.get(id).cloned())
    });
    if let Some(file) = file
        && thumb_long_side() > 0.0
    {
        request_thumb(file, Priority::Shown);
    }
    signal(id)
}

pub fn thumb_url(id: &PhotoId) -> Option<String> {
    THUMBS.with_borrow(
        |thumbs| match thumbs.get(id).and_then(|thumb| thumb.get_untracked()) {
            Some(Thumb::Ready { url, .. }) => Some(url),
            _ => None,
        },
    )
}

fn edit_key(id: &PhotoId, edits: &Edits) -> String {
    format!(
        "{id}|{}|{}",
        thumb_long_side(),
        tgprep_model::json::stringify(edits).unwrap_or_default()
    )
}

async fn render_edited(id: PhotoId, size: Size, edits: Edits, key: String) -> Option<String> {
    let _guard = RENDERING.with(Clone::clone).lock().await;
    if let Some(url) = EDITED.with_borrow(|edited| edited.get(&key).cloned()) {
        return Some(url);
    }
    let blob = BLOBS.with_borrow(|blobs| blobs.get(&id).cloned())?;
    let bitmap: ImageBitmap = wait(window().create_image_bitmap_with_blob(&blob).ok()?)
        .await
        .ok()?;
    let target = fit_within(output_size(size, &edits), thumb_long_side());
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded thumbnail sizes"
    )]
    let output = (target.w as u32, target.h as u32);
    let canvas = RENDERER.with_borrow_mut(|renderer| {
        if renderer.is_none() {
            *renderer = OffscreenCanvas::new(1, 1)
                .ok()
                .and_then(|canvas| Renderer::new(Canvas::Offscreen(canvas)).ok());
        }
        let renderer = renderer.as_mut()?;
        renderer.render(&bitmap, &edits, edits.crop, output).ok()?;
        match &renderer.canvas {
            Canvas::Offscreen(canvas) => Some(canvas.clone()),
            Canvas::Element(_) => None,
        }
    })?;
    let options = ImageEncodeOptions::new();
    options.set_type("image/webp");
    options.set_quality(EDITED_QUALITY);
    let rendered = wait::<Blob>(canvas.convert_to_blob_with_options(&options).ok()?).await;
    RENDERER.with_borrow_mut(|renderer| {
        if let Some(renderer) = renderer.as_mut() {
            renderer.forget(&bitmap);
        }
    });
    bitmap.close();
    let url = Url::create_object_url_with_blob(&rendered.ok()?).ok()?;
    EDITED.with_borrow_mut(|edited| edited.insert(key, url.clone()));
    Some(url)
}

struct PendingRender {
    _timer: Timeout,
    cancelled: Rc<Cell<bool>>,
}

impl Drop for PendingRender {
    fn drop(&mut self) {
        self.cancelled.set(true);
    }
}

pub fn display_thumb(id: PhotoId) -> Memo<Option<DisplayThumb>> {
    let base = signal(&id);
    Effect::new({
        let id = id.clone();
        move |_| {
            THUMB_SIDE.with(|side| side.track());
            base_thumb(&id);
        }
    });
    let edits = Memo::new({
        let id = id.clone();
        move |_| store().doc.with(|doc| doc.meta(&id).edits)
    });
    let rendered: RwSignal<Option<String>> = RwSignal::new(None);
    let watched = base.clone();
    Effect::new(move |_: Option<Option<PendingRender>>| {
        let edits = edits.get();
        let Some(Thumb::Ready { size, .. }) = watched.get() else {
            return None;
        };
        if !edits.is_edited() {
            return None;
        }
        let key = edit_key(&id, &edits);
        if let Some(url) = EDITED.with_borrow(|edited| edited.get(&key).cloned()) {
            rendered.set(Some(url));
            return None;
        }
        let cancelled = Rc::new(Cell::new(false));
        let flag = cancelled.clone();
        let id = id.clone();
        let timer = Timeout::new(EDITED_DELAY_MS, move || {
            spawn_local(async move {
                let url = render_edited(id, size, edits, key).await;
                if !flag.get() && url.is_some() {
                    rendered.set(url);
                }
            });
        });
        Some(PendingRender {
            _timer: timer,
            cancelled,
        })
    });
    Memo::new(move |_| {
        let edits = edits.get();
        let edited = edits.is_edited();
        match base.get()? {
            Thumb::Failed => Some(DisplayThumb {
                url: None,
                aspect: 1.0,
                edited,
            }),
            Thumb::Ready { url, size } => {
                let output = output_size(size, &edits);
                let url = if edited {
                    rendered.get().unwrap_or(url)
                } else {
                    url
                };
                Some(DisplayThumb {
                    url: Some(url),
                    aspect: output.w / output.h,
                    edited,
                })
            }
        }
    })
}
