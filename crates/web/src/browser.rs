use js_sys::{Array, Object, Promise, Reflect};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, FileSystemDirectoryHandle, FileSystemHandle, Window};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = window, js_name = showDirectoryPicker)]
    fn show_directory_picker(options: &JsValue) -> Result<Promise, JsValue>;

    type PermissionHandle;

    #[wasm_bindgen(method, js_name = queryPermission)]
    fn query_permission(this: &PermissionHandle, descriptor: &JsValue) -> Promise;

    #[wasm_bindgen(method, js_name = requestPermission)]
    fn request_permission(this: &PermissionHandle, descriptor: &JsValue) -> Promise;

    #[wasm_bindgen(js_name = ClipboardItem)]
    type ClipboardItem;

    #[wasm_bindgen(constructor, js_class = "ClipboardItem")]
    fn new(items: &Object) -> ClipboardItem;

    type Clipboard;

    #[wasm_bindgen(method)]
    fn write(this: &Clipboard, items: &Array) -> Promise;

    #[wasm_bindgen(method, js_name = writeText)]
    fn write_text(this: &Clipboard, text: &str) -> Promise;
}

pub fn window() -> Window {
    web_sys::window().unwrap_or_else(|| wasm_bindgen::throw_str("no window"))
}

pub fn document() -> Document {
    window()
        .document()
        .unwrap_or_else(|| wasm_bindgen::throw_str("no document"))
}

pub fn now() -> f64 {
    window()
        .performance()
        .map_or(0.0, |performance| performance.now())
}

pub fn rem_px() -> f64 {
    document()
        .document_element()
        .and_then(|root| window().get_computed_style(&root).ok().flatten())
        .and_then(|style| style.get_property_value("font-size").ok())
        .and_then(|size| size.trim_end_matches("px").parse().ok())
        .unwrap_or(16.0)
}

pub fn error_message(error: &JsValue) -> String {
    error.dyn_ref::<js_sys::Error>().map_or_else(
        || error.as_string().unwrap_or_else(|| format!("{error:?}")),
        |error| String::from(error.to_string()),
    )
}

pub async fn wait<T: JsCast>(promise: Promise) -> Result<T, JsValue> {
    JsFuture::from(promise).await?.dyn_into()
}

pub async fn settle(promise: Promise) -> Result<JsValue, JsValue> {
    JsFuture::from(promise).await
}

pub fn supports_directory_picker() -> bool {
    Reflect::has(&window(), &JsValue::from_str("showDirectoryPicker")).unwrap_or(false)
}

fn options(entries: &[(&str, &str)]) -> JsValue {
    let object = Object::new();
    for (name, value) in entries {
        let _ = Reflect::set(&object, &JsValue::from_str(name), &JsValue::from_str(value));
    }
    object.into()
}

pub async fn pick_directory() -> Result<FileSystemDirectoryHandle, JsValue> {
    wait(show_directory_picker(&options(&[
        ("id", "tgprep"),
        ("mode", "readwrite"),
    ]))?)
    .await
}

pub async fn ensure_write_permission(handle: &FileSystemHandle) -> Result<bool, JsValue> {
    let descriptor = options(&[("mode", "readwrite")]);
    let handle: &PermissionHandle = handle.unchecked_ref();
    if settle(handle.query_permission(&descriptor))
        .await?
        .as_string()
        .as_deref()
        == Some("granted")
    {
        return Ok(true);
    }
    Ok(settle(handle.request_permission(&descriptor))
        .await?
        .as_string()
        .as_deref()
        == Some("granted"))
}

fn clipboard() -> Clipboard {
    Reflect::get(&window().navigator(), &JsValue::from_str("clipboard"))
        .unwrap_or(JsValue::UNDEFINED)
        .unchecked_into()
}

pub fn write_image(png: &Promise) -> Promise {
    let items = Object::new();
    let _ = Reflect::set(&items, &JsValue::from_str("image/png"), png);
    clipboard().write(&Array::of1(&ClipboardItem::new(&items)))
}

pub fn write_text(text: &str) -> Promise {
    clipboard().write_text(text)
}

fn retire<T: 'static>(closure: T) {
    wasm_bindgen_futures::spawn_local(async move { drop(closure) });
}

pub struct Listener {
    target: web_sys::EventTarget,
    name: &'static str,
    capture: bool,
    callback: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Listener {
    pub fn new(
        target: &web_sys::EventTarget,
        name: &'static str,
        capture: bool,
        callback: impl FnMut(web_sys::Event) + 'static,
    ) -> Self {
        let callback = Closure::<dyn FnMut(web_sys::Event)>::new(callback);
        let _ = target.add_event_listener_with_callback_and_bool(
            name,
            callback.as_ref().unchecked_ref(),
            capture,
        );
        Self {
            target: target.clone(),
            name,
            capture,
            callback: Some(callback),
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            let _ = self.target.remove_event_listener_with_callback_and_bool(
                self.name,
                callback.as_ref().unchecked_ref(),
                self.capture,
            );
            retire(callback);
        }
    }
}

pub struct Frame {
    id: i32,
    callback: Option<Closure<dyn FnMut(f64)>>,
}

impl Frame {
    pub fn request(callback: impl FnOnce(f64) + 'static) -> Self {
        let mut callback = Some(callback);
        let closure = Closure::<dyn FnMut(f64)>::new(move |time| {
            if let Some(callback) = callback.take() {
                callback(time);
            }
        });
        let id = window()
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .unwrap_or_default();
        Self {
            id,
            callback: Some(closure),
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let _ = window().cancel_animation_frame(self.id);
        if let Some(callback) = self.callback.take() {
            retire(callback);
        }
    }
}

pub struct Timeout {
    id: i32,
    callback: Option<Closure<dyn FnMut()>>,
}

impl Timeout {
    pub fn new(milliseconds: i32, callback: impl FnOnce() + 'static) -> Self {
        let mut callback = Some(callback);
        let closure = Closure::<dyn FnMut()>::new(move || {
            if let Some(callback) = callback.take() {
                callback();
            }
        });
        let id = window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                milliseconds,
            )
            .unwrap_or_default();
        Self {
            id,
            callback: Some(closure),
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        window().clear_timeout_with_handle(self.id);
        if let Some(callback) = self.callback.take() {
            retire(callback);
        }
    }
}

#[derive(Default)]
pub struct Serial {
    busy: std::cell::Cell<bool>,
    waiting: RefCell<std::collections::VecDeque<js_sys::Function>>,
}

pub struct SerialGuard(Rc<Serial>);

impl Serial {
    pub async fn lock(self: &Rc<Self>) -> SerialGuard {
        if self.busy.get() {
            let ready =
                Promise::new(&mut |resolve, _| self.waiting.borrow_mut().push_back(resolve));
            let _ = JsFuture::from(ready).await;
        } else {
            self.busy.set(true);
        }
        SerialGuard(self.clone())
    }
}

impl Drop for SerialGuard {
    fn drop(&mut self) {
        let next = self.0.waiting.borrow_mut().pop_front();
        match next {
            Some(resolve) => {
                let _ = resolve.call0(&JsValue::NULL);
            }
            None => self.0.busy.set(false),
        }
    }
}
