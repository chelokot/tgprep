import type { PointerEvent as ReactPointerEvent } from 'react'
import { create } from 'zustand'
import { canPlaceInPost, movePost, placePhotos, unplacePhotos } from '../model/ops'
import { clearSelection, commit, useDoc } from '../model/store'

export type DragPayload = { kind: 'photos'; ids: string[]; fromPostId: string | null } | { kind: 'post'; postId: string }

export type Box = { left: number; top: number; width: number; height: number }

export type DropTarget =
  | { kind: 'post'; postId: string; index: number; valid: boolean; ring: Box; line: Box | null }
  | { kind: 'gap'; index: number; category: string; line: Box }
  | { kind: 'library'; ring: Box }

type Point = { x: number; y: number }

type DragState = { payload: DragPayload | null; pointer: Point; target: DropTarget | null }

export const useDrag = create<DragState>(() => ({ payload: null, pointer: { x: 0, y: 0 }, target: null }))

export type ClickModifiers = { range: boolean; toggle: boolean }

const DRAG_THRESHOLD_PX = 5
const EDGE_BAND_PX = 14
const AUTOSCROLL_ZONE_PX = 72
const AUTOSCROLL_SPEED = 0.03
const LINE_PX = 3

type Card = { element: HTMLElement; index: number; rect: DOMRect }

const boxOf = (rect: DOMRect): Box => ({ left: rect.left, top: rect.top, width: rect.width, height: rect.height })

const gapTarget = (view: HTMLElement, cards: Card[], index: number): DropTarget => {
  const viewRect = view.getBoundingClientRect()
  const before = cards.find((card) => card.index === index - 1)
  const after = cards.find((card) => card.index === index)
  const y = before && after ? (before.rect.bottom + after.rect.top) / 2 : before ? before.rect.bottom + 8 : after ? after.rect.top - 8 : viewRect.top + 24
  const { posts, categories } = useDoc.getState().doc
  const filter = view.dataset.category
  const neighbour = posts[index - 1] ?? posts[index]
  const category = filter && filter !== 'all' ? filter : neighbour ? neighbour.category : categories[0].id
  return { kind: 'gap', index, category, line: { left: viewRect.left + 12, top: y - LINE_PX / 2, width: viewRect.width - 24, height: LINE_PX } }
}

const isNoopInsert = (items: string[], ids: string[], index: number) => {
  const positions = ids.map((id) => items.indexOf(id))
  if (positions.some((position) => position === -1)) return false
  const first = Math.min(...positions)
  const last = Math.max(...positions)
  return last - first + 1 === ids.length && index >= first && index <= last + 1
}

const postTarget = (card: Card, ids: string[], point: Point): DropTarget | null => {
  const post = useDoc.getState().doc.posts[card.index]
  const ring = boxOf(card.rect)
  if (!canPlaceInPost(post, ids)) return { kind: 'post', postId: post.id, index: post.items.length, valid: false, ring, line: null }
  const tiles = [...card.element.querySelectorAll<HTMLElement>('[data-tile-index]')].map((element) => ({
    index: Number(element.dataset.tileIndex),
    rect: element.getBoundingClientRect(),
  }))
  if (tiles.length === 0) return { kind: 'post', postId: post.id, index: 0, valid: true, ring, line: null }

  const rows: (typeof tiles)[] = []
  for (const tile of tiles) {
    const row = rows.at(-1)
    if (row && Math.abs(row[0].rect.top - tile.rect.top) < 2) row.push(tile)
    else rows.push([tile])
  }
  const lastRow = rows.at(-1)!
  const lastTile = lastRow.at(-1)!
  const row = rows.find((candidate) => point.y <= candidate[0].rect.bottom + 2) ?? lastRow
  const beyondEnd = point.y > lastTile.rect.bottom + 2
  const next = beyondEnd ? undefined : row.find((tile) => point.x < tile.rect.left + tile.rect.width / 2)
  const previous = next ? row.find((tile) => tile.index === next.index - 1) : beyondEnd ? lastTile : row.at(-1)
  const index = next ? next.index : previous!.index + 1
  if (isNoopInsert(post.items, ids, index)) return null
  const x = next && previous ? (previous.rect.right + next.rect.left) / 2 : next ? next.rect.left - 3 : previous!.rect.right + 3
  const rowRect = (next ?? previous!).rect
  return { kind: 'post', postId: post.id, index, valid: true, ring, line: { left: x - LINE_PX / 2, top: rowRect.top, width: LINE_PX, height: rowRect.height } }
}

