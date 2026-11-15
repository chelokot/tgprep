use crate::browser::{Listener, document, window};
use crate::dnd::DragOverlay;
use crate::i18n::{fill, t};
use crate::icons::{self, svg};
use crate::library::Library;
use crate::persistence::{pick_folder, watch_saves};
use crate::posts::PostsPane;
use crate::store::{DEFAULT_PANE_WIDTHS, Mode, PostsView, TELEGRAM_LONG_SIDE, clear_selection, redo, save_settings, store, undo};
use crate::ui::{Choice, IconButton, Label, Segmented, Toasts};
use crate::viewer::ViewerLayer;
use crate::welcome::Welcome;
use leptos::html::Div;
use leptos::portal::Portal;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{KeyboardEvent, PointerEvent};

const MIN_PANE: f64 = 0.18;

fn is_typing(target: Option<web_sys::EventTarget>) -> bool {
    target.is_some_and(|target| {
        target.has_type::<web_sys::HtmlTextAreaElement>() || target.has_type::<web_sys::HtmlSelectElement>() || target.dyn_ref::<web_sys::HtmlInputElement>().is_some_and(|input| input.type_() != "range")
    })
}

#[component]
fn TopBar() -> impl IntoView {
    let store = store();
    let folder = move || store.library.with(|library| library.as_ref().map(|library| library.root.name()).unwrap_or_default());
    let stats = move || {
        let placed = store.placements.with(|placements| placements.len());
        let posts = store.doc.with(|doc| doc.posts.len());
        let total = store.library.with(|library| library.as_ref().map_or(0, |library| library.files.len()));
        fill(t().stats, &[("posts", &posts.to_string()), ("placed", &placed.to_string()), ("total", &total.to_string())])
    };
    view! {
        <header class="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-panel px-3">
            <div class="grid size-8 place-items-center rounded-lg bg-accent text-white">{svg(icons::SEND, "size-4 -translate-x-px")}</div>
            <span class="font-semibold text-fg">{t().app_name}</span>
            <button
                type="button"
                title=t().change_folder
                on:click=move |_| pick_folder()
                class="ml-1 flex h-8 items-center gap-1.5 rounded-lg px-2 text-sm text-muted transition hover:bg-raised hover:text-fg"
            >
                {svg(icons::FOLDER_OPEN, "size-4")}
                " "
                {folder}
            </button>
            <span class="hidden text-xs text-faint lg:inline">{stats}</span>
            <div class="flex-1"></div>
            <IconButton icon=icons::UNDO_2 label=t().undo disabled=Signal::derive(move || !store.history.with(|history| history.can_undo())) on_click=move |()| undo() />
            <IconButton icon=icons::REDO_2 label=t().redo disabled=Signal::derive(move || !store.history.with(|history| history.can_redo())) on_click=move |()| redo() />
            <Segmented
                value=Signal::derive(move || store.settings.with(|settings| settings.export_long_side))
                on_change=move |long_side| store.settings.update(|settings| settings.export_long_side = long_side)
                choices=vec![
                    Choice { value: Some(TELEGRAM_LONG_SIDE), label: Label::Text(t().export_telegram), title: t().export_size },
                    Choice { value: None, label: Label::Text(t().export_original), title: t().export_size },
                ]
            />
            <Segmented
                value=store.mode
                on_change=move |mode| store.mode.set(mode)
                choices=vec![
                    Choice { value: Mode::Compose, label: Label::Text(t().compose), title: t().compose_hint },
                    Choice { value: Mode::Send, label: Label::Text(t().send), title: t().send_hint },
                ]
            />
        </header>
    }
}

fn set_resizing(on: bool) {
    if let Some(root) = document().document_element() {
        let _ = if on { root.set_attribute("data-resizing", "") } else { root.remove_attribute("data-resizing") };
    }
}

#[component]
fn Splitter(first: bool, container: NodeRef<Div>) -> impl IntoView {
    let store = store();
    let resizing = StoredValue::new_local(Vec::<Listener>::new());
    let start = move |event: PointerEvent| {
        event.prevent_default();
        let Some(element) = container.get_untracked() else { return };
        let width = element.get_bounding_client_rect().width();
        let origin = f64::from(event.client_x());
        let [left, middle] = store.settings.with_untracked(|settings| settings.pane_widths);
        let target: web_sys::EventTarget = window().into();
        let moving = Listener::new(&target, "pointermove", false, move |event| {
            let Some(event) = event.dyn_ref::<PointerEvent>() else { return };
            let delta = (f64::from(event.client_x()) - origin) / width;
            let next = if first {
                let resized = (left + delta).clamp(MIN_PANE, left + middle - MIN_PANE);
                [resized, left + middle - resized]
            } else {
                [left, (middle + delta).clamp(MIN_PANE, 1.0 - left - MIN_PANE)]
            };
            store.settings.update(|settings| settings.pane_widths = next);
        });
        let ending = Listener::new(&target, "pointerup", false, move |_| {
            set_resizing(false);
            resizing.update_value(Vec::clear);
        });
        set_resizing(true);
        resizing.set_value(vec![moving, ending]);
    };
    view! {
        <div
            role="separator"
            aria-orientation="vertical"
            on:pointerdown=start
            on:dblclick=move |_| store.settings.update(|settings| settings.pane_widths = DEFAULT_PANE_WIDTHS)
            class="group relative z-10 w-px shrink-0 cursor-col-resize bg-line"
        >
            <div class="absolute inset-y-0 -right-1.5 -left-1.5 transition group-hover:bg-accent/30"></div>
        </div>
    }
}

#[component]
fn Workspace() -> impl IntoView {
    let store = store();
    let container = NodeRef::<Div>::new();
    let width = move |index: usize| format!("width: {}%;", store.settings.with(|settings| settings.pane_widths[index]) * 100.0);
    view! {
        <div
            class="flex h-dvh flex-col"
            on:pointerdown:capture=move |_| {
                if store.highlight.with_untracked(Option::is_some) {
                    store.highlight.set(None);
                }
            }
        >
            <TopBar />
            <div node_ref=container class="flex min-h-0 flex-1">
                <div style=move || width(0) class="min-w-0">
                    <PostsPane view=PostsView::A />
                </div>
                <Splitter first=true container=container />
                <div style=move || width(1) class="min-w-0">
                    <PostsPane view=PostsView::B />
                </div>
                <Splitter first=false container=container />
                <div class="min-w-0 flex-1">
                    <Library />
                </div>
            </div>
            <Portal>
                <DragOverlay />
            </Portal>
            <Portal>
                <ViewerLayer />
            </Portal>
            <Toasts />
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let store = store();
    Effect::new(move |_| store.settings.with(save_settings));
    watch_saves();
    let target: web_sys::EventTarget = window().into();
    let shortcuts = StoredValue::new_local(Listener::new(&target, "keydown", false, move |event| {
        let Some(event) = event.dyn_ref::<KeyboardEvent>() else { return };
        if is_typing(event.target()) {
            return;
        }
        let key = event.key().to_lowercase();
        let command = event.ctrl_key() || event.meta_key();
        if command && key == "z" {
            event.prevent_default();
            if event.shift_key() { redo() } else { undo() }
        } else if command && key == "y" {
            event.prevent_default();
            redo();
        } else if event.key() == "Escape" && store.viewer.with_untracked(Option::is_none) {
            clear_selection();
        }
    }));
    shortcuts.with_value(|_| ());
    let open = Memo::new(move |_| store.library.with(Option::is_some));
    move || if open.get() { view! { <Workspace /> }.into_any() } else { view! { <Welcome /> }.into_any() }
}
