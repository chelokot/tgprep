use crate::browser::{settle, window};
use js_sys::{Function, Promise};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransactionMode};

#[derive(Clone)]
pub struct KeyValueStore {
    database: &'static str,
    store: &'static str,
    opened: Rc<RefCell<Option<Promise>>>,
}

fn completion(request: &IdbRequest) -> Promise {
    let request = request.clone();
    Promise::new(&mut |resolve: Function, reject: Function| {
        let succeeded = request.clone();
        let on_success = Closure::once_into_js(move || {
            let _ = resolve.call1(&JsValue::NULL, &succeeded.result().unwrap_or(JsValue::UNDEFINED));
        });
        let failed = request.clone();
        let on_error = Closure::once_into_js(move || {
            let _ = reject.call1(&JsValue::NULL, &failed.error().ok().flatten().map_or(JsValue::UNDEFINED, JsValue::from));
        });
        request.set_onsuccess(Some(on_success.unchecked_ref()));
        request.set_onerror(Some(on_error.unchecked_ref()));
    })
}

impl KeyValueStore {
    pub fn new(database: &'static str, store: &'static str) -> Self {
        Self { database, store, opened: Rc::new(RefCell::new(None)) }
    }

    fn open(&self) -> Result<Promise, JsValue> {
        if let Some(opened) = self.opened.borrow().as_ref() {
            return Ok(opened.clone());
        }
        let factory = window().indexed_db()?.ok_or_else(|| JsValue::from_str("IndexedDB is unavailable"))?;
        let request: IdbOpenDbRequest = factory.open(self.database)?;
        let store = self.store;
        let upgrading = request.clone();
        let on_upgrade = Closure::once_into_js(move || {
            if let Ok(database) = upgrading.result().and_then(JsCast::dyn_into::<IdbDatabase>) {
                let _ = database.create_object_store(store);
            }
        });
        request.set_onupgradeneeded(Some(on_upgrade.unchecked_ref()));
        let opened = completion(&request);
        *self.opened.borrow_mut() = Some(opened.clone());
        Ok(opened)
    }

    async fn request(&self, mode: IdbTransactionMode, operation: impl FnOnce(&web_sys::IdbObjectStore) -> Result<IdbRequest, JsValue>) -> Result<JsValue, JsValue> {
        let database: IdbDatabase = settle(self.open()?).await?.dyn_into()?;
        let store = database.transaction_with_str_and_mode(self.store, mode)?.object_store(self.store)?;
        settle(completion(&operation(&store)?)).await
    }

    pub async fn get(&self, key: &str) -> Result<JsValue, JsValue> {
        self.request(IdbTransactionMode::Readonly, |store| store.get(&JsValue::from_str(key))).await
    }

    pub async fn set(&self, key: &str, value: &JsValue) -> Result<(), JsValue> {
        self.request(IdbTransactionMode::Readwrite, |store| store.put_with_key(value, &JsValue::from_str(key))).await.map(drop)
    }
}
