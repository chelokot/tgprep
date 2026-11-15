use indexmap::IndexMap;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Index, IndexMut};

pub const MAX_POST_PHOTOS: usize = 10;
pub const MAX_CAPTION_LENGTH: usize = 1024;

macro_rules! id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

id!(PhotoId);
id!(PostId);
id!(CategoryId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Adjustment {
    Exposure,
    Brightness,
    Contrast,
    Highlights,
    Shadows,
    Temperature,
    Tint,
    Saturation,
    Vibrance,
    Sharpness,
    Vignette,
    Fade,
    Grain,
}

impl Adjustment {
    pub const ALL: [Self; 13] = [
        Self::Exposure,
        Self::Brightness,
        Self::Contrast,
        Self::Highlights,
        Self::Shadows,
        Self::Temperature,
        Self::Tint,
        Self::Saturation,
        Self::Vibrance,
        Self::Sharpness,
        Self::Vignette,
        Self::Fade,
        Self::Grain,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Exposure => "exposure",
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Highlights => "highlights",
            Self::Shadows => "shadows",
            Self::Temperature => "temperature",
            Self::Tint => "tint",
            Self::Saturation => "saturation",
            Self::Vibrance => "vibrance",
            Self::Sharpness => "sharpness",
            Self::Vignette => "vignette",
            Self::Fade => "fade",
            Self::Grain => "grain",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Adjustments([f64; 13]);

impl Adjustments {
    pub fn is_neutral(&self) -> bool {
        self.0.iter().all(|value| *value == 0.0)
    }

    pub fn values(&self) -> [f64; 13] {
        self.0
    }
}

impl Index<Adjustment> for Adjustments {
    type Output = f64;

    fn index(&self, adjustment: Adjustment) -> &f64 {
        &self.0[adjustment as usize]
    }
}

impl IndexMut<Adjustment> for Adjustments {
    fn index_mut(&mut self, adjustment: Adjustment) -> &mut f64 {
        &mut self.0[adjustment as usize]
    }
}

impl Serialize for Adjustments {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(Adjustment::ALL.len()))?;
        for adjustment in Adjustment::ALL {
            map.serialize_entry(adjustment.key(), &self[adjustment])?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Adjustments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AdjustmentsVisitor;

        impl<'de> Visitor<'de> for AdjustmentsVisitor {
            type Value = Adjustments;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object with every adjustment")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Adjustments, A::Error> {
                let mut values = [None; 13];
                while let Some(key) = map.next_key::<String>()? {
                    match Adjustment::ALL
                        .iter()
                        .find(|adjustment| adjustment.key() == key)
                    {
                        Some(adjustment) => {
                            values[*adjustment as usize] = Some(map.next_value::<f64>()?)
                        }
                        None => {
                            map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }
                let mut adjustments = Adjustments::default();
                for adjustment in Adjustment::ALL {
                    adjustments[adjustment] = values[adjustment as usize]
                        .ok_or_else(|| de::Error::missing_field(adjustment.key()))?;
                }
                Ok(adjustments)
            }
        }

        deserializer.deserialize_map(AdjustmentsVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Crop {
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Aspect {
    #[serde(rename = "free")]
    Free,
    #[serde(rename = "original")]
    Original,
    #[serde(rename = "1:1")]
    Square,
    #[serde(rename = "4:5")]
    Portrait4x5,
    #[serde(rename = "5:4")]
    Landscape5x4,
    #[serde(rename = "3:4")]
    Portrait3x4,
    #[serde(rename = "4:3")]
    Landscape4x3,
    #[serde(rename = "2:3")]
    Portrait2x3,
    #[serde(rename = "3:2")]
    Landscape3x2,
    #[serde(rename = "9:16")]
    Portrait9x16,
    #[serde(rename = "16:9")]
    Landscape16x9,
}

impl Aspect {
    pub const ALL: [Self; 11] = [
        Self::Free,
        Self::Original,
        Self::Square,
        Self::Portrait4x5,
        Self::Landscape5x4,
        Self::Portrait3x4,
        Self::Landscape4x3,
        Self::Portrait2x3,
        Self::Landscape3x2,
        Self::Portrait9x16,
        Self::Landscape16x9,
    ];

    pub const fn ratio(self) -> Option<(u8, u8)> {
        match self {
            Self::Free | Self::Original => None,
            Self::Square => Some((1, 1)),
            Self::Portrait4x5 => Some((4, 5)),
            Self::Landscape5x4 => Some((5, 4)),
            Self::Portrait3x4 => Some((3, 4)),
            Self::Landscape4x3 => Some((4, 3)),
            Self::Portrait2x3 => Some((2, 3)),
            Self::Landscape3x2 => Some((3, 2)),
            Self::Portrait9x16 => Some((9, 16)),
            Self::Landscape16x9 => Some((16, 9)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Quarter {
    #[default]
    None,
    Right,
    Half,
    Left,
}

impl Quarter {
    pub const fn turned(self, clockwise: bool) -> Self {
        match (self, clockwise) {
            (Self::None, true) | (Self::Half, false) => Self::Right,
            (Self::Right, true) | (Self::Left, false) => Self::Half,
            (Self::Half, true) | (Self::None, false) => Self::Left,
            (Self::Left, true) | (Self::Right, false) => Self::None,
        }
    }

    pub const fn is_sideways(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
}

impl From<Quarter> for u8 {
    fn from(quarter: Quarter) -> Self {
        quarter as Self
    }
}

#[derive(Debug, thiserror::Error)]
#[error("a quarter turn is 0, 1, 2 or 3, not {0}")]
pub struct InvalidQuarter(u8);

impl TryFrom<u8> for Quarter {
    type Error = InvalidQuarter;

    fn try_from(value: u8) -> Result<Self, InvalidQuarter> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Right),
            2 => Ok(Self::Half),
            3 => Ok(Self::Left),
            other => Err(InvalidQuarter(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Edits {
    pub crop: Crop,
    pub aspect: Aspect,
    pub angle: f64,
    pub quarter: Quarter,
    pub flip: bool,
    pub adjust: Adjustments,
}

impl Default for Edits {
    fn default() -> Self {
        Self {
            crop: Crop::FULL,
            aspect: Aspect::Free,
            angle: 0.0,
            quarter: Quarter::None,
            flip: false,
            adjust: Adjustments::default(),
        }
    }
}

impl Edits {
    pub fn has_geometry(&self) -> bool {
        self.angle != 0.0 || self.quarter != Quarter::None || self.flip || self.crop != Crop::FULL
    }

    pub fn is_edited(&self) -> bool {
        self.has_geometry() || !self.adjust.is_neutral()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PhotoMeta {
    pub checked: bool,
    pub archived: bool,
    #[serde(default)]
    pub category: Option<CategoryId>,
    pub edits: Edits,
}

impl PhotoMeta {
    pub fn is_default(&self) -> bool {
        !self.checked && !self.archived && self.category.is_none() && !self.edits.is_edited()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Post {
    pub id: PostId,
    pub category: CategoryId,
    pub items: Vec<PhotoId>,
    pub caption: String,
    pub sent: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum App {
    #[serde(rename = "tgprep")]
    Tgprep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct FormatVersion;

#[derive(Debug, thiserror::Error)]
#[error("unsupported state version {0}")]
pub struct UnsupportedVersion(u8);

impl TryFrom<u8> for FormatVersion {
    type Error = UnsupportedVersion;

    fn try_from(value: u8) -> Result<Self, UnsupportedVersion> {
        if value == 1 {
            Ok(Self)
        } else {
            Err(UnsupportedVersion(value))
        }
    }
}

impl From<FormatVersion> for u8 {
    fn from(_: FormatVersion) -> Self {
        1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawDoc")]
pub struct Doc {
    pub app: App,
    pub version: FormatVersion,
    pub categories: Vec<Category>,
    pub posts: Vec<Post>,
    pub photos: IndexMap<PhotoId, PhotoMeta>,
}

#[derive(Deserialize)]
struct RawDoc {
    app: App,
    version: FormatVersion,
    categories: Vec<Category>,
    posts: Vec<Post>,
    photos: IndexMap<PhotoId, PhotoMeta>,
}

#[derive(Debug, thiserror::Error)]
#[error("a state needs at least one category")]
pub struct NoCategories;

impl TryFrom<RawDoc> for Doc {
    type Error = NoCategories;

    fn try_from(raw: RawDoc) -> Result<Self, NoCategories> {
        if raw.categories.is_empty() {
            return Err(NoCategories);
        }
        Ok(Self {
            app: raw.app,
            version: raw.version,
            categories: raw.categories,
            posts: raw.posts,
            photos: raw.photos,
        })
    }
}

pub const CATEGORY_COLORS: [&str; 6] = [
    "#3390ec", "#e5487d", "#30a46c", "#f5a524", "#8e4ec6", "#12a594",
];

static DEFAULT_META: PhotoMeta = PhotoMeta {
    checked: false,
    archived: false,
    category: None,
    edits: Edits {
        crop: Crop::FULL,
        aspect: Aspect::Free,
        angle: 0.0,
        quarter: Quarter::None,
        flip: false,
        adjust: Adjustments([0.0; 13]),
    },
};

impl Default for Doc {
    fn default() -> Self {
        Self {
            app: App::Tgprep,
            version: FormatVersion,
            categories: vec![
                Category {
                    id: CategoryId("friends".into()),
                    name: "Друзьям".into(),
                    color: CATEGORY_COLORS[0].into(),
                },
                Category {
                    id: CategoryId("family".into()),
                    name: "Маме".into(),
                    color: CATEGORY_COLORS[1].into(),
                },
            ],
            posts: Vec::new(),
            photos: IndexMap::new(),
        }
    }
}

impl Doc {
    pub fn parse(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        crate::json::stringify(self)
    }

    pub fn meta(&self, id: &PhotoId) -> &PhotoMeta {
        self.photos.get(id).unwrap_or(&DEFAULT_META)
    }

    pub fn first_category(&self) -> &Category {
        &self.categories[0]
    }

    pub fn category(&self, id: &CategoryId) -> Option<&Category> {
        self.categories.iter().find(|category| &category.id == id)
    }

    pub fn photo_category(&self, id: &PhotoId) -> &CategoryId {
        self.meta(id)
            .category
            .as_ref()
            .unwrap_or(&self.first_category().id)
    }

    pub fn post(&self, id: &PostId) -> Option<&Post> {
        self.posts.iter().find(|post| &post.id == id)
    }

    pub fn post_index(&self, id: &PostId) -> Option<usize> {
        self.posts.iter().position(|post| &post.id == id)
    }
}
