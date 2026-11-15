mod app;
mod browser;
mod dnd;
mod export;
mod i18n;
mod icons;
mod idb;
mod library;
mod persistence;
mod posts;
mod renderer;
mod store;
mod thumbs;
mod tile;
mod ui;
mod viewer;
mod virtual_list;
mod welcome;

use wasm_bindgen::JsCast;

fn main() {
    store::store();
    thumbs::start_workers();
    viewer::start_decoder();
    if let Some(root) = browser::document().get_element_by_id("root") {
        leptos::mount::mount_to(root.unchecked_into(), app::App).forget();
    }
}
