use std::collections::HashMap;
use tgprep_model::doc::{CategoryId, Doc, MAX_POST_PHOTOS, PhotoId, Post, PostId};
use tgprep_model::ops::{self, PhotoDestination};

fn id(name: &str) -> PhotoId {
    PhotoId(name.into())
}

fn ids(names: &[&str]) -> Vec<PhotoId> {
    names.iter().map(|name| id(name)).collect()
}

fn category(name: &str) -> CategoryId {
    CategoryId(name.into())
}

fn post(number: usize, category_name: &str, items: &[&str], caption: &str) -> Post {
    Post { caption: caption.into(), ..ops::create_post(PostId(format!("p{number}")), ids(items), category(category_name)) }
}

fn doc(posts: Vec<Post>) -> Doc {
    Doc { posts, ..Doc::default() }
}

fn shape(doc: &Doc) -> Vec<(String, Vec<String>)> {
    doc.posts.iter().map(|post| (post.category.0.clone(), post.items.iter().map(|item| item.0.clone()).collect())).collect()
}

fn expected(rows: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    rows.iter().map(|(category, items)| ((*category).to_owned(), items.iter().map(|item| (*item).to_owned()).collect())).collect()
}

fn fresh_ids() -> impl FnMut() -> PostId {
    let mut counter = 100;
    move || {
        counter += 1;
        PostId(format!("p{counter}"))
    }
}

fn post_id(state: &Doc, index: usize) -> PostId {
    state.posts[index].id.clone()
}

#[test]
fn moves_a_photo_between_posts_of_a_category_and_prunes_the_emptied_post() {
    let mut state = doc(vec![post(0, "friends", &["a"], ""), post(1, "friends", &["b"], "")]);
    let (target, source) = (post_id(&state, 1), post_id(&state, 0));
    ops::place_photos(&mut state, &ids(&["a"]), PhotoDestination::Post { post: target, index: 0 }, Some(&source), fresh_ids());
    assert_eq!(shape(&state), expected(&[("friends", &["a", "b"])]));
}

#[test]
fn keeps_an_emptied_post_that_has_a_caption() {
    let mut state = doc(vec![post(0, "friends", &["a"], "keep me"), post(1, "friends", &["b"], "")]);
    let (target, source) = (post_id(&state, 1), post_id(&state, 0));
    ops::place_photos(&mut state, &ids(&["a"]), PhotoDestination::Post { post: target, index: 1 }, Some(&source), fresh_ids());
    assert_eq!(shape(&state), expected(&[("friends", &[]), ("friends", &["b", "a"])]));
}

#[test]
fn lets_a_library_photo_join_another_category_without_leaving_the_first_one() {
    let mut state = doc(vec![post(0, "friends", &["a"], ""), post(1, "family", &["b"], "")]);
    let target = post_id(&state, 1);
    ops::place_photos(&mut state, &ids(&["a"]), PhotoDestination::Post { post: target, index: 1 }, None, fresh_ids());
    assert_eq!(shape(&state), expected(&[("friends", &["a"]), ("family", &["b", "a"])]));
}

#[test]
fn splits_a_large_drop_into_albums_of_ten() {
    let names: Vec<String> = (0..23).map(|index| format!("p{index}")).collect();
    let photos: Vec<PhotoId> = names.iter().map(|name| id(name)).collect();
    let mut state = doc(Vec::new());
    ops::place_photos(&mut state, &photos, PhotoDestination::Gap { index: 0, category: category("family") }, None, fresh_ids());
    assert_eq!(state.posts.iter().map(|post| post.items.len()).collect::<Vec<_>>(), [MAX_POST_PHOTOS, MAX_POST_PHOTOS, 3]);
    assert!(state.posts.iter().all(|post| post.category == category("family")));
}

#[test]
fn inserts_before_the_first_remaining_photo_when_moving_within_a_post() {
    let mut state = doc(vec![post(0, "friends", &["a", "b", "c", "d"], "")]);
    let target = post_id(&state, 0);
    ops::place_photos(&mut state, &ids(&["a"]), PhotoDestination::Post { post: target.clone(), index: 3 }, Some(&target), fresh_ids());
    assert_eq!(shape(&state), expected(&[("friends", &["b", "c", "a", "d"])]));
}

#[test]
fn inserts_a_gap_drop_before_the_first_surviving_post() {
    let mut state = doc(vec![post(0, "friends", &["a"], ""), post(1, "friends", &["b"], ""), post(2, "friends", &["c"], "")]);
    let source = post_id(&state, 1);
    ops::place_photos(&mut state, &ids(&["b"]), PhotoDestination::Gap { index: 1, category: category("friends") }, Some(&source), fresh_ids());
    assert_eq!(shape(&state), expected(&[("friends", &["a"]), ("friends", &["b"]), ("friends", &["c"])]));
    assert_eq!(state.posts[1].id, PostId("p101".into()));
}

