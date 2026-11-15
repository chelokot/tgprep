import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDownWideNarrow, Copy, Download, EyeOff, GripVertical, Plus, Send, Trash2, X } from 'lucide-react'
import { memo, useEffect, useMemo, useState, type PointerEvent } from 'react'
import { pressToDrag } from '../dnd/drag'
import { t } from '../i18n'
import { copyPhoto, copyText, exportPost, preparePng } from '../imaging/export'
import { useDisplayThumb } from '../imaging/thumbs'
import {
  addCategory,
  createPost,
  deleteCategory,
  deletePost,
  insertPost,
  renameCategory,
  setPostCategory,
  sortPostsByTime,
  unplacePhotos,
  updateMeta,
  updatePost,
} from '../model/ops'
import { ALL_CATEGORIES, useSettings, type PostsViewKey } from '../model/settings'
import { commit, openViewer, useDoc, useLibrary, useUi } from '../model/store'
import { MAX_CAPTION_LENGTH, MAX_POST_PHOTOS, metaOf, type Category, type Post } from '../model/types'
import { CornerButton, Tile } from './Tile'
import { toast } from './toast'
import { Chip, IconButton, PaneHeader, RangeInput } from './ui'

const CARD_GAP_PX = 12

const activeFilterOf = (filter: string, categories: Category[]) =>
  filter === ALL_CATEGORIES || categories.some((category) => category.id === filter) ? filter : ALL_CATEGORIES

const visiblePosts = (posts: Post[], filter: string, hideSent: boolean) =>
  posts.map((post, index) => ({ post, index })).filter(({ post }) => (filter === ALL_CATEGORIES || post.category === filter) && !(hideSent && post.sent))

const viewPhotoIds = (view: PostsViewKey) => {
  const { doc } = useDoc.getState()
  const settings = useSettings.getState()
  const hideSent = useUi.getState().mode === 'send' && settings.hideSent
  return visiblePosts(doc.posts, activeFilterOf(settings.postsFilter[view], doc.categories), hideSent).flatMap(({ post }) => post.items)
}

const copyPostPhoto = async (id: string, number: number) => {
  const file = useLibrary.getState().byId.get(id)
  if (!file) return
  try {
    await copyPhoto(file, metaOf(useDoc.getState().doc, id).edits, useSettings.getState().exportLongSide)
    commit((doc) => updateMeta(doc, [id], { checked: true }))
    toast(t('copiedPhoto', { number }))
  } catch (error) {
    toast(t('copyFailed', { reason: error instanceof Error ? error.message : String(error) }))
  }
}

const PostTile = ({ id, index, post, view }: { id: string; index: number; post: Post; view: PostsViewKey }) => {
  const thumb = useDisplayThumb(id)
  const mode = useUi((state) => state.mode)
  const height = useSettings((state) => state.postTileHeight)
  const onPointerDown = (event: PointerEvent) => {
    if (mode === 'send') return void (event.button === 0 && copyPostPhoto(id, index + 1))
    pressToDrag(
      event,
      () => ({ kind: 'photos', ids: [id], fromPostId: post.id }),
      () => openViewer(viewPhotoIds(view), id, view),
    )
  }
  return (
    <Tile
      id={id}
      view={view}
      thumb={thumb}
      fit="cover"
      actions={mode === 'compose'}
      onPointerDown={onPointerDown}
      style={{ height: `${height}rem`, width: `${height * (thumb?.aspect ?? 1)}rem` }}
      className="shrink-0"
    >
      <span data-tile-index={index} className="absolute inset-0" />
      <span className="absolute bottom-1.5 left-1.5 grid size-5 place-items-center rounded-full bg-black/55 text-[0.6875rem] font-semibold text-white backdrop-blur group-hover/tile:opacity-0">
        {index + 1}
      </span>
      {mode === 'compose' ? (
        <CornerButton
          label={t('removeFromPost')}
          onClick={() => commit((doc) => unplacePhotos(doc, [id], post.id))}
          className="bottom-1.5 left-1.5 bg-black/55 text-white opacity-0 backdrop-blur group-hover/tile:opacity-100 hover:bg-danger"
        >
          <X className="size-3.5" />
        </CornerButton>
      ) : (
        <span className="absolute inset-0 grid place-items-center bg-black/0 opacity-0 transition group-hover/tile:bg-black/35 group-hover/tile:opacity-100">
          <Copy className="size-6 text-white drop-shadow" />
        </span>
      )}
    </Tile>
  )
}

