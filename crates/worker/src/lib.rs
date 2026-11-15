use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Blob, File, ImageBitmap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Thumb,
    View,
}

impl Job {
    const fn name(self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::View => "view",
        }
    }
}

pub struct Request {
    pub job: Job,
    pub key: String,
    pub file: File,
    pub long_side: f64,
}

pub struct Thumbnail {
    pub blob: Blob,
    pub width: f64,
    pub height: f64,
}

pub enum Response {
    Ready,
    Thumb { key: String, thumbnail: Option<Thumbnail> },
    View { key: String, bitmap: ImageBitmap },
}

#[derive(Debug)]
pub struct Malformed;

fn object(entries: &[(&str, &JsValue)]) -> JsValue {
    let object = Object::new();
    for (name, value) in entries {
        let _ = Reflect::set(&object, &JsValue::from_str(name), value);
    }
    object.into()
}

fn field(value: &JsValue, name: &str) -> Result<JsValue, Malformed> {
    Reflect::get(value, &JsValue::from_str(name)).map_err(|_| Malformed)
}

fn text(value: &JsValue, name: &str) -> Result<String, Malformed> {
    field(value, name)?.as_string().ok_or(Malformed)
}

fn number(value: &JsValue, name: &str) -> Result<f64, Malformed> {
    field(value, name)?.as_f64().ok_or(Malformed)
}

fn cast<T: JsCast>(value: &JsValue, name: &str) -> Result<T, Malformed> {
    field(value, name)?.dyn_into().map_err(|_| Malformed)
}

impl Request {
    pub fn to_js(&self) -> JsValue {
        object(&[
            ("job", &JsValue::from_str(self.job.name())),
            ("key", &JsValue::from_str(&self.key)),
            ("file", &self.file),
            ("longSide", &JsValue::from_f64(self.long_side)),
        ])
    }

    pub fn from_js(value: &JsValue) -> Result<Self, Malformed> {
        let job = match text(value, "job")?.as_str() {
            "thumb" => Job::Thumb,
            "view" => Job::View,
            _ => return Err(Malformed),
        };
        Ok(Self { job, key: text(value, "key")?, file: cast(value, "file")?, long_side: number(value, "longSide")? })
    }
}

impl Response {
    pub fn to_js(&self) -> JsValue {
        match self {
            Self::Ready => object(&[("kind", &JsValue::from_str("ready"))]),
            Self::Thumb { key, thumbnail: None } => object(&[("kind", &JsValue::from_str("thumb")), ("key", &JsValue::from_str(key))]),
            Self::Thumb { key, thumbnail: Some(thumbnail) } => object(&[
                ("kind", &JsValue::from_str("thumb")),
                ("key", &JsValue::from_str(key)),
                ("blob", &thumbnail.blob),
                ("width", &JsValue::from_f64(thumbnail.width)),
                ("height", &JsValue::from_f64(thumbnail.height)),
            ]),
            Self::View { key, bitmap } => {
                object(&[("kind", &JsValue::from_str("view")), ("key", &JsValue::from_str(key)), ("bitmap", bitmap)])
            }
        }
    }

    pub fn from_js(value: &JsValue) -> Result<Self, Malformed> {
        match text(value, "kind")?.as_str() {
            "ready" => Ok(Self::Ready),
            "thumb" => {
                let blob = field(value, "blob")?;
                let thumbnail = if blob.is_undefined() {
                    None
                } else {
                    Some(Thumbnail { blob: blob.dyn_into().map_err(|_| Malformed)?, width: number(value, "width")?, height: number(value, "height")? })
                };
                Ok(Self::Thumb { key: text(value, "key")?, thumbnail })
            }
            "view" => Ok(Self::View { key: text(value, "key")?, bitmap: cast(value, "bitmap")? }),
            _ => Err(Malformed),
        }
    }
}

pub fn scaled(width: f64, height: f64, long_side: f64) -> (f64, f64) {
    let scale = (long_side / width.max(height)).min(1.0);
    ((width * scale).round().max(1.0), (height * scale).round().max(1.0))
}
