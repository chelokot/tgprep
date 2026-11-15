use crate::browser::{Serial, settle, wait, window};
use crate::persistence::EXPORT_DIR;
use crate::renderer::{Canvas, Renderer};
use crate::store::PhotoFile;
use js_sys::Promise;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tgprep_model::doc::{Edits, Post};
use tgprep_model::geometry::{Size, fit_within, output_size};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys::{
    Blob, File, FileSystemDirectoryHandle, FileSystemGetDirectoryOptions, FileSystemGetFileOptions,
    FileSystemRemoveOptions, FileSystemWritableFileStream, ImageBitmap, ImageBitmapOptions,
    ImageEncodeOptions, OffscreenCanvas, ResizeQuality,
};

const PNG_CACHE_LIMIT: usize = 24;
const JPEG_QUALITY: f64 = 0.93;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
}

thread_local! {
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
    static EXPORTS: Rc<Serial> = Rc::new(Serial::default());
    static PNG_CACHE: RefCell<VecDeque<(String, Promise)>> = const { RefCell::new(VecDeque::new()) };
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded pixel sizes"
)]
fn pixels(value: f64) -> u32 {
    value.round() as u32
}

fn with_renderer<T>(
    use_renderer: impl FnOnce(&mut Renderer) -> Result<T, JsValue>,
) -> Result<T, JsValue> {
    RENDERER.with_borrow_mut(|renderer| {
        if renderer.is_none() {
            *renderer = Some(Renderer::new(Canvas::Offscreen(OffscreenCanvas::new(
                1, 1,
            )?))?);
        }
        use_renderer(
            renderer
                .as_mut()
                .ok_or_else(|| JsValue::from_str("renderer"))?,
        )
    })
}

pub async fn render_photo(
    file: &PhotoFile,
    edits: &Edits,
    long_side: Option<u32>,
    format: Format,
) -> Result<Blob, JsValue> {
    let _guard = EXPORTS.with(Clone::clone).lock().await;
    let data: File = wait(file.handle.get_file()).await?;
    let original: ImageBitmap = wait(window().create_image_bitmap_with_blob(&data)?).await?;
    let (width, height) = (f64::from(original.width()), f64::from(original.height()));
    let max_texture_size = with_renderer(|renderer| Ok(renderer.max_texture_size))?;
    let output = output_size(
        Size {
            w: width,
            h: height,
        },
        edits,
    );
    let target = fit_within(
        output,
        long_side
            .map_or(f64::INFINITY, f64::from)
            .min(max_texture_size),
    );
    let scale = (target.w / output.w)
        .min(max_texture_size / width.max(height))
        .min(1.0);
    let source = if scale < 1.0 {
        let options = ImageBitmapOptions::new();
        options.set_resize_width(pixels(width * scale));
        options.set_resize_height(pixels(height * scale));
        options.set_resize_quality(ResizeQuality::High);
        wait(
            window().create_image_bitmap_with_image_bitmap_and_image_bitmap_options(
                &original, &options,
            )?,
        )
        .await?
    } else {
        original.clone()
    };
    let canvas = with_renderer(|renderer| {
        renderer.render(
            &source,
            edits,
            edits.crop,
            (pixels(target.w), pixels(target.h)),
        )?;
        match &renderer.canvas {
            Canvas::Offscreen(canvas) => Ok(canvas.clone()),
            Canvas::Element(_) => Err(JsValue::from_str("export renders offscreen")),
        }
    })?;
    let options = ImageEncodeOptions::new();
    options.set_type(match format {
        Format::Png => "image/png",
        Format::Jpeg => "image/jpeg",
    });
    options.set_quality(JPEG_QUALITY);
    let blob = wait(canvas.convert_to_blob_with_options(&options)?).await;
    with_renderer(|renderer| {
        renderer.forget(&source);
        Ok(())
    })?;
    source.close();
    original.close();
    blob
}

