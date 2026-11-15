import { useVirtualizer } from '@tanstack/react-virtual'
import { Archive, ArrowDownAZ, CalendarClock, Check, Grid2x2, List, Rows3, SquarePlus, X } from 'lucide-react'
import { memo, useEffect, useState, type PointerEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { pressToDrag, type ClickModifiers } from '../dnd/drag'
import { t } from '../i18n'
import { useDisplayThumb } from '../imaging/thumbs'
import { placementsByPhoto, placePhotos, setArchived, updateMeta } from '../model/ops'
import { ALL_CATEGORIES, useSettings } from '../model/settings'
import { clearSelection, commit, openViewer, revealPhoto, useDoc, useLibrary, useUi } from '../model/store'
import { metaOf, photoCategory, type PhotoFile } from '../model/types'
import { Tile } from './Tile'
import { Chip, IconButton, PaneHeader, RangeInput, Segmented } from './ui'

const GAP_PX = 6
const LIST_ROW_REM = 3.5

const useLibraryCategory = () => {
  const filter = useSettings((state) => state.libraryCategory)
  const known = useDoc((state) => state.doc.categories.some((category) => category.id === filter))
  return known ? filter : ALL_CATEGORIES
}

const useVisibleFiles = () => {
  const files = useLibrary((state) => state.files)
  const { hideChecked, hidePlaced, showArchived, librarySort } = useSettings()
  const category = useLibraryCategory()
  return useDoc(
    useShallow(({ doc }) => {
      const placements = placementsByPhoto(doc.posts)
      const visible = files.filter((file) => {
        const meta = metaOf(doc, file.id)
        if (meta.archived !== showArchived) return false
        if (hideChecked && meta.checked) return false
        if (category !== ALL_CATEGORIES && photoCategory(doc, file.id) !== category) return false
        return !(hidePlaced && placements.has(file.id))
      })
      return librarySort === 'date' ? visible.toSorted((a, b) => a.lastModified - b.lastModified) : visible
    }),
  )
}

const useWidth = (element: HTMLElement | null) => {
  const [width, setWidth] = useState(0)
  useEffect(() => {
    if (!element) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(element)
    return () => observer.disconnect()
  }, [element])
  return width
}

const selectFrom = (visible: PhotoFile[], id: string, modifiers: ClickModifiers) => {
  const { selection, selectionAnchor } = useUi.getState()
  const ids = visible.map((file) => file.id)
  if (modifiers.range && selectionAnchor && ids.includes(selectionAnchor)) {
    const [from, to] = [ids.indexOf(selectionAnchor), ids.indexOf(id)].sort((a, b) => a - b)
    return useUi.setState({ selection: new Set([...selection, ...ids.slice(from, to + 1)]) })
  }
  const next = new Set(selection)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  useUi.setState({ selection: next, selectionAnchor: id })
}

const CategoryMark = ({ id }: { id: string }) => {
  const category = useDoc((state) => {
    const { categories } = state.doc
    const current = photoCategory(state.doc, id)
    return current === categories[0].id ? null : categories.find((candidate) => candidate.id === current)
  })
  if (!category) return null
  return <span title={category.name} style={{ backgroundColor: category.color }} className="size-3 shrink-0 rounded-full ring-2 ring-white/90" />
}

const PlacementBadges = ({ id }: { id: string }) => {
  const placements = useDoc(useShallow((state) => placementsByPhoto(state.doc.posts).get(id) ?? []))
  const postCategories = useDoc(useShallow((state) => (placementsByPhoto(state.doc.posts).get(id) ?? []).map((index) => state.doc.posts[index].category)))
  const categories = useDoc((state) => state.doc.categories)
  if (placements.length === 0) return null
  return (
    <div className="flex gap-1">
      {placements.map((index, position) => (
        <button
          key={index}
          type="button"
          title={t('showInPosts')}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => {
            event.stopPropagation()
            revealPhoto(id, useUi.getState().lastPostsView)
          }}
          style={{ backgroundColor: categories.find((category) => category.id === postCategories[position])?.color }}
          className="rounded-md px-1.5 py-0.5 text-[0.6875rem] leading-none font-semibold text-white shadow hover:brightness-110"
        >
          #{index + 1}
        </button>
      ))}
    </div>
  )
}

