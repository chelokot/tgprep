use crate::doc::{
    CATEGORY_COLORS, Category, CategoryId, Doc, MAX_POST_PHOTOS, PhotoId, PhotoMeta, Post, PostId,
};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

#[derive(Debug, Clone, PartialEq)]
pub enum PhotoDestination {
    Post { post: PostId, index: usize },
    Gap { index: usize, category: CategoryId },
}

pub fn create_post(id: PostId, items: Vec<PhotoId>, category: CategoryId) -> Post {
    Post {
        id,
        category,
        items,
        caption: String::new(),
        sent: false,
    }
}

fn first_survivor<T>(order: &[T], from: usize, survives: impl Fn(&T) -> bool) -> Option<&T> {
    order
        .get(from..)
        .and_then(|rest| rest.iter().find(|item| survives(item)))
}

fn detach(
    doc: &mut Doc,
    moving: &HashSet<&PhotoId>,
    in_scope: impl Fn(&Post) -> bool,
    keep: Option<&PostId>,
) {
    let mut emptied = HashSet::new();
    for post in &mut doc.posts {
        if !in_scope(post) {
            continue;
        }
        let before = post.items.len();
        post.items.retain(|id| !moving.contains(id));
        if post.items.len() != before && post.items.is_empty() {
            emptied.insert(post.id.clone());
        }
    }
    doc.posts.retain(|post| {
        Some(&post.id) == keep || !emptied.contains(&post.id) || !post.caption.trim().is_empty()
    });
}

pub fn place_photos(
    doc: &mut Doc,
    ids: &[PhotoId],
    destination: PhotoDestination,
    source: Option<&PostId>,
    mut new_post_id: impl FnMut() -> PostId,
) {
    let moving: HashSet<&PhotoId> = ids.iter().collect();
    match destination {
        PhotoDestination::Post { post, index } => {
            let Some(target) = doc.post(&post) else {
                return;
            };
            let category = target.category.clone();
            let anchor = first_survivor(&target.items, index, |id| !moving.contains(id)).cloned();
            detach(
                doc,
                &moving,
                |candidate| candidate.category == category || Some(&candidate.id) == source,
                Some(&post),
            );
            let Some(target) = doc.posts.iter_mut().find(|candidate| candidate.id == post) else {
                return;
            };
            let position = anchor
                .and_then(|anchor| target.items.iter().position(|id| *id == anchor))
                .unwrap_or(target.items.len());
            target.items.splice(position..position, ids.iter().cloned());
        }
        PhotoDestination::Gap { index, category } => {
            let order: Vec<PostId> = doc.posts.iter().map(|post| post.id.clone()).collect();
            detach(
                doc,
                &moving,
                |candidate| candidate.category == category || Some(&candidate.id) == source,
                None,
            );
            let surviving: HashSet<&PostId> = doc.posts.iter().map(|post| &post.id).collect();
            let position = first_survivor(&order, index, |id| surviving.contains(id))
                .and_then(|anchor| doc.post_index(anchor))
                .unwrap_or(doc.posts.len());
            let created: Vec<Post> = ids
                .chunks(MAX_POST_PHOTOS)
                .map(|chunk| create_post(new_post_id(), chunk.to_vec(), category.clone()))
                .collect();
            doc.posts.splice(position..position, created);
        }
    }
}

pub fn unplace_photos(doc: &mut Doc, ids: &[PhotoId], post: &PostId) {
    detach(
        doc,
        &ids.iter().collect(),
        |candidate| &candidate.id == post,
        None,
    );
}

pub fn move_post(doc: &mut Doc, post: &PostId, index: usize) {
    let order: Vec<PostId> = doc
        .posts
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect();
    let anchor = first_survivor(&order, index, |id| id != post).cloned();
    let Some(from) = doc.post_index(post) else {
        return;
    };
    let moved = doc.posts.remove(from);
    let position = anchor
        .and_then(|anchor| doc.post_index(&anchor))
        .unwrap_or(doc.posts.len());
    doc.posts.insert(position, moved);
}