#[test]
fn moves_a_post_to_a_gap_index() {
    let mut state = doc(vec![post(0, "friends", &["a"], ""), post(1, "friends", &["b"], ""), post(2, "friends", &["c"], "")]);
    let first = post_id(&state, 0);
    ops::move_post(&mut state, &first, 2);
    assert_eq!(shape(&state).into_iter().map(|(_, items)| items[0].clone()).collect::<Vec<_>>(), ["b", "a", "c"]);
    let last = post_id(&state, 2);
    ops::move_post(&mut state, &last, 0);
    assert_eq!(shape(&state).into_iter().map(|(_, items)| items[0].clone()).collect::<Vec<_>>(), ["c", "b", "a"]);
}

#[test]
fn removes_duplicates_from_other_posts_when_a_post_changes_category() {
    let mut state = doc(vec![post(0, "friends", &["a", "b"], ""), post(1, "family", &["a", "c"], "")]);
    let first = post_id(&state, 0);
    ops::set_post_category(&mut state, &first, &category("family"));
    assert_eq!(shape(&state), expected(&[("family", &["a", "b"]), ("family", &["c"])]));
}

#[test]
fn sorts_posts_by_the_median_time_of_their_photos_and_keeps_empty_posts_after_their_predecessor() {
    let times: HashMap<PhotoId, f64> = ["a", "b", "c", "d", "e", "f"].iter().zip(1..).map(|(name, time)| (id(name), f64::from(time))).collect();
    let mut state = doc(vec![post(0, "friends", &["e", "f"], ""), post(1, "friends", &[], ""), post(2, "family", &["a", "f", "b"], ""), post(3, "friends", &["c"], "")]);
    ops::sort_posts_by_time(&mut state, &times);
    assert_eq!(shape(&state).into_iter().map(|(_, items)| items).collect::<Vec<_>>(), expected(&[("", &["a", "f", "b"]), ("", &["c"]), ("", &["e", "f"]), ("", &[])]).into_iter().map(|(_, items)| items).collect::<Vec<_>>());
}

#[test]
fn moves_posts_of_a_deleted_category_to_the_first_remaining_one() {
    let mut state = doc(vec![post(0, "family", &["a"], "")]);
    ops::delete_category(&mut state, &category("family"));
    assert_eq!(state.categories.iter().map(|category| category.id.0.as_str()).collect::<Vec<_>>(), ["friends"]);
    assert_eq!(shape(&state), expected(&[("friends", &["a"])]));
    ops::delete_category(&mut state, &category("friends"));
    assert_eq!(state.categories.len(), 1);
}

#[test]
fn treats_uncategorized_photos_as_the_first_category_and_stores_only_other_ones() {
    let mut state = doc(Vec::new());
    ops::set_photo_category(&mut state, &ids(&["a"]), &category("family"));
    assert_eq!(state.photo_category(&id("a")), &category("family"));
    assert_eq!(state.photo_category(&id("b")), &category("friends"));
    ops::set_photo_category(&mut state, &ids(&["a"]), &category("friends"));
    assert!(state.photos.is_empty());
}

#[test]
fn returns_photos_of_a_deleted_category_to_the_first_one() {
    let mut state = doc(Vec::new());
    ops::set_photo_category(&mut state, &ids(&["a"]), &category("family"));
    ops::delete_category(&mut state, &category("family"));
    assert!(state.photos.is_empty());
}

#[test]
fn archiving_removes_photos_from_every_post() {
    let mut state = doc(vec![post(0, "friends", &["a", "b"], ""), post(1, "family", &["a"], "")]);
    ops::set_archived(&mut state, &ids(&["a"]), true);
    assert_eq!(shape(&state), expected(&[("friends", &["b"])]));
    assert!(state.meta(&id("a")).archived);
    ops::set_archived(&mut state, &ids(&["a"]), false);
    assert!(state.photos.is_empty());
}

#[test]
fn counts_photos_already_in_a_post_once_when_checking_its_capacity() {
    let names: Vec<String> = (0..MAX_POST_PHOTOS).map(|index| format!("p{index}")).collect();
    let full = post(0, "friends", &names.iter().map(String::as_str).collect::<Vec<_>>(), "");
    assert!(ops::can_place_in_post(&full, &ids(&["p3"])));
    assert!(!ops::can_place_in_post(&full, &ids(&["new"])));
}

#[test]
fn indexes_the_posts_each_photo_is_in() {
    let state = doc(vec![post(0, "friends", &["a", "b"], ""), post(1, "family", &["a"], "")]);
    let placements = ops::placements_by_photo(&state.posts);
    assert_eq!(placements[&id("a")], [0, 1]);
    assert_eq!(placements[&id("b")], [0]);
    assert_eq!(placements.len(), 2);
}
