import { produce } from 'immer'
import { describe, expect, it } from 'vitest'
import { createPost, deleteCategory, movePost, placePhotos, setPhotoCategory, setPostCategory, sortPostsByTime } from '../../src/model/ops'
import { EMPTY_DOC, photoCategory, type Doc, type Post } from '../../src/model/types'

const doc = (posts: Post[]): Doc => ({ ...EMPTY_DOC, posts })
const post = (category: string, items: string[], caption = '') => ({ ...createPost(items, category), caption })
const shape = (state: Doc) => state.posts.map((candidate) => [candidate.category, candidate.items])

describe('placePhotos', () => {
  it('moves a photo between posts of a category and prunes the emptied post', () => {
    const state = doc([post('friends', ['a']), post('friends', ['b'])])
    const next = produce(state, (draft) => placePhotos(draft, ['a'], { kind: 'post', postId: state.posts[1].id, index: 0 }, state.posts[0].id))
    expect(shape(next)).toEqual([['friends', ['a', 'b']]])
  })

  it('keeps an emptied post that has a caption', () => {
    const state = doc([post('friends', ['a'], 'keep me'), post('friends', ['b'])])
    const next = produce(state, (draft) => placePhotos(draft, ['a'], { kind: 'post', postId: state.posts[1].id, index: 1 }, state.posts[0].id))
    expect(shape(next)).toEqual([
      ['friends', []],
      ['friends', ['b', 'a']],
    ])
  })

  it('lets a library photo join another category without leaving the first one', () => {
    const state = doc([post('friends', ['a']), post('family', ['b'])])
    const next = produce(state, (draft) => placePhotos(draft, ['a'], { kind: 'post', postId: state.posts[1].id, index: 1 }, null))
    expect(shape(next)).toEqual([
      ['friends', ['a']],
      ['family', ['b', 'a']],
    ])
  })

  it('splits a large drop into albums of ten', () => {
    const ids = Array.from({ length: 23 }, (_, index) => `p${index}`)
    const next = produce(doc([]), (draft) => placePhotos(draft, ids, { kind: 'gap', index: 0, category: 'family' }, null))
    expect(next.posts.map((candidate) => candidate.items.length)).toEqual([10, 10, 3])
    expect(new Set(next.posts.map((candidate) => candidate.category))).toEqual(new Set(['family']))
  })

  it('inserts before the first remaining photo when moving within a post', () => {
    const state = doc([post('friends', ['a', 'b', 'c', 'd'])])
    const next = produce(state, (draft) => placePhotos(draft, ['a'], { kind: 'post', postId: state.posts[0].id, index: 3 }, state.posts[0].id))
    expect(shape(next)).toEqual([['friends', ['b', 'c', 'a', 'd']]])
  })
})

describe('post order and categories', () => {
  it('moves a post to a gap index', () => {
    const state = doc([post('friends', ['a']), post('friends', ['b']), post('friends', ['c'])])
    const next = produce(state, (draft) => movePost(draft, state.posts[0].id, 2))
    expect(next.posts.map((candidate) => candidate.items[0])).toEqual(['b', 'a', 'c'])
  })

  it('removes duplicates from other posts when a post changes category', () => {
    const state = doc([post('friends', ['a', 'b']), post('family', ['a', 'c'])])
    const next = produce(state, (draft) => setPostCategory(draft, state.posts[0].id, 'family'))
    expect(shape(next)).toEqual([
      ['family', ['a', 'b']],
      ['family', ['c']],
    ])
  })

  it('sorts posts by the median time of their photos and keeps empty posts after their predecessor', () => {
    const times = new Map(['a', 'b', 'c', 'd', 'e', 'f'].map((id, index) => [id, index + 1]))
    const state = doc([post('friends', ['e', 'f']), post('friends', []), post('family', ['a', 'f', 'b']), post('friends', ['c'])])
    const next = produce(state, (draft) => sortPostsByTime(draft, times))
    expect(next.posts.map((candidate) => candidate.items)).toEqual([['a', 'f', 'b'], ['c'], ['e', 'f'], []])
  })

  it('moves posts of a deleted category to the first remaining one', () => {
    const state = doc([post('family', ['a'])])
    const next = produce(state, (draft) => deleteCategory(draft, 'family'))
    expect(next.categories.map((category) => category.id)).toEqual(['friends'])
    expect(shape(next)).toEqual([['friends', ['a']]])
  })
})

describe('photo categories', () => {
  it('treats uncategorized photos as the first category and stores only other ones', () => {
    const family = produce(doc([]), (draft) => setPhotoCategory(draft, ['a'], 'family'))
    expect(photoCategory(family, 'a')).toBe('family')
    expect(photoCategory(family, 'b')).toBe('friends')
    expect(produce(family, (draft) => setPhotoCategory(draft, ['a'], 'friends')).photos).toEqual({})
  })

  it('returns photos of a deleted category to the first one', () => {
    const family = produce(doc([]), (draft) => setPhotoCategory(draft, ['a'], 'family'))
    expect(produce(family, (draft) => deleteCategory(draft, 'family')).photos).toEqual({})
  })
})
