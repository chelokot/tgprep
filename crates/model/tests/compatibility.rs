use serde::Deserialize;
use std::collections::HashMap;
use tgprep_model::doc::{Adjustment, CategoryId, Crop, Doc, Edits, PhotoId, PostId};
use tgprep_model::geometry::{self, Handle, Side, Size};
use tgprep_model::ops::{self, PhotoDestination};

const INPUT: &str = include_str!("fixtures/v1-input.json");
const PARSED: &str = include_str!("fixtures/v1-parsed.json");
const WRITTEN: &str = include_str!("fixtures/v1-written.json");
const GEOMETRY: &str = include_str!("fixtures/geometry.json");

fn photos(names: &[&str]) -> Vec<PhotoId> {
    names.iter().map(|name| PhotoId((*name).to_owned())).collect()
}

#[test]
fn reads_a_state_and_writes_it_as_the_typescript_app_did() {
    let doc = Doc::parse(INPUT).expect("the TypeScript input parses");
    assert_eq!(doc.to_json().expect("serializes"), PARSED.trim_end());
    assert_eq!(doc.meta(&PhotoId("trip/x.jpg".into())).category, None);
}

#[test]
fn rewrites_a_state_written_by_the_typescript_app_byte_for_byte() {
    let doc = Doc::parse(WRITTEN).expect("the TypeScript output parses");
    assert_eq!(doc.to_json().expect("serializes"), WRITTEN.trim_end());
}

