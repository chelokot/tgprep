use crate::browser::supports_directory_picker;
use crate::i18n::t;
use crate::icons::{self, svg};
use crate::persistence::{RecentFolder, load_recent_folders, pick_folder, reopen_folder};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn Welcome() -> impl IntoView {
    let recent: RwSignal<Vec<RecentFolder>, LocalStorage> = RwSignal::new_local(Vec::new());
    spawn_local(async move {
        let folders = load_recent_folders().await;
        let _ = recent.try_set(folders);
    });
    view! {
        <main class="grid min-h-dvh place-items-center bg-canvas p-6">
            <div class="w-full max-w-md animate-pop">
                <div class="mb-6 grid size-14 place-items-center rounded-2xl bg-accent text-white shadow-lg shadow-accent/30">{svg(icons::SEND, "size-7 -translate-x-0.5")}</div>
                <h1 class="text-3xl font-bold tracking-tight text-fg">{t().app_name}</h1>
                <p class="mt-2 text-sm leading-relaxed text-muted">{t().tagline}</p>
                {if supports_directory_picker() {
                    view! {
                        <button
                            type="button"
                            on:click=move |_| pick_folder()
                            class="mt-8 flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-accent font-semibold text-white shadow-lg shadow-accent/25 transition hover:brightness-110"
                        >
                            {svg(icons::FOLDER_OPEN, "size-5")}
                            " "
                            {t().open_folder}
                        </button>
                    }
                    .into_any()
                } else {
                    view! { <p class="mt-8 rounded-xl bg-danger/10 p-4 text-sm text-danger">{t().unsupported}</p> }.into_any()
                }}
                {move || {
                    recent.with(|recent| {
                        (!recent.is_empty())
                            .then(|| {
                                view! {
                                    <section class="mt-8">
                                        <h2 class="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-faint uppercase">
                                            {svg(icons::HISTORY, "size-3.5")}
                                            " "
                                            {t().recent_folders}
                                        </h2>
                                        <ul class="space-y-1">
                                            {recent
                                                .iter()
                                                .map(|folder| {
                                                    let handle = folder.handle.clone();
                                                    view! {
                                                        <li>
                                                            <button
                                                                type="button"
                                                                on:click=move |_| reopen_folder(handle.clone())
                                                                class="flex h-10 w-full items-center gap-2 rounded-lg px-3 text-left text-sm text-fg transition hover:bg-raised"
                                                            >
                                                                {svg(icons::FOLDER_OPEN, "size-4 text-muted")}
                                                                " "
                                                                {folder.name.clone()}
                                                            </button>
                                                        </li>
                                                    }
                                                })
                                                .collect_view()}
                                        </ul>
                                    </section>
                                }
                            })
                    })
                }}
            </div>
        </main>
    }
}
