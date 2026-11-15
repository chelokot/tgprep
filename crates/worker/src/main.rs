use js_sys::{Array, Promise};
use tgprep_worker::{Job, Request, Response, Thumbnail, scaled};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, DedicatedWorkerGlobalScope, ImageBitmap, ImageBitmapOptions, ImageEncodeOptions, MessageEvent, OffscreenCanvas, OffscreenCanvasRenderingContext2d, ResizeQuality};

const THUMB_QUALITY: f64 = 0.85;

async fn wait<T: JsCast>(promise: Result<Promise, JsValue>) -> Result<T, JsValue> {
    JsFuture::from(promise?).await?.dyn_into()
}

async fn resized(scope: &DedicatedWorkerGlobalScope, original: &ImageBitmap, long_side: f64) -> Result<ImageBitmap, JsValue> {
    let (width, height) = scaled(f64::from(original.width()), f64::from(original.height()), long_side);
    let options = ImageBitmapOptions::new();
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "rounded pixel sizes of a decoded image")]
    let (width, height) = (width as u32, height as u32);
    options.set_resize_width(width);
    options.set_resize_height(height);
    options.set_resize_quality(ResizeQuality::High);
    wait(scope.create_image_bitmap_with_image_bitmap_and_image_bitmap_options(original, &options)).await
}

async fn thumbnail(scope: &DedicatedWorkerGlobalScope, file: &web_sys::File, long_side: f64) -> Result<Thumbnail, JsValue> {
    let original: ImageBitmap = wait(scope.create_image_bitmap_with_blob(file)).await?;
    let small = resized(scope, &original, long_side).await?;
    let canvas = OffscreenCanvas::new(small.width(), small.height())?;
    let context: OffscreenCanvasRenderingContext2d = canvas.get_context("2d")?.ok_or_else(|| JsValue::from_str("2d context"))?.dyn_into()?;
    context.draw_image_with_image_bitmap(&small, 0.0, 0.0)?;
    let options = ImageEncodeOptions::new();
    options.set_type("image/webp");
    options.set_quality(THUMB_QUALITY);
    let blob: Blob = wait(canvas.convert_to_blob_with_options(&options)).await?;
    let thumbnail = Thumbnail { blob, width: f64::from(original.width()), height: f64::from(original.height()) };
    original.close();
    small.close();
    Ok(thumbnail)
}

async fn view(scope: &DedicatedWorkerGlobalScope, file: &web_sys::File, max_side: f64) -> Result<ImageBitmap, JsValue> {
    let original: ImageBitmap = wait(scope.create_image_bitmap_with_blob(file)).await?;
    if f64::from(original.width().max(original.height())) <= max_side {
        return Ok(original);
    }
    let bitmap = resized(scope, &original, max_side).await?;
    original.close();
    Ok(bitmap)
}

async fn handle(scope: DedicatedWorkerGlobalScope, request: Request) {
    match request.job {
        Job::Thumb => {
            let thumbnail = thumbnail(&scope, &request.file, request.long_side).await.ok();
            let _ = scope.post_message(&Response::Thumb { key: request.key, thumbnail }.to_js());
        }
        Job::View => {
            if let Ok(bitmap) = view(&scope, &request.file, request.long_side).await {
                let transfer = Array::of1(&bitmap);
                let _ = scope.post_message_with_transfer(&Response::View { key: request.key, bitmap }.to_js(), &transfer);
            }
        }
    }
}

fn main() {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let receiver = scope.clone();
    let onmessage = Closure::<dyn Fn(MessageEvent)>::new(move |event: MessageEvent| {
        if let Ok(request) = Request::from_js(&event.data()) {
            spawn_local(handle(receiver.clone(), request));
        }
    });
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    let _ = scope.post_message(&Response::Ready.to_js());
}
