use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use std::io::Read;
use tgprep_server::{Client, router};
use tower::ServiceExt;

fn client() -> (tempfile::TempDir, axum::Router) {
    let directory = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(directory.path().join("assets")).expect("assets dir");
    std::fs::write(directory.path().join("index.html"), "<!doctype html><title>Prepost</title>").expect("index");
    std::fs::write(directory.path().join("favicon.svg"), "<svg/>").expect("favicon");
    std::fs::write(directory.path().join("assets/tgprep-0123456789ab.wasm"), vec![0_u8; 4096]).expect("wasm");
    std::fs::write(directory.path().join("assets/tgprep-0123456789ab.js"), "export default 1;".repeat(100)).expect("js");
    let app = router(Client::load(directory.path()).expect("loads"));
    (directory, app)
}

async fn get(app: &axum::Router, path: &str, encoding: Option<&str>) -> axum::response::Response {
    let mut request = Request::get(path);
    if let Some(encoding) = encoding {
        request = request.header(header::ACCEPT_ENCODING, encoding);
    }
    app.clone().oneshot(request.body(Body::empty()).expect("request")).await.expect("response")
}

fn header_of<'a>(response: &'a axum::response::Response, name: header::HeaderName) -> Option<&'a str> {
    response.headers().get(name).map(|value| value.to_str().expect("ascii header"))
}

#[tokio::test]
async fn serves_the_page_uncached_and_hashed_assets_immutable() {
    let (_directory, app) = client();
    let page = get(&app, "/", None).await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(header_of(&page, header::CACHE_CONTROL), Some("no-cache"));
    assert_eq!(header_of(&page, header::CONTENT_TYPE), Some("text/html; charset=utf-8"));
    let body = page.into_body().collect().await.expect("body").to_bytes();
    assert_eq!(&body[..], b"<!doctype html><title>Prepost</title>");

    let wasm = get(&app, "/assets/tgprep-0123456789ab.wasm", None).await;
    assert_eq!(wasm.status(), StatusCode::OK);
    assert_eq!(header_of(&wasm, header::CONTENT_TYPE), Some("application/wasm"));
    assert_eq!(header_of(&wasm, header::CACHE_CONTROL), Some("public, max-age=31536000, immutable"));
    assert_eq!(wasm.into_body().collect().await.expect("body").to_bytes().len(), 4096);

    let favicon = get(&app, "/favicon.svg", None).await;
    assert_eq!(header_of(&favicon, header::CONTENT_TYPE), Some("image/svg+xml"));
    assert_eq!(header_of(&favicon, header::CACHE_CONTROL), Some("no-cache"));
}

#[tokio::test]
async fn compresses_text_and_wasm_by_the_accepted_encoding() {
    let (_directory, app) = client();
    let original = "export default 1;".repeat(100);

    let brotli = get(&app, "/assets/tgprep-0123456789ab.js", Some("gzip, deflate, br, zstd")).await;
    assert_eq!(header_of(&brotli, header::CONTENT_ENCODING), Some("br"));
    assert_eq!(header_of(&brotli, header::VARY), Some("accept-encoding"));
    let compressed = brotli.into_body().collect().await.expect("body").to_bytes();
    let mut decoded = String::new();
    brotli::Decompressor::new(&compressed[..], 4096).read_to_string(&mut decoded).expect("brotli");
    assert_eq!(decoded, original);

    let gzip = get(&app, "/assets/tgprep-0123456789ab.js", Some("gzip;q=1.0")).await;
    assert_eq!(header_of(&gzip, header::CONTENT_ENCODING), Some("gzip"));
    let compressed = gzip.into_body().collect().await.expect("body").to_bytes();
    let mut decoded = String::new();
    flate2::read::GzDecoder::new(&compressed[..]).read_to_string(&mut decoded).expect("gzip");
    assert_eq!(decoded, original);

    let identity = get(&app, "/assets/tgprep-0123456789ab.js", None).await;
    assert_eq!(header_of(&identity, header::CONTENT_ENCODING), None);
}

#[tokio::test]
async fn answers_unknown_paths_with_not_found() {
    let (_directory, app) = client();
    assert_eq!(get(&app, "/assets/missing.js", None).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(get(&app, "/posts", None).await.status(), StatusCode::NOT_FOUND);
}

#[test]
fn refuses_a_directory_without_a_page() {
    let directory = tempfile::tempdir().expect("temp dir");
    assert!(Client::load(directory.path()).is_err());
}
