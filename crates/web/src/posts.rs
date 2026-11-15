use crate::browser::{Frame, error_message, new_id, settle, write_text};
use crate::dnd::{DragPayload, press_to_drag};
use crate::export::{copy_photo, export_post, prepare_png};
use crate::i18n::{fill, t};
use crate::icons::{self, svg};
use crate::store::{
    CategoryFilter, Mode, PostsView, ViewKey, commit, commit_keyed, open_viewer, store, toast,
};
use crate::thumbs::display_thumb;
use crate::tile::{CornerButton, Fit, TileProps, tile};
use crate::ui::{Chip, IconButton, PaneHeader, RangeInput};
use crate::virtual_list::{ResizeWatch, centered_offset, visible_range, watch_viewport};
use leptos::html::Div;
use leptos::prelude::*;
use std::collections::HashMap;

use tgprep_model::doc::{
    Category, CategoryId, Doc, MAX_CAPTION_LENGTH, MAX_POST_PHOTOS, PhotoId, Post, PostId,
};
use tgprep_model::ops;
use wasm_bindgen_futures::spawn_local;
use web_sys::PointerEvent;

const CARD_GAP_PX: f64 = 12.0;
const ESTIMATED_CARD_PX: f64 = 220.0;
const OVERSCAN: usize = 4;

fn visible_posts(doc: &Doc, filter: &CategoryFilter, hide_sent: bool) -> Vec<(PostId, usize)> {
    doc.posts
        .iter()
        .enumerate()
        .filter(|(_, post)| filter.admits(&post.category) && !(hide_sent && post.sent))
        .map(|(index, post)| (post.id.clone(), index))
        .collect()
}

fn active_filter(view: PostsView) -> Memo<CategoryFilter> {
    let store = store();
    Memo::new(move |_| {
        let filter = store
            .settings
            .with(|settings| settings.posts_filter.get(view).clone());
        store.doc.with(|doc| filter.active(doc))
    })
}

fn hides_sent() -> bool {
    let store = store();
    store.mode.get() == Mode::Send && store.settings.with(|settings| settings.hide_sent)
}

fn view_photo_ids(view: PostsView) -> Vec<PhotoId> {
    let store = store();
    let hide_sent = store.mode.get_untracked() == Mode::Send
        && store.settings.with_untracked(|settings| settings.hide_sent);
    let filter = store
        .settings
        .with_untracked(|settings| settings.posts_filter.get(view).clone());
    store.doc.with_untracked(|doc| {
        let filter = filter.active(doc);
        visible_posts(doc, &filter, hide_sent)
            .into_iter()
            .flat_map(|(_, index)| doc.posts[index].items.clone())
            .collect()
    })
}

fn set_filter(view: PostsView, filter: CategoryFilter) {
    store()
        .settings
        .update(|settings| settings.posts_filter.set(view, filter));
}

fn copy_post_photo(id: &PhotoId, number: usize) {
    let store = store();
    let Some(file) = store.library.with_untracked(|library| {
        library
            .as_ref()
            .and_then(|library| library.by_id.get(id).cloned())
    }) else {
        return;
    };
    let edits = store.doc.with_untracked(|doc| doc.meta(id).edits);
    let copying = copy_photo(
        &file,
        &edits,
        store
            .settings
            .with_untracked(|settings| settings.export_long_side),
    );
    let id = id.clone();
    spawn_local(async move {
        match settle(copying).await {
            Ok(_) => {
                commit(move |doc| ops::update_meta(doc, &[id], |meta| meta.checked = true));
                toast(fill(t().copied_photo, &[("number", &number.to_string())]));
            }
            Err(error) => toast(fill(t().copy_failed, &[("reason", &error_message(&error))])),
        }
    });
}

