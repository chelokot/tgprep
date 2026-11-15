use crate::browser::{Timeout, ensure_write_permission, pick_directory, settle, wait};
use crate::idb::KeyValueStore;
use crate::store::{Library, PhotoFile, reset_doc, store};
use crate::thumbs::reset_thumbs;
use js_sys::{Array, Function, Object, Promise, Reflect};
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tgprep_model::doc::{Doc, PhotoId};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    File, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
    FileSystemHandle, FileSystemHandleKind, FileSystemWritableFileStream,
};

const STATE_FILE: &str = ".tgprep.json";
pub const EXPORT_DIR: &str = "tgprep-export";
const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "avif", "gif", "bmp"];
const RECENT_LIMIT: usize = 6;
const SAVE_DELAY_MS: i32 = 400;

thread_local! {
    static HANDLES: KeyValueStore = KeyValueStore::new("tgprep", "handles");
    static SAVER: RefCell<Option<Saver>> = const { RefCell::new(None) };
}

pub struct RecentFolder {
    pub name: String,
    pub handle: FileSystemDirectoryHandle,
}

fn handles() -> KeyValueStore {
    HANDLES.with(Clone::clone)
}

pub async fn load_recent_folders() -> Vec<RecentFolder> {
    let Ok(stored) = handles()
        .get("recent")
        .await
        .and_then(JsCast::dyn_into::<Array>)
    else {
        return Vec::new();
    };
    stored
        .iter()
        .filter_map(|entry| {
            let name = Reflect::get(&entry, &JsValue::from_str("name"))
                .ok()?
                .as_string()?;
            let handle = Reflect::get(&entry, &JsValue::from_str("handle"))
                .ok()?
                .dyn_into()
                .ok()?;
            Some(RecentFolder { name, handle })
        })
        .collect()
}

fn recent_entry(name: &str, handle: &FileSystemDirectoryHandle) -> JsValue {
    let entry = Object::new();
    let _ = Reflect::set(&entry, &JsValue::from_str("name"), &JsValue::from_str(name));
    let _ = Reflect::set(&entry, &JsValue::from_str("handle"), handle);
    entry.into()
}

async fn remember_folder(handle: &FileSystemDirectoryHandle) -> Result<(), JsValue> {
    let next = Array::of1(&recent_entry(&handle.name(), handle));
    for folder in load_recent_folders().await {
        if next.length() as usize == RECENT_LIMIT {
            break;
        }
        if settle(folder.handle.is_same_entry(handle)).await?.as_bool() != Some(true) {
            next.push(&recent_entry(&folder.name, &folder.handle));
        }
    }
    handles().set("recent", &next).await
}

