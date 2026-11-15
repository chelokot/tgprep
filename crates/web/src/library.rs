use crate::browser::rem_px;
use crate::dnd::{ClickModifiers, DragPayload, press_to_drag};
use crate::i18n::{fill, t};
use crate::icons::{self, stroked, svg};
use crate::store::{CategoryFilter, LibraryLayout, LibrarySort, Settings, ViewKey, clear_selection, commit, new_id, open_viewer, reveal_photo, store};
use crate::thumbs::display_thumb;
use crate::tile::{Fit, TileProps, tile};
use crate::ui::{Chip, Choice, IconButton, Label, PaneHeader, RangeInput, Segmented};
use crate::virtual_list::{centered_offset, watch_viewport};
use leptos::html::Div;
use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tgprep_model::doc::{PhotoId, PostId};
use tgprep_model::ops::{self, PhotoDestination};
use web_sys::PointerEvent;

const GAP_PX: f64 = 6.0;
const LIST_ROW_REM: f64 = 3.5;
const OVERSCAN: usize = 3;

fn update(change: impl FnOnce(&mut Settings)) {
    store().settings.update(change);
}

fn select_from(visible: &[PhotoId], id: &PhotoId, modifiers: ClickModifiers) {
    store().selection.update(|selection| {
        let anchor = selection.anchor.as_ref().and_then(|anchor| visible.iter().position(|candidate| candidate == anchor));
        if let (true, Some(anchor), Some(clicked)) = (modifiers.range, anchor, visible.iter().position(|candidate| candidate == id)) {
            selection.ids.extend(visible[anchor.min(clicked)..=anchor.max(clicked)].iter().cloned());
            return;
        }
        if !selection.ids.remove(id) {
            selection.ids.insert(id.clone());
        }
        selection.anchor = Some(id.clone());
    });
}

#[component]
fn CategoryMark(id: PhotoId) -> impl IntoView {
    let mark = Memo::new(move |_| {
        store().doc.with(|doc| {
            let current = doc.photo_category(&id);
            (current != &doc.first_category().id).then(|| doc.category(current).map(|category| (category.name.clone(), category.color.clone()))).flatten()
        })
    });
    move || mark.get().map(|(name, color)| view! { <span title=name style:background-color=color class="size-3 shrink-0 rounded-full ring-2 ring-white/90"></span> })
}