#[test]
fn rejects_states_the_typescript_schema_rejects() {
    let empty_categories = r#"{"app":"tgprep","version":1,"categories":[],"posts":[],"photos":{}}"#;
    let other_version = r##"{"app":"tgprep","version":2,"categories":[{"id":"a","name":"A","color":"#fff"}],"posts":[],"photos":{}}"##;
    let missing_adjustment = WRITTEN.replacen(r#""exposure":35,"#, "", 1);
    for text in [empty_categories, other_version, missing_adjustment.as_str()] {
        assert!(Doc::parse(text).is_err(), "{text}");
    }
}

#[test]
fn replays_the_typescript_edit_session_to_the_same_file() {
    let mut doc = Doc::parse(INPUT).expect("parses");
    let mut counter = 0;
    let mut next_id = move || {
        counter += 1;
        format!("uuid-{counter}")
    };
    let mut new_post = || PostId(next_id());
    let friends = CategoryId("friends".into());
    let family = CategoryId("family".into());
    ops::place_photos(&mut doc, &photos(&["a.jpg", "b.jpg", "c.jpg"]), PhotoDestination::Gap { index: 0, category: friends.clone() }, None, &mut new_post);
    let numbered: Vec<String> = (0..12).map(|index| format!("n{index}.jpg")).collect();
    let numbered: Vec<&str> = numbered.iter().map(String::as_str).collect();
    ops::place_photos(&mut doc, &photos(&numbered), PhotoDestination::Gap { index: 1, category: family.clone() }, None, &mut new_post);
    let second = doc.posts[1].id.clone();
    ops::place_photos(&mut doc, &photos(&["a.jpg"]), PhotoDestination::Post { post: second.clone(), index: 1 }, None, &mut new_post);
    ops::set_caption(&mut doc, &second, "Подпись".into());
    ops::set_photo_category(&mut doc, &photos(&["a.jpg", "n1.jpg"]), &family);
    ops::set_photo_category(&mut doc, &photos(&["a.jpg"]), &friends);
    ops::update_meta(&mut doc, &photos(&["n3.jpg"]), |meta| meta.checked = true);
    ops::update_meta(&mut doc, &photos(&["c.jpg"]), |meta| meta.archived = false);
    ops::set_archived(&mut doc, &photos(&["n4.jpg"]), true);
    ops::add_category(&mut doc, CategoryId("uuid-4".into()), "Работа".into());
    ops::rename_category(&mut doc, &family, "Мама".into());
    let first = doc.posts[0].id.clone();
    ops::move_post(&mut doc, &first, 3);
    let first = doc.posts[0].id.clone();
    ops::set_post_category(&mut doc, &first, &CategoryId("uuid-1".into()));
    ops::update_meta(&mut doc, &photos(&["b.jpg"]), |meta| {
        meta.edits = Edits { angle: -12.700_000_000_000_001, ..Edits::default() };
        meta.edits.adjust[Adjustment::Exposure] = 35.0;
        meta.edits.adjust[Adjustment::Grain] = 100.0;
        meta.edits.adjust[Adjustment::Vignette] = -20.0;
    });
    let times: HashMap<PhotoId, f64> =
        [("a.jpg", 5.0), ("b.jpg", 1.0), ("n0.jpg", 3.0), ("n10.jpg", 9.0), ("trip/x.jpg", 7.0)].into_iter().map(|(id, time)| (PhotoId(id.into()), time)).collect();
    ops::sort_posts_by_time(&mut doc, &times);
    ops::delete_category(&mut doc, &CategoryId("uuid-1".into()));
    ops::insert_post(&mut doc, ops::create_post(PostId("uuid-5".into()), Vec::new(), family), 2);
    let last = doc.posts[doc.posts.len() - 1].id.clone();
    ops::delete_post(&mut doc, &last);
    assert_eq!(doc.to_json().expect("serializes"), WRITTEN.trim_end());
}

#[derive(Deserialize)]
struct SizeFixture {
    w: f64,
    h: f64,
}

#[derive(Deserialize)]
struct HandleFixture {
    x: i8,
    y: i8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    source: SizeFixture,
    edits: Edits,
    crop: Crop,
    matrix: [f64; 6],
    fits: bool,
    fit_free: Crop,
    fit_ratio: Crop,
    moved: Crop,
    resized: Vec<[Crop; 2]>,
}

#[derive(Deserialize)]
struct GeometryFixture {
    handles: Vec<HandleFixture>,
    cases: Vec<Case>,
}

fn side(value: i8) -> Side {
    match value {
        -1 => Side::Start,
        0 => Side::Middle,
        _ => Side::End,
    }
}

fn assert_close(actual: Crop, expected: Crop, context: &str) {
    for (a, e) in [(actual.x, expected.x), (actual.y, expected.y), (actual.w, expected.w), (actual.h, expected.h)] {
        assert!((a - e).abs() < 1e-6, "{context}: {actual:?} != {expected:?}");
    }
}

#[test]
fn computes_the_same_geometry_as_the_typescript_app() {
    let fixture: GeometryFixture = serde_json::from_str(GEOMETRY).expect("geometry fixture parses");
    let handles: Vec<Handle> = fixture.handles.iter().map(|handle| Handle { x: side(handle.x), y: side(handle.y) }).collect();
    assert_eq!(fixture.cases.len(), 216);
    for (number, case) in fixture.cases.iter().enumerate() {
        let source = Size { w: case.source.w, h: case.source.h };
        let context = format!("case {number}");
        let matrix = geometry::output_to_source_uv(source, &case.edits, case.crop);
        for (actual, expected) in [matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f].into_iter().zip(case.matrix) {
            assert!((actual - expected).abs() < 1e-6, "{context}: {matrix:?} != {:?}", case.matrix);
        }
        assert_eq!(geometry::crop_fits(source, &case.edits, case.crop), case.fits, "{context}");
        let ratio = geometry::aspect_ratio(tgprep_model::doc::Aspect::Portrait4x5, geometry::frame_size(source, case.edits.quarter));
        assert_close(geometry::fit_crop(source, &case.edits, case.crop, None), case.fit_free, &context);
        assert_close(geometry::fit_crop(source, &case.edits, case.crop, ratio), case.fit_ratio, &context);
        assert_close(geometry::move_crop(source, &case.edits, case.crop, 0.3, -0.15), case.moved, &context);
        for (handle, [free, constrained]) in handles.iter().zip(&case.resized) {
            assert_close(geometry::resize_crop(source, &case.edits, case.crop, *handle, 0.95, 0.02, None), *free, &context);
            assert_close(geometry::resize_crop(source, &case.edits, case.crop, *handle, 0.7, 0.9, ratio), *constrained, &context);
        }
    }
}