const PostCard = memo(({ post, index, view }: { post: Post; index: number; view: PostsViewKey }) => {
  const mode = useUi((state) => state.mode)
  const categories = useDoc((state) => state.doc.categories)
  const category = categories.find((candidate) => candidate.id === post.category) ?? categories[0]
  const overLimit = post.caption.length > MAX_CAPTION_LENGTH

  const prepare = () => {
    if (mode !== 'send') return
    const { byId } = useLibrary.getState()
    const { doc } = useDoc.getState()
    const longSide = useSettings.getState().exportLongSide
    for (const id of post.items) {
      const file = byId.get(id)
      if (file) void preparePng(file, metaOf(doc, id).edits, longSide)
    }
  }

  const save = async () => {
    const { root, byId } = useLibrary.getState()
    if (!root) return
    const { doc } = useDoc.getState()
    const photos = post.items.flatMap((id) => {
      const file = byId.get(id)
      return file ? [{ file, edits: metaOf(doc, id).edits }] : []
    })
    toast(t('exported', { path: await exportPost(root, post, index + 1, photos, useSettings.getState().exportLongSide) }))
  }

  return (
    <article
      data-post-index={index}
      onPointerEnter={prepare}
      className={`rounded-2xl border border-line bg-panel p-3 shadow-sm transition-opacity ${post.sent ? 'opacity-55' : ''}`}
      style={{ borderLeftColor: category.color, borderLeftWidth: '0.25rem' }}
    >
      <header className="mb-2 flex items-center gap-1.5">
        {mode === 'compose' && (
          <button
            type="button"
            title={t('movePost')}
            aria-label={t('movePost')}
            onPointerDown={(event) => pressToDrag(event, () => ({ kind: 'post', postId: post.id }), () => {})}
            className="-ml-1 grid h-7 w-5 cursor-grab place-items-center rounded text-faint hover:bg-raised hover:text-fg"
          >
            <GripVertical className="size-4" />
          </button>
        )}
        <span className="text-sm font-semibold text-fg tabular-nums">#{index + 1}</span>
        <select
          aria-label={t('postCategory')}
          title={t('postCategory')}
          value={post.category}
          disabled={mode === 'send'}
          onChange={(event) => commit((doc) => setPostCategory(doc, post.id, event.target.value))}
          style={{ color: category.color }}
          className="max-w-32 truncate rounded-md bg-transparent px-1 py-0.5 text-xs font-semibold hover:bg-raised disabled:appearance-none"
        >
          {categories.map((option) => (
            <option key={option.id} value={option.id}>
              {option.name}
            </option>
          ))}
        </select>
        <span className={`text-xs tabular-nums ${post.items.length === MAX_POST_PHOTOS ? 'text-accent' : 'text-faint'}`}>
          {post.items.length}/{MAX_POST_PHOTOS}
        </span>
        <div className="flex-1" />
        <IconButton
          icon={Send}
          label={post.sent ? t('unmarkSent') : t('markSent')}
          active={post.sent}
          onClick={() => commit((doc) => updatePost(doc, post.id, { sent: !post.sent }))}
        />
        <IconButton icon={Download} label={t('exportPost')} disabled={post.items.length === 0} onClick={() => void save()} />
        {mode === 'compose' && <IconButton icon={Trash2} label={t('deletePost')} onClick={() => commit((doc) => deletePost(doc, post.id))} />}
      </header>
      <div data-post-media className="flex min-h-16 flex-wrap gap-1">
        {post.items.length === 0 ? (
          <div className="grid h-16 w-full place-items-center rounded-lg border border-dashed border-line text-xs text-faint">{t('dropHere')}</div>
        ) : (
          post.items.map((id, itemIndex) => <PostTile key={id} id={id} index={itemIndex} post={post} view={view} />)
        )}
      </div>
      {mode === 'compose' ? (
        <div className="mt-2">
          <textarea
            value={post.caption}
            placeholder={t('caption')}
            rows={1}
            onChange={(event) => commit((doc) => updatePost(doc, post.id, { caption: event.target.value }), `caption:${post.id}`)}
            className="block max-h-60 w-full resize-none rounded-lg bg-raised px-3 py-2 text-sm text-fg outline-none [field-sizing:content] placeholder:text-faint focus:ring-2 focus:ring-accent/50"
          />
          {post.caption.length > 0 && (
            <div title={t('captionLimit', { limit: MAX_CAPTION_LENGTH })} className={`mt-1 text-right text-[0.6875rem] tabular-nums ${overLimit ? 'text-danger' : 'text-faint'}`}>
              {post.caption.length}/{MAX_CAPTION_LENGTH}
            </div>
          )}
        </div>
      ) : (
        <button
          type="button"
          title={t('copyCaption')}
          disabled={post.caption.trim() === ''}
          onClick={() =>
            void copyText(post.caption).then(
              () => toast(t('copiedCaption')),
              (error: unknown) => toast(t('copyFailed', { reason: String(error) })),
            )
          }
          className="group/caption mt-2 flex w-full items-start gap-2 rounded-lg bg-raised px-3 py-2 text-left text-sm whitespace-pre-wrap text-fg transition hover:bg-accent/15 disabled:text-faint disabled:hover:bg-raised"
        >
          <span className="flex-1">{post.caption.trim() === '' ? t('noCaption') : post.caption}</span>
          <Copy className="mt-0.5 size-4 shrink-0 text-faint group-hover/caption:text-accent" />
        </button>
      )}
    </article>
  )
})