#[component]
fn PostTile(id: PhotoId, post: PostId, view: PostsView) -> impl IntoView {
    let store = store();
    let thumb = display_thumb(id.clone());
    let index = Memo::new({
        let (id, post) = (id.clone(), post.clone());
        move |_| {
            store.doc.with(|doc| {
                doc.post(&post)
                    .and_then(|post| post.items.iter().position(|item| *item == id))
                    .unwrap_or_default()
            })
        }
    });
    let compose = Memo::new(move |_| store.mode.get() == Mode::Compose);
    let style = Signal::derive(move || {
        let height = store.settings.with(|settings| settings.post_tile_height);
        let aspect = thumb.with(|thumb| thumb.as_ref().map_or(1.0, |thumb| thumb.aspect));
        format!("height: {height}rem; width: {}rem;", height * aspect)
    });
    let pressed = (id.clone(), post.clone());
    let on_pointer_down = move |event: PointerEvent| {
        let (id, post) = pressed.clone();
        if store.mode.get_untracked() == Mode::Send {
            if event.button() == 0 {
                copy_post_photo(&id, index.get_untracked() + 1);
            }
            return;
        }
        let clicked = id.clone();
        press_to_drag(
            &event,
            move || DragPayload::Photos {
                ids: vec![id],
                from_post: Some(post),
            },
            move |_| open_viewer(view_photo_ids(view), clicked, ViewKey::Posts(view)),
        );
    };
    let removed = (id.clone(), post);
    tile(
        TileProps {
            id,
            view: ViewKey::Posts(view),
            thumb,
            fit: Fit::Cover,
            actions: compose.into(),
            class: "shrink-0",
            style: Some(style),
        },
        on_pointer_down,
        view! {
            <span data-tile-index=move || index.get() class="absolute inset-0"></span>
            <span class="absolute bottom-1.5 left-1.5 grid size-5 place-items-center rounded-full bg-black/55 text-[0.6875rem] font-semibold text-white backdrop-blur group-hover/tile:opacity-0">
                {move || index.get() + 1}
            </span>
            {move || {
                if compose.get() {
                    let removed = removed.clone();
                    view! {
                        <CornerButton
                            label=t().remove_from_post
                            on_click=move |()| {
                                let (id, post) = removed.clone();
                                commit(move |doc| ops::unplace_photos(doc, &[id], &post));
                            }
                            class="bottom-1.5 left-1.5 bg-black/55 text-white opacity-0 backdrop-blur group-hover/tile:opacity-100 hover:bg-danger"
                        >
                            {svg(icons::X, "size-3.5")}
                        </CornerButton>
                    }
                    .into_any()
                } else {
                    view! {
                        <span class="absolute inset-0 grid place-items-center bg-black/0 opacity-0 transition group-hover/tile:bg-black/35 group-hover/tile:opacity-100">
                            {svg(icons::COPY, "size-6 text-white drop-shadow")}
                        </span>
                    }
                    .into_any()
                }
            }}
        },
    )
}

