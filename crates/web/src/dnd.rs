use crate::browser::{Listener, document, now, window};
use crate::i18n::t;
use crate::store::{CategoryFilter, clear_selection, commit, new_id, store};
use crate::thumbs::thumb_url;
use leptos::prelude::*;
use std::cell::RefCell;
use tgprep_model::doc::{CategoryId, PhotoId, PostId};
use tgprep_model::ops::{self, PhotoDestination};
use wasm_bindgen::prelude::*;
use web_sys::{Element, KeyboardEvent, PointerEvent};

const DRAG_THRESHOLD_PX: f64 = 5.0;
const EDGE_BAND_PX: f64 = 14.0;
const AUTOSCROLL_ZONE_PX: f64 = 72.0;
const AUTOSCROLL_SPEED: f64 = 0.03;
const LINE_PX: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub enum DragPayload {
    Photos {
        ids: Vec<PhotoId>,
        from_post: Option<PostId>,
    },
    Post {
        post: PostId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn of(element: &Element) -> Self {
        let rect = element.get_bounding_client_rect();
        Self {
            left: rect.left(),
            top: rect.top(),
            width: rect.width(),
            height: rect.height(),
        }
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }

    fn right(&self) -> f64 {
        self.left + self.width
    }

    fn style(&self) -> String {
        format!(
            "left: {}px; top: {}px; width: {}px; height: {}px;",
            self.left, self.top, self.width, self.height
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DropTarget {
    Post {
        post: PostId,
        index: usize,
        valid: bool,
        ring: Bounds,
        line: Option<Bounds>,
    },
    Gap {
        index: usize,
        category: CategoryId,
        line: Bounds,
    },
    Library {
        ring: Bounds,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickModifiers {
    pub range: bool,
    pub toggle: bool,
}

#[derive(Clone, Copy)]
struct Drag {
    payload: RwSignal<Option<DragPayload>>,
    pointer: RwSignal<(f64, f64)>,
    target: RwSignal<Option<DropTarget>>,
}

thread_local! {
    static DRAG: Drag = Drag { payload: RwSignal::new(None), pointer: RwSignal::new((0.0, 0.0)), target: RwSignal::new(None) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

fn drag() -> Drag {
    DRAG.with(|drag| *drag)
}

struct Card {
    element: Element,
    index: usize,
    bounds: Bounds,
}

fn index_attribute(element: &Element, name: &str) -> Option<usize> {
    element.get_attribute(name)?.parse().ok()
}

fn all(root: &Element, selector: &str) -> Vec<Element> {
    root.query_selector_all(selector).map_or_else(
        |_| Vec::new(),
        |list| {
            (0..list.length())
                .filter_map(|index| list.item(index)?.dyn_into().ok())
                .collect()
        },
    )
}

fn gap_target(view: &Element, cards: &[Card], index: usize) -> DropTarget {
    let view_bounds = Bounds::of(view);
    let before = index
        .checked_sub(1)
        .and_then(|previous| cards.iter().find(|card| card.index == previous));
    let after = cards.iter().find(|card| card.index == index);
    let y = match (before, after) {
        (Some(before), Some(after)) => f64::midpoint(before.bounds.bottom(), after.bounds.top),
        (Some(before), None) => before.bounds.bottom() + 8.0,
        (None, Some(after)) => after.bounds.top - 8.0,
        (None, None) => view_bounds.top + 24.0,
    };
    let category = store().doc.with_untracked(|doc| {
        let filter = CategoryFilter::from(view.get_attribute("data-category").unwrap_or_default());
        match filter {
            CategoryFilter::Only(category) => category,
            CategoryFilter::All => index
                .checked_sub(1)
                .and_then(|previous| doc.posts.get(previous))
                .or_else(|| doc.posts.get(index))
                .map_or_else(
                    || doc.first_category().id.clone(),
                    |post| post.category.clone(),
                ),
        }
    });
    DropTarget::Gap {
        index,
        category,
        line: Bounds {
            left: view_bounds.left + 12.0,
            top: y - LINE_PX / 2.0,
            width: view_bounds.width - 24.0,
            height: LINE_PX,
        },
    }
}

fn is_noop_insert(items: &[PhotoId], ids: &[PhotoId], index: usize) -> bool {
    let positions: Option<Vec<usize>> = ids
        .iter()
        .map(|id| items.iter().position(|item| item == id))
        .collect();
    let Some(positions) = positions else {
        return false;
    };
    let (Some(first), Some(last)) = (positions.iter().min(), positions.iter().max()) else {
        return false;
    };
    last - first + 1 == ids.len() && index >= *first && index <= last + 1
}

struct TileBox {
    index: usize,
    bounds: Bounds,
}

fn post_target(card: &Card, ids: &[PhotoId], point: (f64, f64)) -> Option<DropTarget> {
    let post = store()
        .doc
        .with_untracked(|doc| doc.posts.get(card.index).cloned())?;
    let ring = card.bounds;
    if !ops::can_place_in_post(&post, ids) {
        return Some(DropTarget::Post {
            post: post.id,
            index: post.items.len(),
            valid: false,
            ring,
            line: None,
        });
    }
    let tiles: Vec<TileBox> = all(&card.element, "[data-tile-index]")
        .iter()
        .filter_map(|element| {
            Some(TileBox {
                index: index_attribute(element, "data-tile-index")?,
                bounds: Bounds::of(element),
            })
        })
        .collect();
    let Some(last_tile) = tiles.last() else {
        return Some(DropTarget::Post {
            post: post.id,
            index: 0,
            valid: true,
            ring,
            line: None,
        });
    };
    let mut rows: Vec<Vec<&TileBox>> = Vec::new();
    for tile in &tiles {
        match rows.last_mut() {
            Some(row) if (row[0].bounds.top - tile.bounds.top).abs() < 2.0 => row.push(tile),
            _ => rows.push(vec![tile]),
        }
    }
    let last_row = rows.last()?;
    let row = rows
        .iter()
        .find(|row| point.1 <= row[0].bounds.bottom() + 2.0)
        .unwrap_or(last_row);
    let beyond_end = point.1 > last_tile.bounds.bottom() + 2.0;
    let next = if beyond_end {
        None
    } else {
        row.iter()
            .find(|tile| point.0 < tile.bounds.left + tile.bounds.width / 2.0)
            .copied()
    };
    let previous = match next {
        Some(next) => row
            .iter()
            .find(|tile| Some(tile.index) == next.index.checked_sub(1))
            .copied(),
        None if beyond_end => Some(last_tile),
        None => row.last().copied(),
    };
    let index = match (next, previous) {
        (Some(next), _) => next.index,
        (None, Some(previous)) => previous.index + 1,
        (None, None) => return None,
    };
    if is_noop_insert(&post.items, ids, index) {
        return None;
    }
    let (x, row_bounds) = match (next, previous) {
        (Some(next), Some(previous)) => (
            f64::midpoint(previous.bounds.right(), next.bounds.left),
            next.bounds,
        ),
        (Some(next), None) => (next.bounds.left - 3.0, next.bounds),
        (None, Some(previous)) => (previous.bounds.right() + 3.0, previous.bounds),
        (None, None) => return None,
    };
    Some(DropTarget::Post {
        post: post.id,
        index,
        valid: true,
        ring,
        line: Some(Bounds {
            left: x - LINE_PX / 2.0,
            top: row_bounds.top,
            width: LINE_PX,
            height: row_bounds.height,
        }),
    })
}

fn resolve_target(payload: &DragPayload, point: (f64, f64)) -> Option<DropTarget> {
    #[expect(clippy::cast_possible_truncation, reason = "client coordinates")]
    let element = document().element_from_point(point.0 as f32, point.1 as f32)?;
    if let Some(library) = element.closest("[data-drop=\"library\"]").ok().flatten() {
        return matches!(
            payload,
            DragPayload::Photos {
                from_post: Some(_),
                ..
            }
        )
        .then(|| DropTarget::Library {
            ring: Bounds::of(&library),
        });
    }
    let view = element.closest("[data-drop=\"posts\"]").ok().flatten()?;
    let cards: Vec<Card> = all(&view, "[data-post-index]")
        .into_iter()
        .filter_map(|element| {
            Some(Card {
                index: index_attribute(&element, "data-post-index")?,
                bounds: Bounds::of(&element),
                element,
            })
        })
        .collect();
    let card = cards
        .iter()
        .find(|card| point.1 >= card.bounds.top && point.1 <= card.bounds.bottom());
    let moving_post = match payload {
        DragPayload::Post { post } => store().doc.with_untracked(|doc| doc.post_index(post)),
        DragPayload::Photos { .. } => None,
    };
    let gap = |index: usize| {
        (moving_post.is_none_or(|from| index != from && index != from + 1))
            .then(|| gap_target(&view, &cards, index))
    };
    let Some(card) = card else {
        let above = cards
            .iter()
            .rfind(|candidate| candidate.bounds.bottom() < point.1);
        let index = above.map_or_else(
            || {
                cards.first().map_or_else(
                    || store().doc.with_untracked(|doc| doc.posts.len()),
                    |card| card.index,
                )
            },
            |above| above.index + 1,
        );
        return gap(index);
    };
    match payload {
        DragPayload::Post { .. } => gap(if point.1 < card.bounds.top + card.bounds.height / 2.0 {
            card.index
        } else {
            card.index + 1
        }),
        DragPayload::Photos { .. } if point.1 < card.bounds.top + EDGE_BAND_PX => gap(card.index),
        DragPayload::Photos { .. } if point.1 > card.bounds.bottom() - EDGE_BAND_PX => {
            gap(card.index + 1)
        }
        DragPayload::Photos { ids, .. } => post_target(card, ids, point),
    }
}

fn autoscroll(point: (f64, f64), elapsed: f64) {
    #[expect(clippy::cast_possible_truncation, reason = "client coordinates")]
    let Some(container) = document()
        .element_from_point(point.0 as f32, point.1 as f32)
        .and_then(|element| element.closest("[data-scroll]").ok().flatten())
    else {
        return;
    };
    let bounds = Bounds::of(&container);
    let over_top = bounds.top + AUTOSCROLL_ZONE_PX - point.1;
    let over_bottom = point.1 - (bounds.bottom() - AUTOSCROLL_ZONE_PX);
    let push = if over_top > 0.0 {
        -over_top
    } else if over_bottom > 0.0 {
        over_bottom
    } else {
        0.0
    };
    if push != 0.0 {
        #[expect(clippy::cast_possible_truncation, reason = "scroll offsets are pixels")]
        container
            .set_scroll_top(container.scroll_top() + (push * elapsed * AUTOSCROLL_SPEED) as i32);
    }
}

fn drop_on(payload: DragPayload, target: DropTarget) {
    match payload {
        DragPayload::Post { post } => {
            if let DropTarget::Gap { index, .. } = target {
                commit(move |doc| ops::move_post(doc, &post, index));
            }
        }
        DragPayload::Photos { ids, from_post } => {
            match target {
                DropTarget::Library { .. } => {
                    if let Some(source) = from_post {
                        commit(move |doc| ops::unplace_photos(doc, &ids, &source));
                    }
                }
                DropTarget::Gap {
                    index, category, ..
                } => {
                    commit(move |doc| {
                        ops::place_photos(
                            doc,
                            &ids,
                            PhotoDestination::Gap { index, category },
                            from_post.as_ref(),
                            || PostId(new_id()),
                        );
                    });
                }
                DropTarget::Post {
                    post,
                    index,
                    valid: true,
                    ..
                } => {
                    commit(move |doc| {
                        ops::place_photos(
                            doc,
                            &ids,
                            PhotoDestination::Post { post, index },
                            from_post.as_ref(),
                            || PostId(new_id()),
                        );
                    });
                }
                DropTarget::Post { valid: false, .. } => {}
            }
            clear_selection();
        }
    }
}

struct Session {
    start: (f64, f64),
    pointer: (f64, f64),
    modifiers: ClickModifiers,
    payload: Option<DragPayload>,
    get_payload: Option<Box<dyn FnOnce() -> DragPayload>>,
    on_click: Option<Box<dyn FnOnce(ClickModifiers)>>,
    previous_time: f64,
    frame: Option<i32>,
    tick: Option<Closure<dyn FnMut(f64)>>,
    _listeners: Vec<Listener>,
}

fn dataset_dragging(on: bool) {
    if let Some(root) = document().document_element() {
        let _ = if on {
            root.set_attribute("data-dragging", "")
        } else {
            root.remove_attribute("data-dragging")
        };
    }
}

fn request_tick() {
    SESSION.with_borrow_mut(|session| {
        if let Some(session) = session.as_mut()
            && let Some(tick) = &session.tick
        {
            session.frame = window()
                .request_animation_frame(tick.as_ref().unchecked_ref())
                .ok();
        }
    });
}

fn tick(time: f64) {
    let state = SESSION.with_borrow_mut(|session| {
        let session = session.as_mut()?;
        let elapsed = time - session.previous_time;
        session.previous_time = time;
        Some((session.pointer, elapsed, session.payload.clone()?))
    });
    let Some((pointer, elapsed, payload)) = state else {
        return;
    };
    autoscroll(pointer, elapsed);
    drag().target.set(resolve_target(&payload, pointer));
    request_tick();
}

fn moved(event: &PointerEvent) {
    let pointer = (f64::from(event.client_x()), f64::from(event.client_y()));
    let started = SESSION.with_borrow_mut(|session| {
        let session = session.as_mut()?;
        session.pointer = pointer;
        if session.payload.is_none() {
            if (pointer.0 - session.start.0).hypot(pointer.1 - session.start.1) < DRAG_THRESHOLD_PX
            {
                return None;
            }
            session.payload = session.get_payload.take().map(|get_payload| get_payload());
            session.previous_time = now();
            session.tick = Some(Closure::<dyn FnMut(f64)>::new(tick));
            return Some((session.payload.clone()?, true));
        }
        Some((session.payload.clone()?, false))
    });
    let Some((payload, first)) = started else {
        return;
    };
    if first {
        dataset_dragging(true);
        request_tick();
    }
    let drag = drag();
    drag.target.set(resolve_target(&payload, pointer));
    drag.pointer.set(pointer);
    if first {
        drag.payload.set(Some(payload));
    }
}

fn finish(commit_drop: bool) {
    let Some(session) = SESSION.with_borrow_mut(Option::take) else {
        return;
    };
    if let Some(frame) = session.frame {
        let _ = window().cancel_animation_frame(frame);
    }
    dataset_dragging(false);
    let drag = drag();
    drag.payload.set(None);
    drag.target.set(None);
    let Session {
        payload,
        on_click,
        modifiers,
        pointer,
        tick,
        _listeners: listeners,
        ..
    } = session;
    wasm_bindgen_futures::spawn_local(async move { drop((tick, listeners)) });
    match payload {
        None if commit_drop => {
            if let Some(on_click) = on_click {
                on_click(modifiers);
            }
        }
        None => {}
        Some(payload) => {
            if let Some(target) = commit_drop
                .then(|| resolve_target(&payload, pointer))
                .flatten()
            {
                drop_on(payload, target);
            }
        }
    }
}

pub fn press_to_drag(
    event: &PointerEvent,
    get_payload: impl FnOnce() -> DragPayload + 'static,
    on_click: impl FnOnce(ClickModifiers) + 'static,
) {
    if event.button() != 0 {
        return;
    }
    finish(false);
    let target: web_sys::EventTarget = window().into();
    let listeners = vec![
        Listener::new(&target, "pointermove", false, |event| {
            if let Some(event) = event.dyn_ref::<PointerEvent>() {
                moved(event);
            }
        }),
        Listener::new(&target, "pointerup", false, |_| finish(true)),
        Listener::new(&target, "pointercancel", false, |_| finish(false)),
        Listener::new(&target, "keydown", true, |event| {
            if event
                .dyn_ref::<KeyboardEvent>()
                .is_some_and(|key| key.key() == "Escape")
            {
                event.stop_propagation();
                finish(false);
            }
        }),
    ];
    let start = (f64::from(event.client_x()), f64::from(event.client_y()));
    let session = Session {
        start,
        pointer: start,
        modifiers: ClickModifiers {
            range: event.shift_key(),
            toggle: event.ctrl_key() || event.meta_key(),
        },
        payload: None,
        get_payload: Some(Box::new(get_payload)),
        on_click: Some(Box::new(on_click)),
        previous_time: now(),
        frame: None,
        tick: None,
        _listeners: listeners,
    };
    SESSION.with_borrow_mut(|slot| *slot = Some(session));
}

#[component]
fn Ghost() -> impl IntoView {
    let drag = drag();
    let transform = move || {
        let (x, y) = drag.pointer.get();
        format!("transform: translate({}px, {}px);", x + 14.0, y + 14.0)
    };
    move || match drag.payload.get()? {
        DragPayload::Post { post } => {
            let number = store()
                .doc
                .with(|doc| doc.post_index(&post).map_or(0, |index| index + 1));
            Some(view! { <div style=transform class="fixed top-0 left-0 rounded-xl bg-accent px-3 py-2 text-sm font-semibold text-white shadow-2xl">"#"{number}</div> }.into_any())
        }
        DragPayload::Photos { ids, .. } => {
            let count = ids.len();
            let stack = ids
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, id)| {
                    let url = thumb_url(id);
                    #[expect(clippy::cast_precision_loss, reason = "at most three cards")]
                    let offset = index as f64;
                    let style = format!("transform: rotate({}deg) translate({}px, {}px); z-index: {};", (offset - 1.0) * 6.0, offset * 6.0, offset * 4.0, 3 - index);
                    view! {
                        <div class="absolute size-20 overflow-hidden rounded-xl border-2 border-white bg-raised shadow-2xl" style=style>
                            {url.map(|url| view! { <img src=url alt="" class="size-full object-cover" /> })}
                        </div>
                    }
                })
                .collect_view();
            Some(
                view! {
                    <div style=transform class="fixed top-0 left-0">
                        {stack}
                        {(count > 1).then(|| view! { <span class="absolute -top-2 left-16 z-10 rounded-full bg-accent px-2 py-0.5 text-xs font-bold text-white shadow">{count}</span> })}
                    </div>
                }
                .into_any(),
            )
        }
    }
}

#[component]
fn Indicator() -> impl IntoView {
    let drag = drag();
    move || {
        match drag.target.get()? {
        DropTarget::Library { ring } => Some(
            view! {
                <div style=ring.style() class="fixed grid place-items-center rounded-lg bg-danger/10 ring-2 ring-danger ring-inset">
                    <span class="rounded-full bg-danger px-3 py-1.5 text-sm font-semibold text-white shadow-lg">{t().remove_here}</span>
                </div>
            }
            .into_any(),
        ),
        DropTarget::Gap { category, line, .. } => {
            let color = store().doc.with(|doc| doc.category(&category).map(|category| category.color.clone()));
            let style = format!("{}{}", line.style(), color.as_ref().map(|color| format!(" background-color: {color};")).unwrap_or_default());
            Some(
                view! {
                    <div style=style class="fixed rounded-full shadow-[0_0_0.75rem_var(--color-accent)]">
                        <span style:background-color=color class="absolute top-1/2 left-1/2 -translate-1/2 rounded-full px-2.5 py-1 text-xs font-semibold whitespace-nowrap text-white shadow-lg">
                            "+ "{t().new_post_here}
                        </span>
                    </div>
                }
                .into_any(),
            )
        }
        DropTarget::Post { valid, ring, line, .. } => Some(
            view! {
                <div style=ring.style() class={if valid { "fixed rounded-2xl ring-2 ring-inset ring-accent/70" } else { "fixed rounded-2xl ring-2 ring-inset bg-danger/10 ring-danger" }}>
                    {(!valid).then(|| view! { <span class="absolute top-2 right-2 rounded-full bg-danger px-2.5 py-1 text-xs font-semibold text-white shadow">{t().max_photos}</span> })}
                </div>
                {line.map(|line| view! { <div style=line.style() class="fixed rounded-full bg-accent shadow-[0_0_0.75rem_var(--color-accent)]"></div> })}
            }
            .into_any(),
        ),
    }
    }
}

#[component]
pub fn DragOverlay() -> impl IntoView {
    view! {
        <div class="pointer-events-none fixed inset-0 z-[60]">
            <Indicator />
            <Ghost />
        </div>
    }
}