const CategoryChip = ({ category, active, onSelect }: { category: Category; active: boolean; onSelect: () => void }) => {
  const [editing, setEditing] = useState(false)
  const deletable = useDoc((state) => state.doc.categories.length > 1)
  if (editing)
    return (
      <span className="flex shrink-0 items-center gap-1 rounded-full bg-raised py-0.5 pr-1 pl-2.5">
        <input
          autoFocus
          aria-label={t('categoryName')}
          defaultValue={category.name}
          onBlur={() => setEditing(false)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' || event.key === 'Escape') event.currentTarget.blur()
          }}
          onChange={(event) => commit((doc) => renameCategory(doc, category.id, event.target.value), `category:${category.id}`)}
          className="w-24 bg-transparent text-xs text-fg outline-none"
        />
        {deletable && (
          <button
            type="button"
            title={t('deleteCategory')}
            aria-label={t('deleteCategory')}
            onPointerDown={(event) => event.preventDefault()}
            onClick={() => commit((doc) => deleteCategory(doc, category.id))}
            className="grid size-5 place-items-center rounded-full text-faint hover:bg-danger hover:text-white"
          >
            <Trash2 className="size-3" />
          </button>
        )}
      </span>
    )
  return (
    <span onDoubleClick={() => setEditing(true)}>
      <Chip label={category.name} active={active} color={category.color} onClick={onSelect}>
        {category.name}
      </Chip>
    </span>
  )
}

