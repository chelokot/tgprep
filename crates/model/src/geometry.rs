use crate::doc::{Aspect, Crop, Edits, Quarter};

const EPSILON: f64 = 1e-6;
const SEARCH_STEPS: usize = 24;
const MIN_CROP_PX: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    #[must_use]
    pub fn multiply(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    #[must_use]
    pub fn translate(self, x: f64, y: f64) -> Self {
        self.multiply(Self {
            e: x,
            f: y,
            ..Self::IDENTITY
        })
    }

    #[must_use]
    pub fn scale(self, x: f64, y: f64) -> Self {
        self.multiply(Self {
            a: x,
            d: y,
            ..Self::IDENTITY
        })
    }

    #[must_use]
    pub fn rotate(self, degrees: f64) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        self.multiply(Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            ..Self::IDENTITY
        })
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

pub fn frame_size(source: Size, quarter: Quarter) -> Size {
    if quarter.is_sideways() {
        Size {
            w: source.h,
            h: source.w,
        }
    } else {
        source
    }
}

pub fn output_size(source: Size, edits: &Edits) -> Size {
    let frame = frame_size(source, edits.quarter);
    Size {
        w: edits.crop.w * frame.w,
        h: edits.crop.h * frame.h,
    }
}

pub fn fit_within(size: Size, long_side: f64) -> Size {
    let scale = (long_side / size.w.max(size.h)).min(1.0);
    Size {
        w: (size.w * scale).round().max(1.0),
        h: (size.h * scale).round().max(1.0),
    }
}

fn quarter_inverse(source: Size, quarter: Quarter) -> Matrix {
    match quarter {
        Quarter::None => Matrix::IDENTITY,
        Quarter::Right => Matrix {
            a: 0.0,
            b: -1.0,
            c: 1.0,
            d: 0.0,
            e: 0.0,
            f: source.h,
        },
        Quarter::Half => Matrix {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            e: source.w,
            f: source.h,
        },
        Quarter::Left => Matrix {
            a: 0.0,
            b: 1.0,
            c: -1.0,
            d: 0.0,
            e: source.w,
            f: 0.0,
        },
    }
}

pub fn frame_to_source_uv(source: Size, edits: &Edits) -> Matrix {
    let frame = frame_size(source, edits.quarter);
    let rotated = Matrix::IDENTITY
        .scale(1.0 / source.w, 1.0 / source.h)
        .multiply(quarter_inverse(source, edits.quarter));
    let flipped = if edits.flip {
        rotated.translate(frame.w, 0.0).scale(-1.0, 1.0)
    } else {
        rotated
    };
    flipped
        .translate(frame.w / 2.0, frame.h / 2.0)
        .rotate(-edits.angle)
        .translate(-frame.w / 2.0, -frame.h / 2.0)
}

pub fn output_to_source_uv(source: Size, edits: &Edits, crop: Crop) -> Matrix {
    let frame = frame_size(source, edits.quarter);
    frame_to_source_uv(source, edits)
        .translate(crop.x * frame.w, crop.y * frame.h)
        .scale(crop.w * frame.w, crop.h * frame.h)
}

pub fn crop_fits(source: Size, edits: &Edits, crop: Crop) -> bool {
    let frame = frame_size(source, edits.quarter);
    let minimum = MIN_CROP_PX - EPSILON;
    if crop.w * frame.w < minimum || crop.h * frame.h < minimum {
        return false;
    }
    let to_source = frame_to_source_uv(source, edits);
    [
        (crop.x, crop.y),
        (crop.x + crop.w, crop.y),
        (crop.x, crop.y + crop.h),
        (crop.x + crop.w, crop.y + crop.h),
    ]
    .into_iter()
    .all(|(x, y)| {
        let (u, v) = to_source.apply(x * frame.w, y * frame.h);
        (-EPSILON..=1.0 + EPSILON).contains(&u) && (-EPSILON..=1.0 + EPSILON).contains(&v)
    })
}

fn lerp_crop(from: Crop, to: Crop, t: f64) -> Crop {
    Crop {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
        w: from.w + (to.w - from.w) * t,
        h: from.h + (to.h - from.h) * t,
    }
}

