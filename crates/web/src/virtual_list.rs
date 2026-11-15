use crate::browser::Listener;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{ResizeObserver, ResizeObserverEntry};

pub struct ResizeWatch {
    observer: ResizeObserver,
    _callback: Closure<dyn FnMut(js_sys::Array)>,
}

impl ResizeWatch {
    pub fn new(element: &web_sys::Element, mut on_resize: impl FnMut(&ResizeObserverEntry) + 'static) -> Option<Self> {
        let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
            for entry in entries.iter() {
                if let Ok(entry) = entry.dyn_into::<ResizeObserverEntry>() {
                    on_resize(&entry);
                }
            }
        });
        let observer = ResizeObserver::new(callback.as_ref().unchecked_ref()).ok()?;
        observer.observe(element);
        Some(Self { observer, _callback: callback })
    }
}

impl Drop for ResizeWatch {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

#[derive(Clone, Copy)]
pub struct Viewport {
    pub scroll_top: RwSignal<f64>,
    pub width: RwSignal<f64>,
    pub height: RwSignal<f64>,
}

pub fn watch_viewport(scroller: NodeRef<Div>) -> Viewport {
    let viewport = Viewport { scroll_top: RwSignal::new(0.0), width: RwSignal::new(0.0), height: RwSignal::new(0.0) };
    let watches = StoredValue::new_local(None::<(ResizeWatch, Listener)>);
    Effect::new(move |_| {
        let Some(element) = scroller.get() else { return };
        let observed = ResizeWatch::new(&element, move |entry| {
            let rect = entry.content_rect();
            viewport.width.set(rect.width());
            viewport.height.set(rect.height());
        });
        let scrolled = element.clone();
        let listener = Listener::new(&element, "scroll", false, move |_| viewport.scroll_top.set(f64::from(scrolled.scroll_top())));
        watches.set_value(observed.map(|observed| (observed, listener)));
    });
    viewport
}

pub fn visible_range(starts: &[f64], sizes: &[f64], scroll_top: f64, height: f64, overscan: usize) -> std::ops::Range<usize> {
    let count = starts.len();
    if count == 0 {
        return 0..0;
    }
    let first = starts.partition_point(|start| *start <= scroll_top).saturating_sub(1);
    let first = (first..count).find(|index| starts[*index] + sizes[*index] > scroll_top).unwrap_or(count - 1);
    let last = starts.partition_point(|start| *start < scroll_top + height).saturating_sub(1).max(first);
    first.saturating_sub(overscan)..(last + overscan + 1).min(count)
}

pub fn centered_offset(start: f64, size: f64, viewport: f64, total: f64) -> f64 {
    (start + size / 2.0 - viewport / 2.0).clamp(0.0, (total - viewport).max(0.0))
}