const LibraryTile = memo(({ file, visible, layout }: { file: PhotoFile; visible: PhotoFile[]; layout: 'grid' | 'list' }) => {
  const thumb = useDisplayThumb(file.id)
  const onPointerDown = (event: PointerEvent) =>
    pressToDrag(
      event,
      () => {
        const { selection } = useUi.getState()
        const ids = selection.has(file.id) ? visible.filter((candidate) => selection.has(candidate.id)).map((candidate) => candidate.id) : [file.id]
        return { kind: 'photos', ids, fromPostId: null }
      },
      (modifiers) =>
        modifiers.range || modifiers.toggle
          ? selectFrom(visible, file.id, modifiers)
          : openViewer(
              visible.map((candidate) => candidate.id),
              file.id,
              'library',
            ),
    )

  if (layout === 'grid')
    return (
      <Tile id={file.id} view="library" thumb={thumb} fit="contain" actions onPointerDown={onPointerDown} className="size-full">
        <div className="absolute bottom-1.5 left-1.5 flex items-center gap-1">
          <CategoryMark id={file.id} />
          <PlacementBadges id={file.id} />
        </div>
      </Tile>
    )

  return (
    <div className="flex h-full items-center gap-3 rounded-lg px-1 hover:bg-raised/60">
      <Tile id={file.id} view="library" thumb={thumb} fit="cover" actions={false} onPointerDown={onPointerDown} className="aspect-square h-[calc(100%-0.5rem)]" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm text-fg">{file.id}</div>
        <div className="text-xs text-faint">
          {(file.size / 1048576).toFixed(1)} MB · {new Date(file.lastModified).toLocaleDateString()}
        </div>
      </div>
      <CategoryMark id={file.id} />
      <PlacementBadges id={file.id} />
    </div>
  )
})

