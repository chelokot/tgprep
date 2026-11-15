import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type LibraryLayout = 'grid' | 'list'
export type LibrarySort = 'name' | 'date'

type Settings = {
  exportLongSide: number | null
  libraryLayout: LibraryLayout
  librarySort: LibrarySort
  libraryZoom: number
  hideChecked: boolean
  hidePlaced: boolean
  libraryCategory: string
  showArchived: boolean
  postTileHeight: number
  hideSent: boolean
  postsFilter: Record<PostsViewKey, string>
  paneWidths: [number, number]
  editPanelOpen: boolean
}

export type PostsViewKey = 'posts-a' | 'posts-b'

export const ALL_CATEGORIES = 'all'

export const TELEGRAM_LONG_SIDE = 2560

export const useSettings = create<Settings>()(
  persist(
    (): Settings => ({
      exportLongSide: TELEGRAM_LONG_SIDE,
      libraryLayout: 'grid',
      librarySort: 'name',
      libraryZoom: 9,
      hideChecked: false,
      hidePlaced: false,
      libraryCategory: ALL_CATEGORIES,
      showArchived: false,
      postTileHeight: 8,
      hideSent: false,
      postsFilter: { 'posts-a': ALL_CATEGORIES, 'posts-b': ALL_CATEGORIES },
      paneWidths: [0.34, 0.34],
      editPanelOpen: false,
    }),
    { name: 'tgprep-settings' },
  ),
)