pub fn prepare_png(file: &Rc<PhotoFile>, edits: &Edits, long_side: Option<u32>) -> Promise {
    let key = format!(
        "{}|{}|{}|{}",
        file.id,
        file.last_modified,
        long_side.map_or_else(|| "null".to_owned(), |side| side.to_string()),
        tgprep_model::json::stringify(edits).unwrap_or_default()
    );
    let cached = PNG_CACHE.with_borrow_mut(|cache| {
        let position = cache.iter().position(|(candidate, _)| *candidate == key)?;
        let entry = cache.remove(position)?;
        let promise = entry.1.clone();
        cache.push_back(entry);
        Some(promise)
    });
    if let Some(promise) = cached {
        return promise;
    }
    let (file, edits) = (file.clone(), *edits);
    let forget = key.clone();
    let rendering = future_to_promise(async move {
        let rendered = render_photo(&file, &edits, long_side, Format::Png).await;
        if rendered.is_err() {
            PNG_CACHE.with_borrow_mut(|cache| cache.retain(|(candidate, _)| *candidate != forget));
        }
        rendered.map(JsValue::from)
    });
    PNG_CACHE.with_borrow_mut(|cache| {
        cache.push_back((key, rendering.clone()));
        while cache.len() > PNG_CACHE_LIMIT {
            cache.pop_front();
        }
    });
    rendering
}

pub fn copy_photo(file: &Rc<PhotoFile>, edits: &Edits, long_side: Option<u32>) -> Promise {
    crate::browser::write_image(&prepare_png(file, edits, long_side))
}

enum Contents<'a> {
    Image(&'a Blob),
    Text(&'a str),
}

async fn write_file(
    folder: &FileSystemDirectoryHandle,
    name: &str,
    contents: Contents<'_>,
) -> Result<(), JsValue> {
    let options = FileSystemGetFileOptions::new();
    options.set_create(true);
    let handle: web_sys::FileSystemFileHandle =
        wait(folder.get_file_handle_with_options(name, &options)).await?;
    let writable: FileSystemWritableFileStream = wait(handle.create_writable()).await?;
    let written = match contents {
        Contents::Image(blob) => writable.write_with_blob(blob)?,
        Contents::Text(text) => writable.write_with_str(text)?,
    };
    settle(written).await?;
    settle(writable.close()).await.map(drop)
}

pub async fn export_post(
    root: &FileSystemDirectoryHandle,
    post: &Post,
    number: usize,
    photos: Vec<(Rc<PhotoFile>, Edits)>,
    long_side: Option<u32>,
) -> Result<String, JsValue> {
    let directory = FileSystemGetDirectoryOptions::new();
    directory.set_create(true);
    let export_root: FileSystemDirectoryHandle =
        wait(root.get_directory_handle_with_options(EXPORT_DIR, &directory)).await?;
    let name = format!("post-{number:03}");
    let recursive = FileSystemRemoveOptions::new();
    recursive.set_recursive(true);
    let _ = settle(export_root.remove_entry_with_options(&name, &recursive)).await;
    let folder: FileSystemDirectoryHandle =
        wait(export_root.get_directory_handle_with_options(&name, &directory)).await?;
    let writes = js_sys::Array::new();
    for (index, (file, edits)) in photos.into_iter().enumerate() {
        let folder = folder.clone();
        writes.push(&future_to_promise(async move {
            let blob = render_photo(&file, &edits, long_side, Format::Jpeg).await?;
            write_file(
                &folder,
                &format!("{:02}.jpg", index + 1),
                Contents::Image(&blob),
            )
            .await
            .map(|()| JsValue::UNDEFINED)
        }));
    }
    settle(Promise::all(&writes)).await?;
    if !post.caption.trim().is_empty() {
        write_file(&folder, "caption.txt", Contents::Text(&post.caption)).await?;
    }
    Ok(format!("{EXPORT_DIR}/{name}"))
}