#[component]
fn PostCard(id: PostId, view: PostsView) -> impl IntoView {
    let store = store();
    let post: Memo<Option<Post>> = Memo::new({
        let id = id.clone();
        move |_| store.doc.with(|doc| doc.post(&id).cloned())
    });
    let index = Memo::new({
        let id = id.clone();
        move |_| {
            store
                .doc
                .with(|doc| doc.post_index(&id).unwrap_or_default())
        }
    });
    let categories = Memo::new(move |_| store.doc.with(|doc| doc.categories.clone()));
    let color = Memo::new(move |_| {
        let category = post.with(|post| post.as_ref().map(|post| post.category.clone()));
        categories.with(|categories| {
            category
                .and_then(|category| categories.iter().find(|candidate| candidate.id == category))
                .or(categories.first())
                .map(|category| category.color.clone())
                .unwrap_or_default()
        })
    });
    let compose = Memo::new(move |_| store.mode.get() == Mode::Compose);
    let sent = Memo::new(move |_| post.with(|post| post.as_ref().is_some_and(|post| post.sent)));
    let caption = Memo::new(move |_| {
        post.with(|post| {
            post.as_ref()
                .map(|post| post.caption.clone())
                .unwrap_or_default()
        })
    });
    let items = Memo::new(move |_| {
        post.with(|post| {
            post.as_ref()
                .map(|post| post.items.clone())
                .unwrap_or_default()
        })
    });
    let caption_length = Memo::new(move |_| caption.with(|caption| caption.encode_utf16().count()));

    let prepare = move |_| {
        if store.mode.get_untracked() != Mode::Send {
            return;
        }
        let long_side = store
            .settings
            .with_untracked(|settings| settings.export_long_side);
        let files: Vec<_> = store.library.with_untracked(|library| {
            items
                .get_untracked()
                .iter()
                .filter_map(|id| {
                    library
                        .as_ref()
                        .and_then(|library| library.by_id.get(id).cloned())
                })
                .collect()
        });
        for file in files {
            let edits = store.doc.with_untracked(|doc| doc.meta(&file.id).edits);
            let _ = prepare_png(&file, &edits, long_side);
        }
    };
    let save = move |()| {
        let Some(library) = store.library.get_untracked() else {
            return;
        };
        let Some(post) = post.get_untracked() else {
            return;
        };
        let photos: Vec<_> = store.doc.with_untracked(|doc| {
            post.items
                .iter()
                .filter_map(|id| Some((library.by_id.get(id).cloned()?, doc.meta(id).edits)))
                .collect()
        });
        let long_side = store
            .settings
            .with_untracked(|settings| settings.export_long_side);
        let number = index.get_untracked() + 1;
        spawn_local(async move {
            if let Ok(path) = export_post(&library.root, &post, number, photos, long_side).await {
                toast(fill(t().exported, &[("path", &path)]));
            }
        });
    };
    let dragged = id.clone();
    let category_id = id.clone();
    let sent_id = id.clone();
    let deleted = id.clone();
    let captioned = id.clone();
    let tiles_post = id;

    view! {
        <article
            data-post-index=move || index.get()
            on:pointerenter=prepare
            class=move || if sent.get() { "rounded-2xl border border-line bg-panel p-3 shadow-sm transition-opacity opacity-55" } else { "rounded-2xl border border-line bg-panel p-3 shadow-sm transition-opacity " }
            style=move || format!("border-left-color: {}; border-left-width: 0.25rem;", color.get())
        >
            <header class="mb-2 flex items-center gap-1.5">
                {move || {
                    let dragged = dragged.clone();
                    compose
                        .get()
                        .then(move || {
                            view! {
                                <button
                                    type="button"
                                    title=t().move_post
                                    aria-label=t().move_post
                                    on:pointerdown=move |event: PointerEvent| {
                                        let post = dragged.clone();
                                        press_to_drag(&event, move || DragPayload::Post { post }, |_| ());
                                    }
                                    class="-ml-1 grid h-7 w-5 cursor-grab place-items-center rounded text-faint hover:bg-raised hover:text-fg"
                                >
                                    {svg(icons::GRIP_VERTICAL, "size-4")}
                                </button>
                            }
                        })
                }}
                <span class="text-sm font-semibold text-fg tabular-nums">"#"{move || index.get() + 1}</span>
                <select
                    aria-label=t().post_category
                    title=t().post_category
                    disabled=move || !compose.get()
                    on:change=move |event| {
                        let category = CategoryId(event_target_value(&event));
                        let post = category_id.clone();
                        commit(move |doc| ops::set_post_category(doc, &post, &category));
                    }
                    style:color=move || color.get()
                    class="max-w-32 truncate rounded-md bg-transparent px-1 py-0.5 text-xs font-semibold hover:bg-raised disabled:appearance-none"
                >
                    <For each=move || categories.get() key=|category: &Category| category.id.clone() let:option>
                        {
                            let chosen = option.id.clone();
                            view! {
                                <option value=option.id.0.clone() prop:selected=move || post.with(|post| post.as_ref().is_some_and(|post| post.category == chosen))>
                                    {option.name.clone()}
                                </option>
                            }
                        }
                    </For>
                </select>
                <span class=move || if items.with(Vec::len) == MAX_POST_PHOTOS { "text-xs tabular-nums text-accent" } else { "text-xs tabular-nums text-faint" }>
                    {move || items.with(Vec::len)}
                    "/"
                    {MAX_POST_PHOTOS}
                </span>
                <div class="flex-1"></div>
                <IconButton
                    icon=icons::SEND
                    label=Signal::derive(move || if sent.get() { t().unmark_sent } else { t().mark_sent })
                    active=sent
                    on_click=move |()| {
                        let (post, sent) = (sent_id.clone(), !sent.get_untracked());
                        commit(move |doc| ops::set_sent(doc, &post, sent));
                    }
                />
                <IconButton icon=icons::DOWNLOAD label=t().export_post disabled=Signal::derive(move || items.with(Vec::is_empty)) on_click=save />
                {move || {
                    let deleted = deleted.clone();
                    compose
                        .get()
                        .then(move || {
                            view! {
                                <IconButton
                                    icon=icons::TRASH_2
                                    label=t().delete_post
                                    on_click=move |()| {
                                        let post = deleted.clone();
                                        commit(move |doc| ops::delete_post(doc, &post));
                                    }
                                />
                            }
                        })
                }}
            </header>
            <div data-post-media="" class="flex min-h-16 flex-wrap gap-1">
                {move || {
                    if items.with(Vec::is_empty) {
                        view! { <div class="grid h-16 w-full place-items-center rounded-lg border border-dashed border-line text-xs text-faint">{t().drop_here}</div> }.into_any()
                    } else {
                        let post = tiles_post.clone();
                        view! { <For each=move || items.get() key=Clone::clone let:id><PostTile id=id post=post.clone() view=view /></For> }.into_any()
                    }
                }}
            </div>
            {move || {
                let captioned = captioned.clone();
                if compose.get() {
                    view! {
                        <div class="mt-2">
                            <textarea
                                prop:value=move || caption.get()
                                placeholder=t().caption
                                rows="1"
                                on:input=move |event| {
                                    let (post, text) = (captioned.clone(), event_target_value(&event));
                                    commit_keyed(Some(format!("caption:{post}")), move |doc| ops::set_caption(doc, &post, text));
                                }
                                class="block max-h-60 w-full resize-none rounded-lg bg-raised px-3 py-2 text-sm text-fg outline-none [field-sizing:content] placeholder:text-faint focus:ring-2 focus:ring-accent/50"
                            ></textarea>
                            {move || {
                                let length = caption_length.get();
                                (length > 0)
                                    .then(|| {
                                        view! {
                                            <div
                                                title=fill(t().caption_limit, &[("limit", &MAX_CAPTION_LENGTH.to_string())])
                                                class={if length > MAX_CAPTION_LENGTH { "mt-1 text-right text-[0.6875rem] tabular-nums text-danger" } else { "mt-1 text-right text-[0.6875rem] tabular-nums text-faint" }}
                                            >
                                                {length}
                                                "/"
                                                {MAX_CAPTION_LENGTH}
                                            </div>
                                        }
                                    })
                            }}
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <button
                            type="button"
                            title=t().copy_caption
                            disabled=move || caption.with(|caption| caption.trim().is_empty())
                            on:click=move |_| {
                                let copying = write_text(&caption.get_untracked());
                                spawn_local(async move {
                                    match settle(copying).await {
                                        Ok(_) => toast(t().copied_caption.to_owned()),
                                        Err(error) => toast(fill(t().copy_failed, &[("reason", &error_message(&error))])),
                                    }
                                });
                            }
                            class="group/caption mt-2 flex w-full items-start gap-2 rounded-lg bg-raised px-3 py-2 text-left text-sm whitespace-pre-wrap text-fg transition hover:bg-accent/15 disabled:text-faint disabled:hover:bg-raised"
                        >
                            <span class="flex-1">{move || caption.with(|caption| if caption.trim().is_empty() { t().no_caption.to_owned() } else { caption.clone() })}</span>
                            {svg(icons::COPY, "mt-0.5 size-4 shrink-0 text-faint group-hover/caption:text-accent")}
                        </button>
                    }
                    .into_any()
                }
            }}
        </article>
    }
}

