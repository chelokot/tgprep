use crate::browser::{Timeout, now, window};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use tgprep_model::doc::{Adjustments, CategoryId, Doc, PhotoId};
use web_sys::{FileSystemDirectoryHandle, FileSystemFileHandle};

const HISTORY_LIMIT: usize = 300;
const COALESCE_MS: f64 = 1200.0;
const TOAST_MS: i32 = 1800;
const SETTINGS_KEY: &str = "tgprep-settings";
pub const TELEGRAM_LONG_SIDE: u32 = 2560;
pub const DEFAULT_PANE_WIDTHS: [f64; 2] = [0.34, 0.34];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostsView {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewKey {
    Library,
    Posts(PostsView),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Compose,
    Send,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub ids: Arc<Vec<PhotoId>>,
    pub id: PhotoId,
    pub origin: ViewKey,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Highlight {
    pub id: PhotoId,
    pub view: ViewKey,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selection {
    pub ids: HashSet<PhotoId>,
    pub anchor: Option<PhotoId>,
}

#[derive(Debug, Default)]
pub struct History {
    past: Vec<Arc<Doc>>,
    future: Vec<Arc<Doc>>,
    last_key: Option<String>,
    last_at: f64,
}

impl History {
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

pub struct PhotoFile {
    pub id: PhotoId,
    pub handle: FileSystemFileHandle,
    pub size: f64,
    pub last_modified: f64,
}

pub struct Library {
    pub root: FileSystemDirectoryHandle,
    pub files: Vec<Rc<PhotoFile>>,
    pub by_id: HashMap<PhotoId, Rc<PhotoFile>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum CategoryFilter {
    All,
    Only(CategoryId),
}

impl From<String> for CategoryFilter {
    fn from(value: String) -> Self {
        if value == "all" {
            Self::All
        } else {
            Self::Only(CategoryId(value))
        }
    }
}

impl From<CategoryFilter> for String {
    fn from(filter: CategoryFilter) -> Self {
        match filter {
            CategoryFilter::All => "all".into(),
            CategoryFilter::Only(id) => id.0,
        }
    }
}

impl CategoryFilter {
    pub fn active(&self, doc: &Doc) -> Self {
        match self {
            Self::Only(id) if doc.category(id).is_some() => self.clone(),
            _ => Self::All,
        }
    }

    pub fn admits(&self, category: &CategoryId) -> bool {
        match self {
            Self::All => true,
            Self::Only(id) => id == category,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryLayout {
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibrarySort {
    Name,
    Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostsFilters {
    #[serde(rename = "posts-a")]
    pub a: CategoryFilter,
    #[serde(rename = "posts-b")]
    pub b: CategoryFilter,
}

impl PostsFilters {
    pub fn get(&self, view: PostsView) -> &CategoryFilter {
        match view {
            PostsView::A => &self.a,
            PostsView::B => &self.b,
        }
    }

    pub fn set(&mut self, view: PostsView, filter: CategoryFilter) {
        match view {
            PostsView::A => self.a = filter,
            PostsView::B => self.b = filter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Settings {
    pub export_long_side: Option<u32>,
    pub library_layout: LibraryLayout,
    pub library_sort: LibrarySort,
    pub library_zoom: f64,
    pub hide_checked: bool,
    pub hide_placed: bool,
    pub library_category: CategoryFilter,
    pub show_archived: bool,
    pub post_tile_height: f64,
    pub hide_sent: bool,
    pub posts_filter: PostsFilters,
    pub pane_widths: [f64; 2],
    pub edit_panel_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            export_long_side: Some(TELEGRAM_LONG_SIDE),
            library_layout: LibraryLayout::Grid,
            library_sort: LibrarySort::Name,
            library_zoom: 9.0,
            hide_checked: false,
            hide_placed: false,
            library_category: CategoryFilter::All,
            show_archived: false,
            post_tile_height: 8.0,
            hide_sent: false,
            posts_filter: PostsFilters {
                a: CategoryFilter::All,
                b: CategoryFilter::All,
            },
            pane_widths: DEFAULT_PANE_WIDTHS,
            edit_panel_open: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedSettings {
    state: Settings,
    version: u8,
}

fn load_settings() -> Settings {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(SETTINGS_KEY).ok().flatten())
        .and_then(|text| serde_json::from_str::<PersistedSettings>(&text).ok())
        .map(|persisted| persisted.state)
        .unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    let persisted = PersistedSettings {
        state: settings.clone(),
        version: 0,
    };
    if let (Some(storage), Ok(text)) = (
        window().local_storage().ok().flatten(),
        serde_json::to_string(&persisted),
    ) {
        let _ = storage.set_item(SETTINGS_KEY, &text);
    }
}

#[derive(Clone, Copy)]
pub struct Store {
    pub doc: RwSignal<Arc<Doc>>,
    pub history: RwSignal<History>,
    pub library: RwSignal<Option<Rc<Library>>, LocalStorage>,
    pub settings: RwSignal<Settings>,
    pub mode: RwSignal<Mode>,
    pub selection: RwSignal<Selection>,
    pub viewer: RwSignal<Option<Viewer>>,
    pub highlight: RwSignal<Option<Highlight>>,
    pub last_posts_view: RwSignal<PostsView>,
    pub copied_adjustments: RwSignal<Option<Adjustments>>,
    pub toast: RwSignal<Option<(String, u64)>>,
    pub placements: Memo<Arc<HashMap<PhotoId, Vec<usize>>>>,
}

fn create_store() -> Store {
    let doc = RwSignal::new(Arc::new(Doc::default()));
    Store {
        doc,
        history: RwSignal::new(History::default()),
        library: RwSignal::new_local(None),
        settings: RwSignal::new(load_settings()),
        mode: RwSignal::new(Mode::Compose),
        selection: RwSignal::new(Selection::default()),
        viewer: RwSignal::new(None),
        highlight: RwSignal::new(None),
        last_posts_view: RwSignal::new(PostsView::A),
        copied_adjustments: RwSignal::new(None),
        toast: RwSignal::new(None),
        placements: Memo::new(move |_| {
            Arc::new(tgprep_model::ops::placements_by_photo(&doc.read().posts))
        }),
    }
}

thread_local! {
    static STORE: Store = create_store();
    static NONCE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TOAST_TIMER: RefCell<Option<Timeout>> = const { RefCell::new(None) };
}

pub fn store() -> Store {
    STORE.with(|store| *store)
}

fn nonce() -> u64 {
    NONCE.with(|nonce| {
        nonce.set(nonce.get() + 1);
        nonce.get()
    })
}

pub fn commit(recipe: impl FnOnce(&mut Doc)) {
    commit_keyed(None, recipe);
}

pub fn commit_keyed(key: Option<String>, recipe: impl FnOnce(&mut Doc)) {
    let store = store();
    let current = store.doc.get_untracked();
    let mut next = (*current).clone();
    recipe(&mut next);
    if next == *current {
        return;
    }
    let time = now();
    store.history.update(|history| {
        let coalesce =
            key.is_some() && key == history.last_key && time - history.last_at < COALESCE_MS;
        if !coalesce {
            if history.past.len() > HISTORY_LIMIT {
                history.past.remove(0);
            }
            history.past.push(current);
        }
        history.future.clear();
        history.last_key = key;
        history.last_at = time;
    });
    store.doc.set(Arc::new(next));
}

pub fn reset_doc(doc: Doc) {
    let store = store();
    store.history.set(History::default());
    store.doc.set(Arc::new(doc));
}

pub fn undo() {
    let store = store();
    let current = store.doc.get_untracked();
    let mut previous = None;
    store.history.update(|history| {
        previous = history.past.pop();
        if previous.is_some() {
            history.future.insert(0, current);
            history.last_key = None;
        }
    });
    if let Some(previous) = previous {
        store.doc.set(previous);
    }
}

pub fn redo() {
    let store = store();
    let current = store.doc.get_untracked();
    let mut next = None;
    store.history.update(|history| {
        if !history.future.is_empty() {
            next = Some(history.future.remove(0));
            history.past.push(current);
            history.last_key = None;
        }
    });
    if let Some(next) = next {
        store.doc.set(next);
    }
}

pub fn open_viewer(ids: Vec<PhotoId>, id: PhotoId, origin: ViewKey) {
    let store = store();
    store.viewer.set(Some(Viewer {
        ids: Arc::new(ids),
        id,
        origin,
    }));
    store.highlight.set(None);
}

pub fn close_viewer() {
    let store = store();
    if let Some(viewer) = store.viewer.get_untracked() {
        store.viewer.set(None);
        store.highlight.set(Some(Highlight {
            id: viewer.id,
            view: viewer.origin,
            nonce: nonce(),
        }));
    }
}

pub fn reveal_photo(id: PhotoId, view: ViewKey) {
    store().highlight.set(Some(Highlight {
        id,
        view,
        nonce: nonce(),
    }));
}

pub fn clear_selection() {
    store().selection.set(Selection::default());
}

pub fn toast(message: String) {
    store().toast.set(Some((message, nonce())));
    TOAST_TIMER
        .with_borrow_mut(|timer| *timer = Some(Timeout::new(TOAST_MS, || store().toast.set(None))));
}