const resolveTarget = (payload: DragPayload, point: Point): DropTarget | null => {
  const element = document.elementFromPoint(point.x, point.y)
  if (!element) return null

  const library = element.closest<HTMLElement>('[data-drop="library"]')
  if (library) return payload.kind === 'photos' && payload.fromPostId !== null ? { kind: 'library', ring: boxOf(library.getBoundingClientRect()) } : null

  const view = element.closest<HTMLElement>('[data-drop="posts"]')
  if (!view) return null
  const cards: Card[] = [...view.querySelectorAll<HTMLElement>('[data-post-index]')].map((card) => ({
    element: card,
    index: Number(card.dataset.postIndex),
    rect: card.getBoundingClientRect(),
  }))
  const card = cards.find((candidate) => point.y >= candidate.rect.top && point.y <= candidate.rect.bottom)
  const postFrom = payload.kind === 'post' ? useDoc.getState().doc.posts.findIndex((post) => post.id === payload.postId) : -1
  const gap = (index: number) => (payload.kind === 'post' && (index === postFrom || index === postFrom + 1) ? null : gapTarget(view, cards, index))

  if (!card) {
    const above = cards.filter((candidate) => candidate.rect.bottom < point.y).at(-1)
    return gap(above ? above.index + 1 : (cards[0]?.index ?? useDoc.getState().doc.posts.length))
  }
  if (payload.kind === 'post') return gap(point.y < card.rect.top + card.rect.height / 2 ? card.index : card.index + 1)
  if (point.y < card.rect.top + EDGE_BAND_PX) return gap(card.index)
  if (point.y > card.rect.bottom - EDGE_BAND_PX) return gap(card.index + 1)
  return postTarget(card, payload.ids, point)
}

const autoscroll = (point: Point, elapsed: number) => {
  const container = document.elementFromPoint(point.x, point.y)?.closest<HTMLElement>('[data-scroll]')
  if (!container) return
  const rect = container.getBoundingClientRect()
  const overTop = rect.top + AUTOSCROLL_ZONE_PX - point.y
  const overBottom = point.y - (rect.bottom - AUTOSCROLL_ZONE_PX)
  const push = overTop > 0 ? -overTop : overBottom > 0 ? overBottom : 0
  container.scrollTop += push * elapsed * AUTOSCROLL_SPEED
}

const drop = (payload: DragPayload, target: DropTarget) => {
  if (payload.kind === 'post') {
    if (target.kind === 'gap') commit((doc) => movePost(doc, payload.postId, target.index))
    return
  }
  const source = payload.fromPostId
  if (target.kind === 'library') {
    if (source !== null) commit((doc) => unplacePhotos(doc, payload.ids, source))
  } else if (target.kind === 'gap') {
    commit((doc) => placePhotos(doc, payload.ids, { kind: 'gap', index: target.index, category: target.category }, source))
  } else if (target.valid) {
    commit((doc) => placePhotos(doc, payload.ids, { kind: 'post', postId: target.postId, index: target.index }, source))
  }
  clearSelection()
}

export const pressToDrag = (event: ReactPointerEvent, getPayload: () => DragPayload, onClick: (modifiers: ClickModifiers) => void) => {
  if (event.button !== 0) return
  const modifiers = { range: event.shiftKey, toggle: event.ctrlKey || event.metaKey }
  const start = { x: event.clientX, y: event.clientY }
  let pointer = start
  let payload: DragPayload | null = null
  let frame = 0
  let previousTime = performance.now()

  const tick = (time: number) => {
    autoscroll(pointer, time - previousTime)
    previousTime = time
    useDrag.setState({ target: resolveTarget(payload!, pointer) })
    frame = requestAnimationFrame(tick)
  }

  const move = (moveEvent: PointerEvent) => {
    pointer = { x: moveEvent.clientX, y: moveEvent.clientY }
    if (!payload) {
      if (Math.hypot(pointer.x - start.x, pointer.y - start.y) < DRAG_THRESHOLD_PX) return
      payload = getPayload()
      document.documentElement.dataset.dragging = ''
      previousTime = performance.now()
      frame = requestAnimationFrame(tick)
    }
    useDrag.setState({ payload, pointer })
  }

  const finish = (commitDrop: boolean) => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', release)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', escape, true)
    cancelAnimationFrame(frame)
    delete document.documentElement.dataset.dragging
    useDrag.setState({ payload: null, target: null })
    if (!payload) {
      if (commitDrop) onClick(modifiers)
      return
    }
    const target = commitDrop ? resolveTarget(payload, pointer) : null
    if (target) drop(payload, target)
  }

  const release = () => finish(true)
  const cancel = () => finish(false)
  const escape = (keyEvent: KeyboardEvent) => {
    if (keyEvent.key !== 'Escape') return
    keyEvent.stopPropagation()
    finish(false)
  }

  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', release)
  window.addEventListener('pointercancel', cancel)
  window.addEventListener('keydown', escape, true)
}