export const PostsView = ({ view }: { view: PostsViewKey }) => {
  const posts = useDoc((state) => state.doc.posts)
  const categories = useDoc((state) => state.doc.categories)
  const mode = useUi((state) => state.mode)
  const filter = useSettings((state) => state.postsFilter[view])
  const hideSent = useSettings((state) => state.hideSent)
  const tileHeight = useSettings((state) => state.postTileHeight)
  const highlight = useUi((state) => (state.highlight?.view === view ? state.highlight : null))
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null)

  const setFilter = (value: string) => useSettings.setState((state) => ({ postsFilter: { ...state.postsFilter, [view]: value } }))
  const activeFilter = activeFilterOf(filter, categories)

  const visible = useMemo(() => visiblePosts(posts, activeFilter, mode === 'send' && hideSent), [posts, activeFilter, mode, hideSent])

  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scroller,
    estimateSize: () => 220,
    overscan: 4,
    getItemKey: (index) => visible[index].post.id,
    paddingStart: CARD_GAP_PX,
  })
  useEffect(() => {
    if (!highlight) return
    const position = visible.findIndex(({ post }) => post.items.includes(highlight.id))
    if (position !== -1) return virtualizer.scrollToIndex(position, { align: 'center' })
    if (posts.some((post) => post.items.includes(highlight.id)))
      useSettings.setState((state) => ({ postsFilter: { ...state.postsFilter, [view]: ALL_CATEGORIES } }))
  }, [virtualizer, highlight, visible, posts, view])

  const newPostCategory = activeFilter === ALL_CATEGORIES ? (posts.at(-1)?.category ?? categories[0].id) : activeFilter

  return (
    <section className="flex h-full flex-col bg-canvas" onPointerDownCapture={() => useUi.setState({ lastPostsView: view })}>
      <PaneHeader>
        <h2 className="mr-1 text-sm font-semibold text-fg">{t('posts')}</h2>
        <span className="text-xs text-faint tabular-nums">{visible.length}</span>
        <div className="flex-1" />
        <Chip label={t('allCategories')} active={activeFilter === ALL_CATEGORIES} onClick={() => setFilter(ALL_CATEGORIES)}>
          {t('allCategories')}
        </Chip>
        {categories.map((category) => (
          <CategoryChip key={category.id} category={category} active={activeFilter === category.id} onSelect={() => setFilter(category.id)} />
        ))}
        <IconButton icon={Plus} label={t('newCategory')} onClick={() => commit((doc) => addCategory(doc, `${t('newCategory')} ${doc.categories.length + 1}`))} />
      </PaneHeader>
      <PaneHeader>
        <RangeInput label={t('tileHeight')} min={4} max={20} step={0.5} value={tileHeight} onChange={(postTileHeight) => useSettings.setState({ postTileHeight })} className="w-28" />
        <div className="flex-1" />
        {mode === 'compose' && (
          <IconButton
            icon={ArrowDownWideNarrow}
            label={t('sortPostsByTime')}
            onClick={() => {
              const times = new Map(useLibrary.getState().files.map((file) => [file.id, file.lastModified]))
              commit((doc) => sortPostsByTime(doc, times))
            }}
          />
        )}
        {mode === 'send' && (
          <Chip label={t('hideSentHint')} active={hideSent} onClick={() => useSettings.setState({ hideSent: !hideSent })}>
            <EyeOff className="size-3.5" /> {t('hideSent')}
          </Chip>
        )}
      </PaneHeader>
      <div ref={setScroller} data-scroll data-drop="posts" data-category={activeFilter} className="min-h-0 flex-1 overflow-y-auto px-3">
        {visible.length === 0 && <p className="px-6 pt-16 text-center text-sm text-faint">{t('emptyPosts')}</p>}
        <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((item) => {
            const { post, index } = visible[item.index]
            return (
              <div key={item.key} data-index={item.index} ref={virtualizer.measureElement} className="absolute inset-x-0" style={{ top: item.start, paddingBottom: CARD_GAP_PX }}>
                <PostCard post={post} index={index} view={view} />
              </div>
            )
          })}
        </div>
        {mode === 'compose' && (
          <button
            type="button"
            onClick={() => commit((doc) => insertPost(doc, createPost([], newPostCategory), doc.posts.length))}
            className="mb-40 flex h-12 w-full items-center justify-center gap-2 rounded-2xl border border-dashed border-line text-sm text-muted transition hover:border-accent hover:text-accent"
          >
            <Plus className="size-4" /> {t('newPost')}
          </button>
        )}
      </div>
    </section>
  )
}
