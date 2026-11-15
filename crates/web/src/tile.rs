use crate::i18n::t;
use crate::icons::{self, stroked, svg};
use crate::store::{ViewKey, commit, store};
use crate::thumbs::DisplayThumb;
use crate::ui::check_icon;
use leptos::prelude::*;
use tgprep_model::doc::PhotoId;
use tgprep_model::ops;
use web_sys::PointerEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Contain,
    Cover,
}

#[component]
pub fn CornerButton(
    #[prop(into)] label: Signal<&'static str>,
    #[prop(into)] on_click: Callback<()>,
    #[prop(into)] class: Signal<&'static str>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            type="button"
            title=move || label.get()
            aria-label=move || label.get()
            on:pointerdown=|event: PointerEvent| event.stop_propagation()
            on:click=move |event| {
                event.stop_propagation();
                on_click.run(());
            }
            class=move || format!("absolute grid size-6 place-items-center rounded-full transition {}", class.get())
        >
            {children()}
        </button>
    }
}

pub struct TileProps {
    pub id: PhotoId,
    pub view: ViewKey,
    pub thumb: Memo<Option<DisplayThumb>>,
    pub fit: Fit,
    pub actions: Signal<bool>,
    pub class: &'static str,
    pub style: Option<Signal<String>>,
}

#[allow(clippy::too_many_lines)]
pub fn tile(
    props: TileProps,
    on_pointer_down: impl Fn(PointerEvent) + 'static,
    children: impl IntoView + 'static,
) -> impl IntoView {
    let TileProps {
        id,
        view,
        thumb,
        fit,
        actions,
        class,
        style,
    } = props;
    let store = store();
    let meta = Memo::new({
        let id = id.clone();
        move |_| {
            store.doc.with(|doc| {
                let meta = doc.meta(&id);
                (meta.checked, meta.archived)
            })
        }
    });
    let selected = Memo::new({
        let id = id.clone();
        move |_| {
            store
                .selection
                .with(|selection| selection.ids.contains(&id))
        }
    });
    let highlighted = Memo::new({
        let id = id.clone();
        move |_| {
            store.highlight.with(|highlight| {
                highlight
                    .as_ref()
                    .is_some_and(|highlight| highlight.id == id && highlight.view == view)
            })
        }
    });
    let state = move || {
        if selected.get() {
            "ring-2 ring-accent ring-offset-2 ring-offset-panel"
        } else if highlighted.get() {
            "animate-glow ring-2 ring-accent/70"
        } else {
            ""
        }
    };
    let image_class = match fit {
        Fit::Contain => "size-full object-contain",
        Fit::Cover => "size-full object-cover",
    };
    let check_id = id.clone();
    let archive_id = id.clone();
    view! {
        <div
            data-photo=id.0.clone()
            on:pointerdown=on_pointer_down
            style=move || style.map(|style| style.get())
            class=move || format!("group/tile relative cursor-pointer select-none overflow-hidden rounded-lg bg-raised {} {class}", state())
        >
            {move || match thumb.get() {
                Some(DisplayThumb { url: Some(url), .. }) => {
                    view! { <img src=url alt="" draggable="false" decoding="async" class=image_class /> }.into_any()
                }
                Some(DisplayThumb { url: None, .. }) => {
                    view! { <div class="grid size-full place-items-center text-faint ">{svg(icons::IMAGE_OFF, "size-5")}</div> }.into_any()
                }
                None => view! { <div class="grid size-full place-items-center text-faint animate-pulse"></div> }.into_any(),
            }}
            {move || {
                thumb.get().is_some_and(|thumb| thumb.edited).then(|| {
                    view! {
                        <div title=t().edited class="absolute right-1.5 bottom-1.5 grid size-5 place-items-center rounded-full bg-black/55 text-white backdrop-blur">
                            {stroked(icons::SLIDERS_HORIZONTAL, "size-3", "2.5")}
                        </div>
                    }
                })
            }}
            {children}
            <CornerButton
                label=Signal::derive(move || if meta.get().0 { t().uncheck } else { t().check })
                on_click=move |()| {
                    let checked = !meta.get_untracked().0;
                    let id = check_id.clone();
                    commit(move |doc| ops::update_meta(doc, &[id], |meta| meta.checked = checked));
                }
                class=Signal::derive(move || {
                    if meta.get().0 {
                        "top-1.5 right-1.5 bg-ok text-white shadow"
                    } else {
                        "top-1.5 right-1.5 border-2 border-white/90 bg-black/20 text-transparent opacity-0 shadow group-hover/tile:opacity-100 hover:bg-black/40"
                    }
                })
            >
                {check_icon()}
            </CornerButton>
            {move || {
                let archive_id = archive_id.clone();
                actions.get().then(move || {
                    view! {
                        <CornerButton
                            label=Signal::derive(move || if meta.get().1 { t().restore } else { t().archive })
                            on_click=move |()| {
                                let archived = !meta.get_untracked().1;
                                let id = archive_id.clone();
                                commit(move |doc| ops::set_archived(doc, &[id], archived));
                            }
                            class="top-1.5 left-1.5 bg-black/55 text-white opacity-0 backdrop-blur group-hover/tile:opacity-100 hover:bg-danger"
                        >
                            {move || svg(if meta.get().1 { icons::ARCHIVE_RESTORE } else { icons::ARCHIVE }, "size-3.5")}
                        </CornerButton>
                    }
                })
            }}
        </div>
    }
}