fn is_image(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        IMAGE_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

async fn entries(directory: &FileSystemDirectoryHandle) -> Result<Vec<FileSystemHandle>, JsValue> {
    let iterator = directory.values();
    let mut entries = Vec::new();
    loop {
        let step = settle(iterator.next()?).await?;
        if Reflect::get(&step, &JsValue::from_str("done"))?.as_bool() == Some(true) {
            return Ok(entries);
        }
        entries.push(Reflect::get(&step, &JsValue::from_str("value"))?.dyn_into()?);
    }
}

async fn scan(root: &FileSystemDirectoryHandle) -> Result<Vec<PhotoFile>, JsValue> {
    let mut pending = vec![(root.clone(), String::new())];
    let mut found: Vec<(PhotoId, FileSystemFileHandle)> = Vec::new();
    while let Some((directory, prefix)) = pending.pop() {
        for entry in entries(&directory).await? {
            let name = entry.name();
            if name.starts_with('.') || name == EXPORT_DIR {
                continue;
            }
            if entry.kind() == FileSystemHandleKind::Directory {
                pending.push((entry.unchecked_into(), format!("{prefix}{name}/")));
            } else if is_image(&name) {
                found.push((PhotoId(format!("{prefix}{name}")), entry.unchecked_into()));
            }
        }
    }
    let files = Array::new();
    for (_, handle) in &found {
        files.push(&handle.get_file());
    }
    let files: Array = wait(Promise::all(&files)).await?;
    Ok(found
        .into_iter()
        .zip(files.iter())
        .filter_map(|((id, handle), file)| {
            let file: File = file.dyn_into().ok()?;
            Some(PhotoFile {
                id,
                handle,
                size: file.size(),
                last_modified: file.last_modified(),
            })
        })
        .collect())
}

fn sort_naturally(files: &mut [PhotoFile]) {
    let options = Object::new();
    let _ = Reflect::set(&options, &JsValue::from_str("numeric"), &JsValue::TRUE);
    let _ = Reflect::set(
        &options,
        &JsValue::from_str("sensitivity"),
        &JsValue::from_str("base"),
    );
    let compare: Function = js_sys::Intl::Collator::new(&Array::new(), &options).compare();
    files.sort_by(|a, b| {
        let order = compare
            .call2(
                &JsValue::NULL,
                &JsValue::from_str(a.id.as_str()),
                &JsValue::from_str(b.id.as_str()),
            )
            .ok()
            .and_then(|order| order.as_f64())
            .unwrap_or(0.0);
        order.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
    });
}

async fn read_doc(root: &FileSystemDirectoryHandle) -> Doc {
    let Ok(handle) = wait::<FileSystemFileHandle>(root.get_file_handle(STATE_FILE)).await else {
        return Doc::default();
    };
    let Ok(file) = wait::<File>(handle.get_file()).await else {
        return Doc::default();
    };
    let Ok(text) = settle(file.text()).await else {
        return Doc::default();
    };
    text.as_string()
        .and_then(|text| Doc::parse(&text).ok())
        .unwrap_or_default()
}

struct Saver {
    root: FileSystemDirectoryHandle,
    saved: Arc<Doc>,
    timer: Option<Timeout>,
    writing: bool,
    queued: Option<Arc<Doc>>,
}

async fn write_state(root: &FileSystemDirectoryHandle, doc: &Doc) -> Result<(), JsValue> {
    let text = doc
        .to_json()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let options = FileSystemGetFileOptions::new();
    options.set_create(true);
    let handle: FileSystemFileHandle =
        wait(root.get_file_handle_with_options(STATE_FILE, &options)).await?;
    let writable: FileSystemWritableFileStream = wait(handle.create_writable()).await?;
    settle(writable.write_with_str(&text)?).await?;
    settle(writable.close()).await.map(drop)
}

fn drain_writes() {
    let next = SAVER.with_borrow_mut(|saver| {
        let saver = saver.as_mut()?;
        let doc = saver.queued.take()?;
        saver.writing = true;
        Some((saver.root.clone(), doc))
    });
    let Some((root, doc)) = next else {
        SAVER.with_borrow_mut(|saver| {
            if let Some(saver) = saver.as_mut() {
                saver.writing = false;
            }
        });
        return;
    };
    spawn_local(async move {
        let _ = write_state(&root, &doc).await;
        let current =
            SAVER.with_borrow(|saver| saver.as_ref().is_some_and(|saver| saver.root == root));
        if current {
            drain_writes();
        }
    });
}

fn flush(doc: Arc<Doc>) {
    let idle = SAVER.with_borrow_mut(|saver| {
        let saver = saver.as_mut()?;
        saver.saved = doc.clone();
        saver.queued = Some(doc);
        Some(!saver.writing)
    });
    if idle == Some(true) {
        drain_writes();
    }
}

pub fn watch_saves() {
    Effect::new(move |_| {
        let doc = store().doc.get();
        SAVER.with_borrow_mut(|saver| {
            if let Some(saver) = saver
                .as_mut()
                .filter(|saver| !Arc::ptr_eq(&saver.saved, &doc))
            {
                saver.timer = Some(Timeout::new(SAVE_DELAY_MS, move || flush(doc)));
            }
        });
    });
}

pub async fn open_folder(root: FileSystemDirectoryHandle) -> Result<bool, JsValue> {
    if !ensure_write_permission(&root).await? {
        return Ok(false);
    }
    SAVER.with_borrow_mut(|saver| *saver = None);
    let doc = read_doc(&root).await;
    let mut files = scan(&root).await?;
    sort_naturally(&mut files);
    reset_thumbs();
    let files: Vec<Rc<PhotoFile>> = files.into_iter().map(Rc::new).collect();
    let by_id: HashMap<PhotoId, Rc<PhotoFile>> = files
        .iter()
        .map(|file| (file.id.clone(), file.clone()))
        .collect();
    store().library.set(Some(Rc::new(Library {
        root: root.clone(),
        files,
        by_id,
    })));
    reset_doc(doc);
    let saved = store().doc.get_untracked();
    SAVER.with_borrow_mut(|saver| {
        *saver = Some(Saver {
            root: root.clone(),
            saved,
            timer: None,
            writing: false,
            queued: None,
        });
    });
    remember_folder(&root).await?;
    Ok(true)
}

pub fn pick_folder() {
    spawn_local(async {
        if let Ok(root) = pick_directory().await {
            let _ = open_folder(root).await;
        }
    });
}

pub fn reopen_folder(root: FileSystemDirectoryHandle) {
    spawn_local(async move {
        let _ = open_folder(root).await;
    });
}