fn furthest_fitting(fits: impl Fn(Crop) -> bool, from: Crop, to: Crop) -> Crop {
    if fits(to) {
        return to;
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..SEARCH_STEPS {
        let middle = f64::midpoint(low, high);
        if fits(lerp_crop(from, to, middle)) {
            low = middle;
        } else {
            high = middle;
        }
    }
    lerp_crop(from, to, low)
}

pub fn constrain_crop(source: Size, edits: &Edits, from: Crop, to: Crop) -> Crop {
    furthest_fitting(|crop| crop_fits(source, edits, crop), from, to)
}

pub fn move_crop(source: Size, edits: &Edits, from: Crop, dx: f64, dy: f64) -> Crop {
    let horizontal = constrain_crop(
        source,
        edits,
        from,
        Crop {
            x: from.x + dx,
            ..from
        },
    );
    constrain_crop(
        source,
        edits,
        horizontal,
        Crop {
            y: horizontal.y + dy,
            ..horizontal
        },
    )
}

fn centered(center_x: f64, center_y: f64, w: f64, h: f64) -> Crop {
    Crop {
        x: center_x - w / 2.0,
        y: center_y - h / 2.0,
        w,
        h,
    }
}

fn largest_centered(
    source: Size,
    edits: &Edits,
    center_x: f64,
    center_y: f64,
    w: f64,
    h: f64,
) -> Crop {
    furthest_fitting(
        |crop| crop_fits(source, edits, crop),
        centered(center_x, center_y, w * EPSILON, h * EPSILON),
        centered(center_x, center_y, w, h),
    )
}

pub fn aspect_ratio(aspect: Aspect, source: Size) -> Option<f64> {
    match (aspect, aspect.ratio()) {
        (Aspect::Original, _) => Some(source.w / source.h),
        (_, Some((w, h))) => Some(f64::from(w) / f64::from(h)),
        (_, None) => None,
    }
}

pub fn fit_crop(source: Size, edits: &Edits, base: Crop, ratio: Option<f64>) -> Crop {
    let frame = frame_size(source, edits.quarter);
    let (width, height, center_x, center_y) = match ratio {
        None => (
            base.w * frame.w,
            base.h * frame.h,
            base.x + base.w / 2.0,
            base.y + base.h / 2.0,
        ),
        Some(ratio) => {
            let width = frame.w.min(frame.h * ratio);
            (width, width / ratio, 0.5, 0.5)
        }
    };
    let fitted = largest_centered(
        source,
        edits,
        center_x,
        center_y,
        width / frame.w,
        height / frame.h,
    );
    if crop_fits(source, edits, fitted) {
        fitted
    } else {
        largest_centered(source, edits, 0.5, 0.5, width / frame.w, height / frame.h)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    pub x: Side,
    pub y: Side,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self {
            x: Side::Start,
            y: Side::Start,
        },
        Self {
            x: Side::Middle,
            y: Side::Start,
        },
        Self {
            x: Side::End,
            y: Side::Start,
        },
        Self {
            x: Side::End,
            y: Side::Middle,
        },
        Self {
            x: Side::End,
            y: Side::End,
        },
        Self {
            x: Side::Middle,
            y: Side::End,
        },
        Self {
            x: Side::Start,
            y: Side::End,
        },
        Self {
            x: Side::Start,
            y: Side::Middle,
        },
    ];
}

fn span(side: Side, start: f64, length: f64, pointer: f64) -> (f64, f64) {
    match side {
        Side::Start => (pointer.min(start + length), start + length),
        Side::Middle => (start, start + length),
        Side::End => (start, pointer.max(start)),
    }
}

fn anchored(side: Side, start: f64, length: f64, extent: f64) -> f64 {
    match side {
        Side::End => start,
        Side::Start => start + length - extent,
        Side::Middle => start + length / 2.0 - extent / 2.0,
    }
}

pub fn resize_crop(
    source: Size,
    edits: &Edits,
    start: Crop,
    handle: Handle,
    pointer_x: f64,
    pointer_y: f64,
    ratio: Option<f64>,
) -> Crop {
    let frame = frame_size(source, edits.quarter);
    let (left, right) = span(handle.x, start.x, start.w, pointer_x);
    let (top, bottom) = span(handle.y, start.y, start.h, pointer_y);
    let Some(ratio) = ratio else {
        return constrain_crop(
            source,
            edits,
            start,
            Crop {
                x: left,
                y: top,
                w: right - left,
                h: bottom - top,
            },
        );
    };
    let normalized = ratio * frame.h / frame.w;
    let wide = handle.y == Side::Middle
        || (handle.x != Side::Middle && (right - left) / (bottom - top) > normalized);
    let (w, h) = if wide {
        (right - left, (right - left) / normalized)
    } else {
        ((bottom - top) * normalized, bottom - top)
    };
    let x = anchored(handle.x, start.x, start.w, w);
    let y = anchored(handle.y, start.y, start.h, h);
    constrain_crop(source, edits, start, Crop { x, y, w, h })
}