export const Library = () => {
  const visible = useVisibleFiles()
  const total = useLibrary((state) => state.files.length)
  const settings = useSettings()
  const categories = useDoc((state) => state.doc.categories)
  const category = useLibraryCategory()
  const selectionSize = useUi((state) => state.selection.size)
  const highlight = useUi((state) => (state.highlight?.view === 'library' ? state.highlight : null))
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null)
  const width = useWidth(scroller)
  const remPx = parseFloat(getComputedStyle(document.documentElement).fontSize)

  const grid = settings.libraryLayout === 'grid'
  const columns = grid ? Math.max(1, Math.floor((width + GAP_PX) / (settings.libraryZoom * remPx + GAP_PX))) : 1
  const cellSize = grid ? (width - GAP_PX * (columns - 1)) / columns : LIST_ROW_REM * remPx
  const rows = Math.ceil(visible.length / columns)

  const virtualizer = useVirtualizer({
    count: rows,
    getScrollElement: () => scroller,
    estimateSize: () => cellSize + (grid ? GAP_PX : 0),
    overscan: 3,
    paddingStart: GAP_PX,
    paddingEnd: GAP_PX * 4,
  })
  useEffect(() => virtualizer.measure(), [virtualizer, cellSize, columns])

  useEffect(() => {
    if (!highlight) return
    const index = visible.findIndex((file) => file.id === highlight.id)
    if (index !== -1) virtualizer.scrollToIndex(Math.floor(index / columns), { align: 'center' })
  }, [virtualizer, highlight, visible, columns])

  const selectedIds = () => {
    const { selection } = useUi.getState()
    return visible.filter((file) => selection.has(file.id)).map((file) => file.id)
  }

  return (
    <section className="flex h-full flex-col bg-panel">
      <PaneHeader>
        <h2 className="mr-1 text-sm font-semibold text-fg">{settings.showArchived ? t('archive') : t('library')}</h2>
        <span className="text-xs text-faint tabular-nums">
          {visible.length}
          {visible.length !== total && ` / ${total}`}
        </span>
        <div className="flex-1" />
        <Chip label={t('hideCheckedHint')} active={settings.hideChecked} onClick={() => useSettings.setState({ hideChecked: !settings.hideChecked })}>
          <Check className="size-3.5" strokeWidth={3} /> {t('hideChecked')}
        </Chip>
        <Chip label={t('hidePlacedHint')} active={settings.hidePlaced} onClick={() => useSettings.setState({ hidePlaced: !settings.hidePlaced })}>
          <Rows3 className="size-3.5" /> {t('hidePlaced')}
        </Chip>
        <Chip label={t('showArchivedHint')} active={settings.showArchived} onClick={() => useSettings.setState({ showArchived: !settings.showArchived })}>
          <Archive className="size-3.5" />
        </Chip>
      </PaneHeader>
      <PaneHeader>
        <Segmented
          value={settings.libraryLayout}
          onChange={(libraryLayout) => useSettings.setState({ libraryLayout })}
          options={[
            { value: 'grid', label: <Grid2x2 className="size-3.5" />, title: t('grid') },
            { value: 'list', label: <List className="size-3.5" />, title: t('list') },
          ]}
        />
        {grid && (
          <RangeInput
            label={t('zoom')}
            min={4}
            max={24}
            step={0.5}
            value={settings.libraryZoom}
            onChange={(libraryZoom) => useSettings.setState({ libraryZoom })}
            className="w-16"
          />
        )}
        <div className="flex-1" />
        <Chip label={t('allCategories')} active={category === ALL_CATEGORIES} onClick={() => useSettings.setState({ libraryCategory: ALL_CATEGORIES })}>
          {t('allCategories')}
        </Chip>
        {categories.map((option) => (
          <Chip
            key={option.id}
            label={option.name}
            active={category === option.id}
            color={option.color}
            onClick={() => useSettings.setState({ libraryCategory: option.id })}
          >
            {option.name}
          </Chip>
        ))}
        <Segmented
          value={settings.librarySort}
          onChange={(librarySort) => useSettings.setState({ librarySort })}
          options={[
            { value: 'name', label: <ArrowDownAZ className="size-3.5" />, title: t('sortNameHint') },
            { value: 'date', label: <CalendarClock className="size-3.5" />, title: t('sortDateHint') },
          ]}
        />
      </PaneHeader>
      {selectionSize > 0 && (
        <div className="flex h-11 shrink-0 items-center gap-1 border-b border-line bg-accent/10 px-3">
          <span className="mr-auto text-xs font-semibold text-accent">{t('selected', { count: selectionSize })}</span>
          <IconButton
            icon={SquarePlus}
            label={t('newPostFromSelection')}
            onClick={() => {
              const ids = selectedIds()
              const filter = useSettings.getState().postsFilter[useUi.getState().lastPostsView]
              commit((doc) =>
                placePhotos(doc, ids, { kind: 'gap', index: doc.posts.length, category: filter === ALL_CATEGORIES ? doc.categories[0].id : filter }, null),
              )
              clearSelection()
            }}
          />
          <IconButton icon={Check} label={t('check')} onClick={() => commit((doc) => updateMeta(doc, selectedIds(), { checked: true }))} />
          <IconButton
            icon={Archive}
            label={settings.showArchived ? t('restore') : t('archive')}
            onClick={() => {
              const ids = selectedIds()
              commit((doc) => setArchived(doc, ids, !settings.showArchived))
              clearSelection()
            }}
          />
          <IconButton icon={X} label={t('clearSelection')} onClick={clearSelection} />
        </div>
      )}
      <div ref={setScroller} data-scroll data-drop="library" className="min-h-0 flex-1 overflow-y-auto px-2">
        {visible.length === 0 ? (
          <p className="px-6 py-16 text-center text-sm text-faint">{total === 0 ? t('noPhotos') : t('nothingMatches')}</p>
        ) : (
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((row) => (
              <div key={row.key} className="absolute inset-x-0 flex" style={{ top: row.start, height: cellSize, gap: GAP_PX }}>
                {visible.slice(row.index * columns, row.index * columns + columns).map((file) => (
                  <div key={file.id} style={{ width: grid ? cellSize : '100%', height: cellSize }}>
                    <LibraryTile file={file} visible={visible} layout={settings.libraryLayout} />
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  )
}