#[component]
fn CategoryChip(id: CategoryId, view: PostsView, active: Memo<CategoryFilter>) -> impl IntoView {
    let store = store();
    let editing = RwSignal::new(false);
    let deletable = Memo::new(move |_| store.doc.with(|doc| doc.categories.len() > 1));
    let current = Memo::new({
        let id = id.clone();
        move |_| store.doc.with(|doc| doc.category(&id).cloned())
    });
    move || {
        let id = id.clone();
        if editing.get() {
            let renamed = id.clone();
            let deleted = id.clone();
            let name = current
                .get_untracked()
                .map(|category| category.name)
                .unwrap_or_default();
            view! {
                <span class="flex shrink-0 items-center gap-1 rounded-full bg-raised py-0.5 pr-1 pl-2.5">
                    <input
                        autofocus
                        aria-label=t().category_name
                        value=name
                        on:blur=move |_| editing.set(false)
                        on:keydown=move |event: web_sys::KeyboardEvent| {
                            if (event.key() == "Enter" || event.key() == "Escape")
                                && let Some(input) = event.current_target().and_then(|target| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlElement>(target).ok())
                            {
                                let _ = input.blur();
                            }
                        }
                        on:input=move |event| {
                            let (category, name) = (renamed.clone(), event_target_value(&event));
                            commit_keyed(Some(format!("category:{category}")), move |doc| ops::rename_category(doc, &category, name));
                        }
                        class="w-24 bg-transparent text-xs text-fg outline-none"
                    />
                    {move || {
                        let deleted = deleted.clone();
                        deletable
                            .get()
                            .then(move || {
                                view! {
                                    <button
                                        type="button"
                                        title=t().delete_category
                                        aria-label=t().delete_category
                                        on:pointerdown=|event: PointerEvent| event.prevent_default()
                                        on:click=move |_| {
                                            let category = deleted.clone();
                                            commit(move |doc| ops::delete_category(doc, &category));
                                        }
                                        class="grid size-5 place-items-center rounded-full text-faint hover:bg-danger hover:text-white"
                                    >
                                        {svg(icons::TRASH_2, "size-3")}
                                    </button>
                                }
                            })
                    }}
                </span>
            }
            .into_any()
        } else {
            let Some(category) = current.get() else {
                return ().into_any();
            };
            let selected = category.id.clone();
            view! {
                <span on:dblclick=move |_| editing.set(true)>
                    <Chip
                        label=category.name.clone()
                        active=Signal::derive(move || active.get() == CategoryFilter::Only(selected.clone()))
                        color=Some(category.color.clone())
                        on_click=move |()| set_filter(view, CategoryFilter::Only(id.clone()))
                    >
                        {category.name.clone()}
                    </Chip>
                </span>
            }
            .into_any()
        }
    }
}

