use crate::icons::{Icon, stroked, svg};
use crate::store::store;
use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Overlay,
}

#[component]
pub fn IconButton(
    #[prop(into)] icon: Signal<Icon>,
    #[prop(into)] label: Signal<&'static str>,
    #[prop(into)] on_click: Callback<()>,
    #[prop(into, default = Signal::stored(false))] active: Signal<bool>,
    #[prop(into, default = Signal::stored(false))] disabled: Signal<bool>,
    #[prop(default = Tone::Plain)] tone: Tone,
) -> impl IntoView {
    let class = move || match (tone, active.get()) {
        (Tone::Overlay, true) => "grid size-8 shrink-0 place-items-center rounded-lg transition-colors disabled:opacity-30 bg-accent text-white",
        (Tone::Overlay, false) => "grid size-8 shrink-0 place-items-center rounded-lg transition-colors disabled:opacity-30 text-white/80 hover:bg-white/15 hover:text-white",
        (Tone::Plain, true) => "grid size-8 shrink-0 place-items-center rounded-lg transition-colors disabled:opacity-30 bg-accent/15 text-accent",
        (Tone::Plain, false) => "grid size-8 shrink-0 place-items-center rounded-lg transition-colors disabled:opacity-30 text-muted hover:bg-raised hover:text-fg",
    };
    view! {
        <button
            type="button"
            title=move || label.get()
            aria-label=move || label.get()
            aria-pressed=move || active.get().to_string()
            disabled=move || disabled.get()
            on:click=move |_| on_click.run(())
            class=class
        >
            {move || svg(icon.get(), "size-4")}
        </button>
    }
}

#[component]
pub fn Chip(
    #[prop(into)] active: Signal<bool>,
    #[prop(into)] on_click: Callback<()>,
    #[prop(into)] label: Signal<String>,
    #[prop(into, default = Signal::stored(None))] color: Signal<Option<String>>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            type="button"
            title=move || label.get()
            aria-pressed=move || active.get().to_string()
            on:click=move |_| on_click.run(())
            style:background-color=move || if active.get() { color.get() } else { None }
            class=move || {
                if active.get() {
                    "flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium transition-colors bg-accent text-white"
                } else {
                    "flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium transition-colors bg-raised text-muted hover:text-fg"
                }
            }
        >
            {move || {
                color
                    .get()
                    .filter(|_| !active.get())
                    .map(|color| view! { <span class="size-2 rounded-full" style:background-color=color></span> })
            }}
            {children()}
        </button>
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Label {
    Text(&'static str),
    Icon(Icon),
}

pub struct Choice<T> {
    pub value: T,
    pub label: Label,
    pub title: &'static str,
}

#[component]
pub fn Segmented<T: Copy + PartialEq + Send + Sync + 'static>(#[prop(into)] value: Signal<T>, choices: Vec<Choice<T>>, #[prop(into)] on_change: Callback<T>) -> impl IntoView {
    view! {
        <div class="flex shrink-0 rounded-lg bg-raised p-0.5">
            {choices
                .into_iter()
                .map(|choice| {
                    let selected = move || value.get() == choice.value;
                    view! {
                        <button
                            type="button"
                            title=choice.title
                            aria-pressed=move || selected().to_string()
                            on:click=move |_| on_change.run(choice.value)
                            class=move || {
                                if selected() {
                                    "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-colors bg-panel text-fg shadow-sm"
                                } else {
                                    "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-colors text-muted hover:text-fg"
                                }
                            }
                        >
                            {match choice.label {
                                Label::Text(text) => text.into_any(),
                                Label::Icon(icon) => svg(icon, "size-3.5").into_any(),
                            }}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
pub fn RangeInput(
    #[prop(into)] value: Signal<f64>,
    min: f64,
    max: f64,
    step: f64,
    label: &'static str,
    #[prop(into)] on_change: Callback<f64>,
    #[prop(into, default = Callback::new(|()| ()))] on_start: Callback<()>,
    #[prop(into, default = Callback::new(|()| ()))] on_reset: Callback<()>,
    class: &'static str,
) -> impl IntoView {
    view! {
        <input
            type="range"
            aria-label=label
            title=label
            min=min
            max=max
            step=step
            prop:value=move || value.get()
            on:pointerdown=move |_| on_start.run(())
            on:dblclick=move |_| on_reset.run(())
            on:input=move |event| {
                if let Ok(value) = event_target_value(&event).parse() {
                    on_change.run(value);
                }
            }
            class=class
        />
    }
}

#[component]
pub fn PaneHeader(children: Children) -> impl IntoView {
    view! { <div class="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b border-line px-3 py-2">{children()}</div> }
}

#[component]
pub fn Toasts() -> impl IntoView {
    move || {
        store().toast.get().map(|(message, _)| {
            view! {
                <div class="pointer-events-none fixed inset-x-0 bottom-6 z-[70] flex justify-center">
                    <div role="status" class="animate-pop rounded-full bg-fg px-4 py-2 text-sm font-medium text-canvas shadow-xl">
                        {message}
                    </div>
                </div>
            }
        })
    }
}

pub fn check_icon() -> impl IntoView {
    stroked(crate::icons::CHECK, "size-3.5", "3.5")
}