#[component]
fn PlacementBadges(id: PhotoId) -> impl IntoView {
    let store = store();
    let badges = Memo::new({
        let id = id.clone();
        move |_| {
            let placements = store.placements.get();
            store.doc.with(|doc| {
                placements
                    .get(&id)
                    .map(|indexes| indexes.iter().map(|index| (*index, doc.posts.get(*index).and_then(|post| doc.category(&post.category)).map(|category| category.color.clone()))).collect::<Vec<_>>())
                    .unwrap_or_default()
            })
        }
    });
    move || {
        let badges = badges.get();
        (!badges.is_empty()).then(|| {
            let id = id.clone();
            view! {
                <div class="flex gap-1">
                    {badges
                        .into_iter()
                        .map(|(index, color)| {
                            let id = id.clone();
                            view! {
                                <button
                                    type="button"
                                    title=t().show_in_posts
                                    on:pointerdown=|event: PointerEvent| event.stop_propagation()
                                    on:click=move |event| {
                                        event.stop_propagation();
                                        reveal_photo(id.clone(), ViewKey::Posts(store.last_posts_view.get_untracked()));
                                    }
                                    style:background-color=color
                                    class="rounded-md px-1.5 py-0.5 text-[0.6875rem] leading-none font-semibold text-white shadow hover:brightness-110"
                                >
                                    "#"{index + 1}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            }
        })
    }
}

fn local_date(milliseconds: f64) -> String {
    let date = js_sys::Date::new(&milliseconds.into());
    js_sys::Reflect::get(&date, &"toLocaleDateString".into())
        .ok()
        .and_then(|method| js_sys::Function::from(method).call0(&date).ok())
        .and_then(|text| text.as_string())
        .unwrap_or_default()
}

#[component]
fn LibraryTile(id: PhotoId, visible: Memo<Arc<Vec<PhotoId>>>, layout: LibraryLayout) -> impl IntoView {
    let store = store();
    let thumb = display_thumb(id.clone());
    let pressed = id.clone();
    let on_pointer_down = move |event: PointerEvent| {
        let dragged = pressed.clone();
        let clicked = pressed.clone();
        press_to_drag(
            &event,
            move || {
                let ids = store.selection.with_untracked(|selection| {
                    if selection.ids.contains(&dragged) {
                        visible.with_untracked(|visible| visible.iter().filter(|candidate| selection.ids.contains(candidate)).cloned().collect())
                    } else {
                        vec![dragged]
                    }
                });
                DragPayload::Photos { ids, from_post: None }
            },
            move |modifiers| {
                let visible = visible.get_untracked();
                if modifiers.range || modifiers.toggle {
                    select_from(&visible, &clicked, modifiers);
                } else {
                    open_viewer(visible.to_vec(), clicked, ViewKey::Library);
                }
            },
        );
    };
    let props = |fit, actions: bool, class| TileProps { id: id.clone(), view: ViewKey::Library, thumb, fit, actions: Signal::stored(actions), class, style: None };
    match layout {
        LibraryLayout::Grid => tile(
            props(Fit::Contain, true, "size-full"),
            on_pointer_down,
            view! {
                <div class="absolute bottom-1.5 left-1.5 flex items-center gap-1">
                    <CategoryMark id=id.clone() />
                    <PlacementBadges id=id.clone() />
                </div>
            },
        )
        .into_any(),
        LibraryLayout::List => {
            let file = store.library.with_untracked(|library| library.as_ref().and_then(|library| library.by_id.get(&id).cloned()));
            let details = file.map(|file| (format!("{:.1}", file.size / 1_048_576.0), local_date(file.last_modified))).unwrap_or_default();
            view! {
                <div class="flex h-full items-center gap-3 rounded-lg px-1 hover:bg-raised/60">
                    {tile(props(Fit::Cover, false, "aspect-square h-[calc(100%-0.5rem)]"), on_pointer_down, ())}
                    <div class="min-w-0 flex-1">
                        <div class="truncate text-sm text-fg">{id.0.clone()}</div>
                        <div class="text-xs text-faint">{details.0}" MB · "{details.1}</div>
                    </div>
                    <CategoryMark id=id.clone() />
                    <PlacementBadges id=id.clone() />
                </div>
            }
            .into_any()
        }
    }
}

#[component]
pub fn Library() -> impl IntoView {
    let store = store();
    let settings = store.settings;
    let category = Memo::new(move |_| {
        let filter = settings.with(|settings| settings.library_category.clone());
        store.doc.with(|doc| filter.active(doc))
    });
    let visible: Memo<Arc<Vec<PhotoId>>> = Memo::new(move |_| {
        let (hide_checked, hide_placed, show_archived, sort) = settings.with(|settings| (settings.hide_checked, settings.hide_placed, settings.show_archived, settings.library_sort));
        let category = category.get();
        let placements = store.placements.get();
        let library = store.library.get();
        let Some(library) = library else { return Arc::new(Vec::new()) };
        store.doc.with(|doc| {
            let mut files: Vec<_> = library
                .files
                .iter()
                .filter(|file| {
                    let meta = doc.meta(&file.id);
                    meta.archived == show_archived && !(hide_checked && meta.checked) && category.admits(doc.photo_category(&file.id)) && !(hide_placed && placements.contains_key(&file.id))
                })
                .collect();
            if sort == LibrarySort::Date {
                files.sort_by(|a, b| a.last_modified.total_cmp(&b.last_modified));
            }
            Arc::new(files.into_iter().map(|file| file.id.clone()).collect())
        })
    });
    let total = Memo::new(move |_| store.library.with(|library| library.as_ref().map_or(0, |library| library.files.len())));
    let scroller = NodeRef::<Div>::new();
    let viewport = watch_viewport(scroller);
    let rem = rem_px();
    let layout = Memo::new(move |_| {
        let (layout, zoom) = settings.with(|settings| (settings.library_layout, settings.library_zoom));
        let width = viewport.width.get();
        let grid = layout == LibraryLayout::Grid;
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a small column count")]
        let columns = if grid { (((width + GAP_PX) / (zoom * rem + GAP_PX)).floor() as usize).max(1) } else { 1 };
        #[expect(clippy::cast_precision_loss, reason = "a small column count")]
        let cell = if grid { (width - GAP_PX * (columns as f64 - 1.0)) / columns as f64 } else { LIST_ROW_REM * rem };
        (layout, columns, cell, if grid { cell + GAP_PX } else { cell })
    });
    let kind = Memo::new(move |_| layout.get().0);
    let rows = Memo::new(move |_| visible.with(|visible| visible.len()).div_ceil(layout.get().1));
    #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
    let total_height = move || GAP_PX + rows.get() as f64 * layout.get().3 + GAP_PX * 4.0;
    let window = Memo::new(move |_| {
        let (_, columns, _, row_size) = layout.get();
        let count = rows.get();
        if count == 0 || row_size <= 0.0 {
            return Vec::new();
        }
        let top = viewport.scroll_top.get() - GAP_PX;
        let bottom = top + viewport.height.get();
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "row indexes")]
        let (first, last) = (((top / row_size).floor().max(0.0) as usize).min(count - 1), ((bottom / row_size).ceil().max(1.0) as usize - 1).min(count - 1));
        let range = first.saturating_sub(OVERSCAN) * columns..((last + OVERSCAN + 1).min(count) * columns);
        visible.with(|visible| visible.iter().enumerate().skip(range.start).take(range.len()).map(|(index, id)| (id.clone(), index)).collect::<Vec<_>>())
    });
    let positions: Memo<Arc<HashMap<PhotoId, usize>>> = Memo::new(move |_| Arc::new(visible.with(|visible| visible.iter().enumerate().map(|(index, id)| (id.clone(), index)).collect())));

    Effect::new(move |_| {
        let Some(highlight) = store.highlight.get().filter(|highlight| highlight.view == ViewKey::Library) else { return };
        let Some(index) = positions.with(|positions| positions.get(&highlight.id).copied()) else { return };
        let (_, columns, _, row_size) = layout.get_untracked();
        if let Some(element) = scroller.get_untracked() {
            #[expect(clippy::cast_precision_loss, reason = "row indexes")]
            let start = GAP_PX + (index / columns) as f64 * row_size;
            let offset = centered_offset(start, layout.get_untracked().2, viewport.height.get_untracked(), total_height());
            #[expect(clippy::cast_possible_truncation, reason = "scroll offsets are pixels")]
            element.set_scroll_top(offset as i32);
        }
    });

    let selected_ids = move || {
        store.selection.with_untracked(|selection| visible.with_untracked(|visible| visible.iter().filter(|id| selection.ids.contains(id)).cloned().collect::<Vec<_>>()))
    };
    let selection_size = Memo::new(move |_| store.selection.with(|selection| selection.ids.len()));
    let flag = move |read: fn(&Settings) -> bool| Signal::derive(move || settings.with(read));

    view! {
        <section class="flex h-full flex-col bg-panel">
            <PaneHeader>
                <h2 class="mr-1 text-sm font-semibold text-fg">{move || if settings.with(|settings| settings.show_archived) { t().archive } else { t().library }}</h2>
                <span class="text-xs text-faint tabular-nums">
                    {move || visible.with(|visible| visible.len())}
                    {move || {
                        let (shown, total) = (visible.with(|visible| visible.len()), total.get());
                        (shown != total).then(|| format!(" / {total}"))
                    }}
                </span>
                <div class="flex-1"></div>
                <Chip label=t().hide_checked_hint active=flag(|settings| settings.hide_checked) on_click=move |()| update(|settings| settings.hide_checked = !settings.hide_checked)>
                    {stroked(icons::CHECK, "size-3.5", "3")}
                    " "
                    {t().hide_checked}
                </Chip>
                <Chip label=t().hide_placed_hint active=flag(|settings| settings.hide_placed) on_click=move |()| update(|settings| settings.hide_placed = !settings.hide_placed)>
                    {svg(icons::ROWS_3, "size-3.5")}
                    " "
                    {t().hide_placed}
                </Chip>
                <Chip label=t().show_archived_hint active=flag(|settings| settings.show_archived) on_click=move |()| update(|settings| settings.show_archived = !settings.show_archived)>
                    {svg(icons::ARCHIVE, "size-3.5")}
                </Chip>
            </PaneHeader>
            <PaneHeader>
                <Segmented
                    value=Signal::derive(move || settings.with(|settings| settings.library_layout))
                    on_change=move |layout| update(|settings| settings.library_layout = layout)
                    choices=vec![
                        Choice { value: LibraryLayout::Grid, label: Label::Icon(icons::GRID_2X2), title: t().grid },
                        Choice { value: LibraryLayout::List, label: Label::Icon(icons::LIST), title: t().list },
                    ]
                />
                {move || {
                    (kind.get() == LibraryLayout::Grid)
                        .then(|| {
                            view! {
                                <RangeInput
                                    label=t().zoom
                                    min=4.0
                                    max=24.0
                                    step=0.5
                                    value=Signal::derive(move || settings.with(|settings| settings.library_zoom))
                                    on_change=move |zoom| update(|settings| settings.library_zoom = zoom)
                                    class="h-1 cursor-pointer accent-accent w-16"
                                />
                            }
                        })
                }}
                <div class="flex-1"></div>
                <Chip label=t().all_categories.to_owned() active=Signal::derive(move || category.get() == CategoryFilter::All) on_click=move |()| update(|settings| settings.library_category = CategoryFilter::All)>
                    {t().all_categories}
                </Chip>
                <For each=move || store.doc.with(|doc| doc.categories.clone()) key=|category| (category.id.clone(), category.name.clone(), category.color.clone()) let:option>
                    {
                        let id = option.id.clone();
                        let chosen = option.id.clone();
                        view! {
                            <Chip
                                label=option.name.clone()
                                color=Some(option.color.clone())
                                active=Signal::derive(move || category.get() == CategoryFilter::Only(id.clone()))
                                on_click=move |()| {
                                    let chosen = chosen.clone();
                                    update(|settings| settings.library_category = CategoryFilter::Only(chosen));
                                }
                            >
                                {option.name.clone()}
                            </Chip>
                        }
                    }
                </For>
                <Segmented
                    value=Signal::derive(move || settings.with(|settings| settings.library_sort))
                    on_change=move |sort| update(|settings| settings.library_sort = sort)
                    choices=vec![
                        Choice { value: LibrarySort::Name, label: Label::Icon(icons::ARROW_DOWN_AZ), title: t().sort_name_hint },
                        Choice { value: LibrarySort::Date, label: Label::Icon(icons::CALENDAR_CLOCK), title: t().sort_date_hint },
                    ]
                />
            </PaneHeader>
            {move || {
                (selection_size.get() > 0)
                    .then(|| {
                        view! {
                            <div class="flex h-11 shrink-0 items-center gap-1 border-b border-line bg-accent/10 px-3">
                                <span class="mr-auto text-xs font-semibold text-accent">{move || fill(t().selected, &[("count", &selection_size.get().to_string())])}</span>
                                <IconButton
                                    icon=icons::SQUARE_PLUS
                                    label=t().new_post_from_selection
                                    on_click=move |()| {
                                        let ids = selected_ids();
                                        let filter = store.settings.with_untracked(|settings| settings.posts_filter.get(store.last_posts_view.get_untracked()).clone());
                                        commit(move |doc| {
                                            let category = match filter {
                                                CategoryFilter::All => doc.first_category().id.clone(),
                                                CategoryFilter::Only(category) => category,
                                            };
                                            let index = doc.posts.len();
                                            ops::place_photos(doc, &ids, PhotoDestination::Gap { index, category }, None, || PostId(new_id()));
                                        });
                                        clear_selection();
                                    }
                                />
                                <IconButton
                                    icon=icons::CHECK
                                    label=t().check
                                    on_click=move |()| {
                                        let ids = selected_ids();
                                        commit(move |doc| ops::update_meta(doc, &ids, |meta| meta.checked = true));
                                    }
                                />
                                <IconButton
                                    icon=icons::ARCHIVE
                                    label=Signal::derive(move || if settings.with(|settings| settings.show_archived) { t().restore } else { t().archive })
                                    on_click=move |()| {
                                        let ids = selected_ids();
                                        let archived = !settings.with_untracked(|settings| settings.show_archived);
                                        commit(move |doc| ops::set_archived(doc, &ids, archived));
                                        clear_selection();
                                    }
                                />
                                <IconButton icon=icons::X label=t().clear_selection on_click=move |()| clear_selection() />
                            </div>
                        }
                    })
            }}
            <div node_ref=scroller data-scroll="" data-drop="library" class="min-h-0 flex-1 overflow-y-auto px-2">
                {move || {
                    if visible.with(|visible| visible.is_empty()) {
                        view! { <p class="px-6 py-16 text-center text-sm text-faint">{move || if total.get() == 0 { t().no_photos } else { t().nothing_matches }}</p> }.into_any()
                    } else {
                        view! {
                            <div class="relative w-full" style:height=move || format!("{}px", total_height())>
                                <For each=move || window.get() key=|(id, _)| id.clone() let:((id, _))>
                                    {
                                        let place = id.clone();
                                        let style = move || {
                                            let (layout, columns, cell, row_size) = layout.get();
                                            let index = positions.with(|positions| positions.get(&place).copied().unwrap_or_default());
                                            #[expect(clippy::cast_precision_loss, reason = "tile positions")]
                                            let (top, left) = (GAP_PX + (index / columns) as f64 * row_size, (index % columns) as f64 * (cell + GAP_PX));
                                            match layout {
                                                LibraryLayout::Grid => format!("position: absolute; top: {top}px; left: {left}px; width: {cell}px; height: {cell}px;"),
                                                LibraryLayout::List => format!("position: absolute; top: {top}px; left: 0px; width: 100%; height: {cell}px;"),
                                            }
                                        };
                                        view! {
                                            <div style=style>
                                                {move || view! { <LibraryTile id=id.clone() visible=visible layout=kind.get() /> }}
                                            </div>
                                        }
                                    }
                                </For>
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
        </section>
    }
}
