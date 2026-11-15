import { produce } from 'immer'
import { create } from 'zustand'
import type { PostsViewKey } from './settings'
import { EMPTY_DOC, type Adjustments, type Doc, type PhotoFile } from './types'

const HISTORY_LIMIT = 300
const COALESCE_MS = 1200

type DocState = {
  doc: Doc
  past: Doc[]
  future: Doc[]
  lastKey: string | null
  lastAt: number
}

export const useDoc = create<DocState>(() => ({ doc: EMPTY_DOC, past: [], future: [], lastKey: null, lastAt: 0 }))

export const commit = (recipe: (draft: Doc) => void, coalesceKey: string | null = null) => {
  const state = useDoc.getState()
  const next = produce(state.doc, recipe)
  if (next === state.doc) return
  const now = performance.now()
  const coalesce = coalesceKey !== null && coalesceKey === state.lastKey && now - state.lastAt < COALESCE_MS
  useDoc.setState({
    doc: next,
    past: coalesce ? state.past : [...state.past.slice(-HISTORY_LIMIT), state.doc],
    future: [],
    lastKey: coalesceKey,
    lastAt: now,
  })
}

export const resetDoc = (doc: Doc) => useDoc.setState({ doc, past: [], future: [], lastKey: null, lastAt: 0 })

export const undo = () => {
  const { doc, past, future } = useDoc.getState()
  const previous = past.at(-1)
  if (!previous) return
  useDoc.setState({ doc: previous, past: past.slice(0, -1), future: [doc, ...future], lastKey: null })
}

export const redo = () => {
  const { doc, past, future } = useDoc.getState()
  const [next, ...rest] = future
  if (!next) return
  useDoc.setState({ doc: next, past: [...past, doc], future: rest, lastKey: null })
}

type LibraryState = {
  root: FileSystemDirectoryHandle | null
  files: PhotoFile[]
  byId: Map<string, PhotoFile>
}

export const useLibrary = create<LibraryState>(() => ({ root: null, files: [], byId: new Map() }))

export type ViewKey = 'library' | PostsViewKey

export type Mode = 'compose' | 'send'

type Viewer = { ids: string[]; id: string; origin: ViewKey }

type UiState = {
  mode: Mode
  selection: Set<string>
  selectionAnchor: string | null
  viewer: Viewer | null
  highlight: { id: string; view: ViewKey; nonce: number } | null
  lastPostsView: PostsViewKey
  copiedAdjustments: Adjustments | null
}

export const useUi = create<UiState>(() => ({
  mode: 'compose',
  selection: new Set(),
  selectionAnchor: null,
  viewer: null,
  highlight: null,
  lastPostsView: 'posts-a',
  copiedAdjustments: null,
}))

export const openViewer = (ids: string[], id: string, origin: ViewKey) =>
  useUi.setState({ viewer: { ids, id, origin }, highlight: null })

export const closeViewer = () => {
  const { viewer } = useUi.getState()
  if (!viewer) return
  useUi.setState({ viewer: null, highlight: { id: viewer.id, view: viewer.origin, nonce: performance.now() } })
}

export const revealPhoto = (id: string, view: ViewKey) =>
  useUi.setState({ highlight: { id, view, nonce: performance.now() } })

export const clearSelection = () => useUi.setState({ selection: new Set(), selectionAnchor: null })