pub fn sort_posts_by_time<S: BuildHasher>(doc: &mut Doc, times: &HashMap<PhotoId, f64, S>) {
    let mut previous = f64::NEG_INFINITY;
    let mut keyed: Vec<(f64, Post)> = doc
        .posts
        .drain(..)
        .map(|post| {
            let mut known: Vec<f64> = post
                .items
                .iter()
                .filter_map(|id| times.get(id).copied())
                .collect();
            known.sort_by(f64::total_cmp);
            if !known.is_empty() {
                previous = known[(known.len() - 1) / 2];
            }
            (previous, post)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    doc.posts = keyed.into_iter().map(|(_, post)| post).collect();
}

pub fn insert_post(doc: &mut Doc, post: Post, index: usize) {
    doc.posts.insert(index.min(doc.posts.len()), post);
}

pub fn delete_post(doc: &mut Doc, post: &PostId) {
    doc.posts.retain(|candidate| &candidate.id != post);
}

fn post_mut<'a>(doc: &'a mut Doc, post: &PostId) -> Option<&'a mut Post> {
    doc.posts.iter_mut().find(|candidate| &candidate.id == post)
}

pub fn set_caption(doc: &mut Doc, post: &PostId, caption: String) {
    if let Some(post) = post_mut(doc, post) {
        post.caption = caption;
    }
}

pub fn set_sent(doc: &mut Doc, post: &PostId, sent: bool) {
    if let Some(post) = post_mut(doc, post) {
        post.sent = sent;
    }
}

pub fn set_post_category(doc: &mut Doc, post: &PostId, category: &CategoryId) {
    let Some(target) = post_mut(doc, post) else {
        return;
    };
    target.category = category.clone();
    let items = target.items.clone();
    detach(
        doc,
        &items.iter().collect(),
        |other| &other.category == category && &other.id != post,
        None,
    );
}

pub fn add_category(doc: &mut Doc, id: CategoryId, name: String) {
    let color = CATEGORY_COLORS[doc.categories.len() % CATEGORY_COLORS.len()].to_owned();
    doc.categories.push(Category { id, name, color });
}

pub fn rename_category(doc: &mut Doc, id: &CategoryId, name: String) {
    if let Some(category) = doc
        .categories
        .iter_mut()
        .find(|category| &category.id == id)
    {
        category.name = name;
    }
}

pub fn delete_category(doc: &mut Doc, id: &CategoryId) {
    if doc.categories.len() == 1 {
        return;
    }
    doc.categories.retain(|category| &category.id != id);
    let fallback = doc.first_category().id.clone();
    let moved: Vec<PostId> = doc
        .posts
        .iter()
        .filter(|post| &post.category == id)
        .map(|post| post.id.clone())
        .collect();
    for post in &moved {
        set_post_category(doc, post, &fallback);
    }
    let photos: Vec<PhotoId> = doc
        .photos
        .iter()
        .filter(|(_, meta)| meta.category.as_ref() == Some(id))
        .map(|(photo, _)| photo.clone())
        .collect();
    update_meta(doc, &photos, |meta| meta.category = None);
}

pub fn update_meta(doc: &mut Doc, ids: &[PhotoId], patch: impl Fn(&mut PhotoMeta)) {
    for id in ids {
        let mut meta = doc.meta(id).clone();
        patch(&mut meta);
        if meta.is_default() {
            doc.photos.shift_remove(id);
        } else {
            doc.photos.insert(id.clone(), meta);
        }
    }
}

pub fn set_photo_category(doc: &mut Doc, ids: &[PhotoId], category: &CategoryId) {
    let stored = (category != &doc.first_category().id).then(|| category.clone());
    update_meta(doc, ids, |meta| meta.category.clone_from(&stored));
}

pub fn set_archived(doc: &mut Doc, ids: &[PhotoId], archived: bool) {
    update_meta(doc, ids, |meta| meta.archived = archived);
    if archived {
        detach(doc, &ids.iter().collect(), |_| true, None);
    }
}

pub fn can_place_in_post(post: &Post, ids: &[PhotoId]) -> bool {
    post.items.iter().filter(|id| !ids.contains(id)).count() + ids.len() <= MAX_POST_PHOTOS
}

pub fn placements_by_photo(posts: &[Post]) -> HashMap<PhotoId, Vec<usize>> {
    let mut placements: HashMap<PhotoId, Vec<usize>> = HashMap::new();
    for (index, post) in posts.iter().enumerate() {
        for id in &post.items {
            placements.entry(id.clone()).or_default().push(index);
        }
    }
    placements
}
