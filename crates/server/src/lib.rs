use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");
const NO_CACHE: HeaderValue = HeaderValue::from_static("no-cache");
const COMPRESSIBLE: [&str; 5] = ["html", "js", "css", "wasm", "svg"];

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("reading {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("{0} has no index.html")]
    MissingIndex(PathBuf),
}

struct Asset {
    content_type: &'static str,
    cache: HeaderValue,
    identity: Vec<u8>,
    brotli: Option<Vec<u8>>,
    gzip: Option<Vec<u8>>,
}

pub struct Client {
    assets: HashMap<String, Asset>,
}

fn content_type(extension: &str) -> &'static str {
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

fn brotli(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut output, 1 << 16, 11, 22);
    writer.write_all(data)?;
    drop(writer);
    Ok(output)
}

fn gzip(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

fn collect(directory: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) -> Result<(), ServeError> {
    let read = |source| ServeError::Read { path: directory.to_owned(), source };
    for entry in std::fs::read_dir(directory).map_err(read)? {
        let entry = entry.map_err(read)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if entry.file_type().map_err(read)?.is_dir() {
            collect(&path, &format!("{prefix}{name}/"), files)?;
        } else {
            files.push((format!("{prefix}{name}"), path));
        }
    }
    Ok(())
}

impl Client {
    pub fn load(directory: &Path) -> Result<Self, ServeError> {
        let mut files = Vec::new();
        collect(directory, "/", &mut files)?;
        let mut assets = HashMap::new();
        for (route, path) in files {
            let identity = std::fs::read(&path).map_err(|source| ServeError::Read { path: path.clone(), source })?;
            let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
            let compress = |encode: fn(&[u8]) -> io::Result<Vec<u8>>| {
                COMPRESSIBLE
                    .contains(&extension)
                    .then(|| encode(&identity))
                    .transpose()
                    .map_err(|source| ServeError::Read { path: path.clone(), source })
            };
            let asset = Asset {
                content_type: content_type(extension),
                cache: if route.starts_with("/assets/") { IMMUTABLE } else { NO_CACHE },
                brotli: compress(brotli)?,
                gzip: compress(gzip)?,
                identity,
            };
            assets.insert(route, asset);
        }
        if !assets.contains_key("/index.html") {
            return Err(ServeError::MissingIndex(directory.to_owned()));
        }
        Ok(Self { assets })
    }

    fn respond(&self, path: &str, headers: &HeaderMap) -> Response {
        let route = if path == "/" { "/index.html" } else { path };
        let Some(asset) = self.assets.get(route) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let accepted = headers.get(ACCEPT_ENCODING).and_then(|value| value.to_str().ok()).unwrap_or_default();
        let accepts = |encoding: &str| accepted.split(',').any(|candidate| candidate.split(';').next().is_some_and(|name| name.trim() == encoding));
        let (body, encoding) = match (&asset.brotli, &asset.gzip) {
            (Some(compressed), _) if accepts("br") => (compressed, Some("br")),
            (_, Some(compressed)) if accepts("gzip") => (compressed, Some("gzip")),
            _ => (&asset.identity, None),
        };
        let mut response = Response::new(Body::from(body.clone()));
        let response_headers = response.headers_mut();
        response_headers.insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
        response_headers.insert(CACHE_CONTROL, asset.cache.clone());
        if asset.brotli.is_some() {
            response_headers.insert(VARY, HeaderValue::from_static("accept-encoding"));
        }
        if let Some(encoding) = encoding {
            response_headers.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding));
        }
        response
    }
}

async fn serve(State(client): State<Arc<Client>>, request: Request) -> Response {
    client.respond(request.uri().path(), request.headers())
}

pub fn router(client: Client) -> Router {
    Router::new().fallback(serve).with_state(Arc::new(client))
}