struct Measured {
    _watch: ResizeWatch,
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn PostsPane(view: PostsView) -> impl IntoView {
    let store = store();
    let filter = active_filter(view);
    let compose = Memo::new(move |_| store.mode.get() == Mode::Compose);
    let visible = Memo::new(move |_| {
        let (filter, hide_sent) = (filter.get(), hides_sent());
        store.doc.with(|doc| visible_posts(doc, &filter, hide_sent))
    });
    let sizes: RwSignal<HashMap<PostId, f64>> = RwSignal::new(HashMap::new());
    let scroller = NodeRef::<Div>::new();
    let viewport = watch_viewport(scroller);
    let layout = Memo::new(move |_| {
        sizes.with(|sizes| {
            visible.with(|visible| {
                let mut start = CARD_GAP_PX;
                let mut starts = Vec::with_capacity(visible.len());
                let mut heights = Vec::with_capacity(visible.len());
                for (id, _) in visible {
                    let size = sizes.get(id).copied().unwrap_or(ESTIMATED_CARD_PX);
                    starts.push(start);
                    heights.push(size);
                    start += size;
                }
                (starts, heights, start)
            })
        })
    });
    let window = Memo::new(move |_| {
        let range = layout.with(|(starts, heights, _)| {
            visible_range(
                starts,
                heights,
                viewport.scroll_top.get(),
                viewport.height.get(),
                OVERSCAN,
            )
        });
        visible.with(|visible| {
            visible[range]
                .iter()
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>()
        })
    });
    let scroll_to = move |position: usize| {
        let Some(element) = scroller.get_untracked() else {
            return;
        };
        let offset = layout.with_untracked(|(starts, heights, total)| {
            centered_offset(
                starts[position],
                heights[position],
                viewport.height.get_untracked(),
                *total,
            )
        });
        #[expect(clippy::cast_possible_truncation, reason = "scroll offsets are pixels")]
        element.set_scroll_top(offset as i32);
    };
    let settle_frame = StoredValue::new_local(None::<Frame>);
    Effect::new(move |_| {
        let Some(highlight) = store
            .highlight
            .get()
            .filter(|highlight| highlight.view == ViewKey::Posts(view))
        else {
            return;
        };
        let position = visible.with_untracked(|visible| {
            store.doc.with_untracked(|doc| {
                visible
                    .iter()
                    .position(|(_, index)| doc.posts[*index].items.contains(&highlight.id))
            })
        });
        if let Some(position) = position {
            scroll_to(position);
            settle_frame.set_value(Some(Frame::request(move |_| scroll_to(position))));
        } else if store.doc.with_untracked(|doc| {
            doc.posts
                .iter()
                .any(|post| post.items.contains(&highlight.id))
        }) {
            set_filter(view, CategoryFilter::All);
        }
    });
    let new_post_category = move || {
        store
            .doc
            .with_untracked(|doc| match filter.get_untracked() {
                CategoryFilter::All => doc.posts.last().map_or_else(
                    || doc.first_category().id.clone(),
                    |post| post.category.clone(),
                ),
                CategoryFilter::Only(category) => category,
            })
    };

    view! {
        <section class="flex h-full flex-col bg-canvas" on:pointerdown:capture=move |_| store.last_posts_view.set(view)>
            <PaneHeader>
                <h2 class="mr-1 text-sm font-semibold text-fg">{t().posts}</h2>
                <span class="text-xs text-faint tabular-nums">{move || visible.with(Vec::len)}</span>
                <div class="flex-1"></div>
                <Chip label=t().all_categories.to_owned() active=Signal::derive(move || filter.get() == CategoryFilter::All) on_click=move |()| set_filter(view, CategoryFilter::All)>
                    {t().all_categories}
                </Chip>
                <For each=move || store.doc.with(|doc| doc.categories.iter().map(|category| category.id.clone()).collect::<Vec<_>>()) key=Clone::clone let:id>
                    <CategoryChip id=id view=view active=filter />
                </For>
                <IconButton
                    icon=icons::PLUS
                    label=t().new_category
                    on_click=move |()| commit(|doc| {
                        let name = format!("{} {}", t().new_category, doc.categories.len() + 1);
                        ops::add_category(doc, CategoryId(new_id()), name);
                    })
                />
            </PaneHeader>
            <PaneHeader>
                <RangeInput
                    label=t().tile_height
                    min=4.0
                    max=20.0
                    step=0.5
                    value=Signal::derive(move || store.settings.with(|settings| settings.post_tile_height))
                    on_change=move |height| store.settings.update(|settings| settings.post_tile_height = height)
                    class="h-1 cursor-pointer accent-accent w-28"
                />
                <div class="flex-1"></div>
                {move || {
                    if compose.get() {
                        view! {
                            <IconButton
                                icon=icons::ARROW_DOWN_WIDE_NARROW
                                label=t().sort_posts_by_time
                                on_click=move |()| {
                                    let times: HashMap<_, _> = store.library.with_untracked(|library| library.as_ref().map(|library| library.files.iter().map(|file| (file.id.clone(), file.last_modified)).collect()).unwrap_or_default());
                                    commit(move |doc| ops::sort_posts_by_time(doc, &times));
                                }
                            />
                        }
                        .into_any()
                    } else {
                        view! {
                            <Chip
                                label=t().hide_sent_hint.to_owned()
                                active=Signal::derive(move || store.settings.with(|settings| settings.hide_sent))
                                on_click=move |()| store.settings.update(|settings| settings.hide_sent = !settings.hide_sent)
                            >
                                {svg(icons::EYE_OFF, "size-3.5")}
                                " "
                                {t().hide_sent}
                            </Chip>
                        }
                        .into_any()
                    }
                }}
            </PaneHeader>
            <div node_ref=scroller data-scroll="" data-drop="posts" data-category=move || String::from(filter.get()) class="min-h-0 flex-1 overflow-y-auto px-3">
                {move || visible.with(Vec::is_empty).then(|| view! { <p class="px-6 pt-16 text-center text-sm text-faint">{t().empty_posts}</p> })}
                <div class="relative w-full" style:height=move || format!("{}px", layout.with(|layout| layout.2))>
                    <For each=move || window.get() key=Clone::clone let:id>
                        {
                            let wrapper = NodeRef::<Div>::new();
                            let measured = StoredValue::new_local(None::<Measured>);
                            let placed = id.clone();
                            let watched = id.clone();
                            Effect::new(move |_| {
                                let Some(element) = wrapper.get() else { return };
                                let watched = watched.clone();
                                let target = element.clone();
                                let watch = ResizeWatch::new(&element, move |_| {
                                    let height = target.get_bounding_client_rect().height().round();
                                    let previous = sizes.with_untracked(|sizes| sizes.get(&watched).copied()).unwrap_or(ESTIMATED_CARD_PX);
                                    if (previous - height).abs() < 0.5 {
                                        return;
                                    }
                                    let start = visible.with_untracked(|visible| visible.iter().position(|(id, _)| *id == watched)).and_then(|position| layout.with_untracked(|layout| layout.0.get(position).copied()));
                                    let scroll_top = viewport.scroll_top.get_untracked();
                                    sizes.update(|sizes| {
                                        sizes.insert(watched.clone(), height);
                                    });
                                    if start.is_some_and(|start| start < scroll_top)
                                        && let Some(scroller) = scroller.get_untracked()
                                    {
                                        #[expect(clippy::cast_possible_truncation, reason = "scroll offsets are pixels")]
                                        scroller.set_scroll_top(scroller.scroll_top() + (height - previous) as i32);
                                    }
                                });
                                measured.set_value(watch.map(|watch| Measured { _watch: watch }));
                            });
                            let top = move || {
                                let start = visible.with(|visible| visible.iter().position(|(candidate, _)| *candidate == placed)).and_then(|position| layout.with(|layout| layout.0.get(position).copied())).unwrap_or_default();
                                format!("top: {start}px; padding-bottom: {CARD_GAP_PX}px;")
                            };
                            view! {
                                <div node_ref=wrapper class="absolute inset-x-0" style=top>
                                    <PostCard id=id view=view />
                                </div>
                            }
                        }
                    </For>
                </div>
                {move || {
                    compose
                        .get()
                        .then(|| {
                            view! {
                                <button
                                    type="button"
                                    on:click=move |_| {
                                        let category = new_post_category();
                                        commit(move |doc| {
                                            let index = doc.posts.len();
                                            ops::insert_post(doc, ops::create_post(PostId(new_id()), Vec::new(), category), index);
                                        });
                                    }
                                    class="mb-40 flex h-12 w-full items-center justify-center gap-2 rounded-2xl border border-dashed border-line text-sm text-muted transition hover:border-accent hover:text-accent"
                                >
                                    {svg(icons::PLUS, "size-4")}
                                    " "
                                    {t().new_post}
                                </button>
                            }
                        })
                }}
            </div>
        </section>
    }
}
