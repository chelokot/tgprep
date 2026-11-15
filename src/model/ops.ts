import { CATEGORY_COLORS, DEFAULT_META, isEdited, MAX_POST_PHOTOS, metaOf, type Doc, type PhotoMeta, type Post } from './types'

export type PhotoDestination = { kind: 'post'; postId: string; index: number } | { kind: 'gap'; index: number; category: string }

export const createPost = (items: string[], category: string): Post => ({ id: crypto.randomUUID(), category, items, caption: '', sent: false })

const firstSurvivor = (order: string[], from: number, survives: (id: string) => boolean) => order.slice(from).find(survives) ?? null

const detach = (doc: Doc, moving: Set<string>, inScope: (post: Post) => boolean, keepPostId: string | null) => {
  const emptied = new Set<string>()
  for (const post of doc.posts) {
    if (!inScope(post)) continue
    const remaining = post.items.filter((id) => !moving.has(id))
    if (remaining.length === post.items.length) continue
    post.items = remaining
    if (remaining.length === 0) emptied.add(post.id)
  }
  doc.posts = doc.posts.filter((post) => post.id === keepPostId || !emptied.has(post.id) || post.caption.trim() !== '')
}

export const placePhotos = (doc: Doc, ids: string[], destination: PhotoDestination, sourcePostId: string | null) => {
  const moving = new Set(ids)
  const postOrder = doc.posts.map((post) => post.id)

  if (destination.kind === 'post') {
    const target = doc.posts.find((post) => post.id === destination.postId)
    if (!target) return
    const anchor = firstSurvivor(target.items, destination.index, (id) => !moving.has(id))
    detach(doc, moving, (post) => post.category === target.category || post.id === sourcePostId, target.id)
    const position = anchor === null ? target.items.length : target.items.indexOf(anchor)
    target.items.splice(position, 0, ...ids)
    return
  }

  detach(doc, moving, (post) => post.category === destination.category || post.id === sourcePostId, null)
  const surviving = new Set(doc.posts.map((post) => post.id))
  const anchor = firstSurvivor(postOrder, destination.index, (id) => surviving.has(id))
  const position = anchor === null ? doc.posts.length : doc.posts.findIndex((post) => post.id === anchor)
  const created: Post[] = []
  for (let start = 0; start < ids.length; start += MAX_POST_PHOTOS) created.push(createPost(ids.slice(start, start + MAX_POST_PHOTOS), destination.category))
  doc.posts.splice(position, 0, ...created)
}

export const unplacePhotos = (doc: Doc, ids: string[], postId: string) => detach(doc, new Set(ids), (post) => post.id === postId, null)

export const movePost = (doc: Doc, postId: string, index: number) => {
  const order = doc.posts.map((post) => post.id)
  const anchor = firstSurvivor(order, index, (id) => id !== postId)
  const [post] = doc.posts.splice(order.indexOf(postId), 1)
  const position = anchor === null ? doc.posts.length : doc.posts.findIndex((candidate) => candidate.id === anchor)
  doc.posts.splice(position, 0, post)
}

export const sortPostsByTime = (doc: Doc, times: Map<string, number>) => {
  let previous = -Infinity
  const keyed = doc.posts.map((post) => {
    const known = post.items.flatMap((id) => times.get(id) ?? []).sort((a, b) => a - b)
    if (known.length > 0) previous = known[Math.floor((known.length - 1) / 2)]
    return { post, time: previous }
  })
  doc.posts = keyed.sort((a, b) => a.time - b.time).map(({ post }) => post)
}

export const insertPost = (doc: Doc, post: Post, index: number) => {
  doc.posts.splice(index, 0, post)
}

export const deletePost = (doc: Doc, postId: string) => {
  doc.posts = doc.posts.filter((post) => post.id !== postId)
}

export const updatePost = (doc: Doc, postId: string, patch: Partial<Pick<Post, 'caption' | 'sent'>>) => {
  const post = doc.posts.find((candidate) => candidate.id === postId)
  if (post) Object.assign(post, patch)
}

export const setPostCategory = (doc: Doc, postId: string, category: string) => {
  const post = doc.posts.find((candidate) => candidate.id === postId)
  if (!post) return
  post.category = category
  detach(doc, new Set(post.items), (other) => other.category === category && other.id !== postId, null)
}

export const addCategory = (doc: Doc, name: string) => {
  const color = CATEGORY_COLORS[doc.categories.length % CATEGORY_COLORS.length]
  doc.categories.push({ id: crypto.randomUUID(), name, color })
}

export const renameCategory = (doc: Doc, id: string, name: string) => {
  const category = doc.categories.find((candidate) => candidate.id === id)
  if (category) category.name = name
}

export const deleteCategory = (doc: Doc, id: string) => {
  if (doc.categories.length === 1) return
  doc.categories = doc.categories.filter((category) => category.id !== id)
  const fallback = doc.categories[0].id
  for (const post of doc.posts.filter((candidate) => candidate.category === id)) setPostCategory(doc, post.id, fallback)
  updateMeta(doc, Object.keys(doc.photos).filter((photo) => doc.photos[photo].category === id), { category: null })
}

export const updateMeta = (doc: Doc, ids: string[], patch: Partial<PhotoMeta>) => {
  for (const id of ids) {
    const meta = { ...metaOf(doc, id), ...patch }
    if (meta.checked === DEFAULT_META.checked && meta.archived === DEFAULT_META.archived && meta.category === DEFAULT_META.category && !isEdited(meta.edits))
      delete doc.photos[id]
    else doc.photos[id] = meta
  }
}

export const setPhotoCategory = (doc: Doc, ids: string[], category: string) =>
  updateMeta(doc, ids, { category: category === doc.categories[0].id ? null : category })

export const setArchived = (doc: Doc, ids: string[], archived: boolean) => {
  updateMeta(doc, ids, { archived })
  if (archived) detach(doc, new Set(ids), () => true, null)
}

export const canPlaceInPost = (post: Post, ids: string[]) => {
  const moving = new Set(ids)
  return post.items.filter((id) => !moving.has(id)).length + ids.length <= MAX_POST_PHOTOS
}

const placementCache = new WeakMap<Post[], Map<string, number[]>>()

export const placementsByPhoto = (posts: Post[]) => {
  const cached = placementCache.get(posts)
  if (cached) return cached
  const placements = new Map<string, number[]>()
  posts.forEach((post, index) => post.items.forEach((id) => placements.set(id, [...(placements.get(id) ?? []), index])))
  placementCache.set(posts, placements)
  return placements
}
