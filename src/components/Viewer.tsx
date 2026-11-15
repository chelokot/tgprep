import { Archive, ArchiveRestore, Check, ChevronLeft, ChevronRight, ClipboardCopy, ClipboardPaste, Copy, Crop as CropIcon, Eye, FlipHorizontal2, RotateCcw, RotateCw, SlidersHorizontal, Undo2, X } from 'lucide-react'
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { createPortal } from 'react-dom'
import { t } from '../i18n'
import { copyPhoto } from '../imaging/export'
import { aspectRatio, fitCrop, frameSize, moveCrop, outputSize, resizeCrop, type Handle, type Size } from '../imaging/geometry'
import { Renderer } from '../imaging/renderer'
import type { ViewRequest, ViewResponse } from '../imaging/view.worker'
import ViewWorker from '../imaging/view.worker?worker'
import { useBaseThumb, useDisplayThumb } from '../imaging/thumbs'
import { setArchived, setPhotoCategory, updateMeta } from '../model/ops'
import { useSettings } from '../model/settings'
import { closeViewer, commit, useDoc, useLibrary, useUi } from '../model/store'
import { ASPECTS, DEFAULT_EDITS, FULL_CROP, metaOf, photoCategory, type AdjustmentKey, type Aspect, type Crop, type Edits, type PhotoFile } from '../model/types'
import { toast } from './toast'
import { IconButton, RangeInput } from './ui'

const MAX_VIEW_SIDE = 4096
const BITMAP_CACHE_LIMIT = 5

const bitmaps = new Map<string, Promise<ImageBitmap>>()

const decoder = new ViewWorker()
const decoded = new Map<string, (bitmap: ImageBitmap) => void>()
decoder.onmessage = ({ data }: MessageEvent<ViewResponse>) => {
  decoded.get(data.key)!(data.bitmap)
  decoded.delete(data.key)
}

const loadBitmap = (file: PhotoFile) => {
  const key = `${file.id}|${file.lastModified}`
  const cached = bitmaps.get(key)
  if (cached) return cached
  const loading = file.handle.getFile().then(
    (data) =>
      new Promise<ImageBitmap>((resolve) => {
        decoded.set(key, resolve)
        decoder.postMessage({ key, file: data, maxSide: MAX_VIEW_SIDE } satisfies ViewRequest)
      }),
  )
  bitmaps.set(key, loading)
  for (const stale of [...bitmaps.keys()].slice(0, Math.max(0, bitmaps.size - BITMAP_CACHE_LIMIT))) bitmaps.delete(stale)
  return loading
}

const setEdits = (id: string, update: (edits: Edits) => Edits, key: string) =>
  commit((doc) => updateMeta(doc, [id], { edits: update(metaOf(doc, id).edits) }), `edit:${id}:${key}`)

const fitInto = (box: Size, aspect: number): Size => (box.w / box.h > aspect ? { w: box.h * aspect, h: box.h } : { w: box.w, h: box.w / aspect })

const HANDLES: Handle[] = [
  { x: -1, y: -1 },
  { x: 0, y: -1 },
  { x: 1, y: -1 },
  { x: 1, y: 0 },
  { x: 1, y: 1 },
  { x: 0, y: 1 },
  { x: -1, y: 1 },
  { x: -1, y: 0 },
]

const CropOverlay = ({ id, size, edits }: { id: string; size: Size; edits: Edits }) => {
  const overlay = useRef<HTMLDivElement>(null)
  const ratio = aspectRatio(edits.aspect, frameSize(size, edits.quarter))
  const { crop } = edits

  const start = (event: ReactPointerEvent, handle: Handle | null) => {
    event.stopPropagation()
    event.currentTarget.setPointerCapture(event.pointerId)
    const rect = overlay.current!.getBoundingClientRect()
    const origin = { x: event.clientX, y: event.clientY }
    const initial = crop
    const move = (moveEvent: PointerEvent) => {
      const next: Crop = handle
        ? resizeCrop(size, edits, initial, handle, (moveEvent.clientX - rect.left) / rect.width, (moveEvent.clientY - rect.top) / rect.height, ratio)
        : moveCrop(size, edits, initial, (moveEvent.clientX - origin.x) / rect.width, (moveEvent.clientY - origin.y) / rect.height)
      setEdits(id, (current) => ({ ...current, crop: next }), 'crop')
    }
    const end = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', end)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', end)
  }

  return (
    <div ref={overlay} className="absolute inset-0 overflow-hidden">
      <div
        onPointerDown={(event) => start(event, null)}
        className="absolute cursor-move shadow-[0_0_0_200vmax_rgba(0,0,0,0.6)] ring-1 ring-white"
        style={{ left: `${crop.x * 100}%`, top: `${crop.y * 100}%`, width: `${crop.w * 100}%`, height: `${crop.h * 100}%` }}
      >
        <div className="pointer-events-none absolute inset-0 grid grid-cols-3 grid-rows-3">
          {Array.from({ length: 9 }, (_, cell) => (
            <div key={cell} className="border-[0.5px] border-white/30" />
          ))}
        </div>
        {HANDLES.map((handle) => (
          <div
            key={`${handle.x}${handle.y}`}
            onPointerDown={(event) => start(event, handle)}
            className="absolute size-4 -translate-1/2 rounded-sm border-2 border-white bg-black/40"
            style={{
              left: `${(handle.x + 1) * 50}%`,
              top: `${(handle.y + 1) * 50}%`,
              cursor: handle.x === 0 ? 'ns-resize' : handle.y === 0 ? 'ew-resize' : handle.x === handle.y ? 'nwse-resize' : 'nesw-resize',
            }}
          />
        ))}
      </div>
    </div>
  )
}

const Stage = ({ id, file, size, edits, cropping, comparing }: { id: string; file: PhotoFile; size: Size; edits: Edits; cropping: boolean; comparing: boolean }) => {
  const container = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const renderer = useRef<Renderer | null>(null)
  const [box, setBox] = useState<Size>({ w: 0, h: 0 })
  const [bitmap, setBitmap] = useState<ImageBitmap | null>(null)
  const thumb = useDisplayThumb(id)

  useEffect(() => {
    const observer = new ResizeObserver(([entry]) => setBox({ w: entry.contentRect.width, h: entry.contentRect.height }))
    observer.observe(container.current!)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    let current = true
    void loadBitmap(file).then((loaded) => {
      if (current) setBitmap(loaded)
    })
    return () => {
      current = false
    }
  }, [file])

  const shown = comparing ? DEFAULT_EDITS : edits
  const frame = frameSize(size, shown.quarter)
  const output = cropping ? frame : outputSize(size, shown)
  const display = box.w > 0 ? fitInto(box, output.w / output.h) : { w: 0, h: 0 }

  useLayoutEffect(() => {
    if (!bitmap || display.w === 0) return
    renderer.current ??= new Renderer(canvas.current!)
    const ratio = window.devicePixelRatio
    renderer.current.render(bitmap, shown, cropping ? FULL_CROP : shown.crop, { w: Math.round(display.w * ratio), h: Math.round(display.h * ratio) })
  }, [bitmap, shown, cropping, display.w, display.h])

  return (
    <div ref={container} className={`relative min-h-0 min-w-0 flex-1 ${cropping ? 'm-10' : 'm-4'}`}>
      <div className="absolute top-1/2 left-1/2 -translate-1/2" style={{ width: display.w, height: display.h }}>
        {!bitmap && thumb?.url && <img src={thumb.url} alt="" className="absolute inset-0 size-full object-contain opacity-80 blur-[1px]" />}
        <canvas ref={canvas} className="absolute inset-0 size-full" style={{ visibility: bitmap ? 'visible' : 'hidden' }} />
        {cropping && <CropOverlay id={id} size={size} edits={edits} />}
      </div>
    </div>
  )
}

const ADJUSTMENT_GROUPS: { title: 'light' | 'color' | 'effects'; keys: { key: AdjustmentKey; min: number }[] }[] = [
  {
    title: 'light',
    keys: [
      { key: 'exposure', min: -100 },
      { key: 'brightness', min: -100 },
      { key: 'contrast', min: -100 },
      { key: 'highlights', min: -100 },
      { key: 'shadows', min: -100 },
    ],
  },
  {
    title: 'color',
    keys: [
      { key: 'temperature', min: -100 },
      { key: 'tint', min: -100 },
      { key: 'saturation', min: -100 },
      { key: 'vibrance', min: -100 },
    ],
  },
  {
    title: 'effects',
    keys: [
      { key: 'sharpness', min: 0 },
      { key: 'vignette', min: -100 },
      { key: 'fade', min: 0 },
      { key: 'grain', min: 0 },
    ],
  },
]

const aspectLabel = (aspect: Aspect) => (aspect === 'free' ? t('aspectFree') : aspect === 'original' ? t('aspectOriginal') : aspect)

const EditPanel = ({ id, size, edits, cropping, setCropping }: { id: string; size: Size; edits: Edits; cropping: boolean; setCropping: (value: boolean) => void }) => {
  const baseCrop = useRef(edits.crop)
  const copied = useUi((state) => state.copiedAdjustments)

  const turn = (delta: 1 | -1) =>
    setEdits(
      id,
      (current) => {
        const quarter = ((current.quarter + delta + 4) % 4) as Edits['quarter']
        const next = { ...current, quarter }
        return { ...next, crop: fitCrop(size, next, FULL_CROP, aspectRatio(current.aspect, frameSize(size, quarter))) }
      },
      'quarter',
    )

  return (
    <aside className="flex w-80 shrink-0 flex-col overflow-y-auto border-l border-white/10 bg-[#121317] text-white">
      <section className="space-y-3 border-b border-white/10 p-4">
        <div className="flex items-center gap-1">
          <IconButton tone="overlay" icon={CropIcon} label={t('crop')} active={cropping} onClick={() => setCropping(!cropping)} />
          <IconButton tone="overlay" icon={RotateCcw} label={t('rotateLeft')} onClick={() => turn(-1)} />
          <IconButton tone="overlay" icon={RotateCw} label={t('rotateRight')} onClick={() => turn(1)} />
          <IconButton tone="overlay" icon={FlipHorizontal2} label={t('flip')} active={edits.flip} onClick={() => setEdits(id, (current) => ({ ...current, flip: !current.flip }), 'flip')} />
          <div className="flex-1" />
          <IconButton
            tone="overlay"
            icon={Undo2}
            label={t('resetCrop')}
            onClick={() => setEdits(id, (current) => ({ ...current, crop: FULL_CROP, angle: 0, quarter: 0, flip: false, aspect: 'free' }), 'reset-crop')}
          />
        </div>
        <label className="block text-xs text-white/60">
          <span className="flex justify-between">
            {t('angle')} <span className="tabular-nums">{edits.angle.toFixed(1)}°</span>
          </span>
          <RangeInput
            label={t('angle')}
            min={-45}
            max={45}
            step={0.1}
            value={edits.angle}
            onStart={() => {
              baseCrop.current = edits.crop
            }}
            onReset={() => setEdits(id, (current) => ({ ...current, angle: 0, crop: fitCrop(size, { ...current, angle: 0 }, baseCrop.current, null) }), 'angle')}
            onChange={(angle) => setEdits(id, (current) => ({ ...current, angle, crop: fitCrop(size, { ...current, angle }, baseCrop.current, null) }), 'angle')}
            className="mt-1 w-full"
          />
        </label>
        <div>
          <div className="mb-1.5 text-xs text-white/60">{t('aspect')}</div>
          <div className="flex flex-wrap gap-1">
            {ASPECTS.map((aspect) => (
              <button
                key={aspect}
                type="button"
                aria-pressed={edits.aspect === aspect}
                onClick={() =>
                  setEdits(
                    id,
                    (current) => ({ ...current, aspect, crop: fitCrop(size, current, current.crop, aspectRatio(aspect, frameSize(size, current.quarter))) }),
                    'aspect',
                  )
                }
                className={`rounded-md px-2 py-1 text-xs ${edits.aspect === aspect ? 'bg-accent text-white' : 'bg-white/8 text-white/70 hover:bg-white/15'}`}
              >
                {aspectLabel(aspect)}
              </button>
            ))}
          </div>
        </div>
      </section>
      {ADJUSTMENT_GROUPS.map((group) => (
        <section key={group.title} className="space-y-2.5 border-b border-white/10 p-4">
          <h3 className="text-xs font-semibold tracking-wide text-white/50 uppercase">{t(group.title)}</h3>
          {group.keys.map(({ key, min }) => (
            <label key={key} className="block text-xs text-white/75" title={t('doubleClickReset')}>
              <span className="flex justify-between">
                {t(key)} <span className="text-white/50 tabular-nums">{edits.adjust[key]}</span>
              </span>
              <RangeInput
                label={t(key)}
                min={min}
                max={100}
                step={1}
                value={edits.adjust[key]}
                onReset={() => setEdits(id, (current) => ({ ...current, adjust: { ...current.adjust, [key]: 0 } }), key)}
                onChange={(value) => setEdits(id, (current) => ({ ...current, adjust: { ...current.adjust, [key]: value } }), key)}
                className="mt-1 w-full"
              />
            </label>
          ))}
        </section>
      ))}
      <section className="flex gap-1 p-4">
        <IconButton tone="overlay" icon={ClipboardCopy} label={t('copyAdjustments')} onClick={() => useUi.setState({ copiedAdjustments: edits.adjust })} />
        <IconButton
          tone="overlay"
          icon={ClipboardPaste}
          label={t('pasteAdjustments')}
          disabled={copied === null}
          onClick={() => copied && setEdits(id, (current) => ({ ...current, adjust: copied }), 'paste')}
        />
        <div className="flex-1" />
        <IconButton tone="overlay" icon={Undo2} label={t('resetAll')} onClick={() => setEdits(id, () => DEFAULT_EDITS, 'reset')} />
      </section>
    </aside>
  )
}

const CategoryPicker = ({ id }: { id: string }) => {
  const categories = useDoc((state) => state.doc.categories)
  const current = useDoc((state) => photoCategory(state.doc, id))
  return (
    <div className="mr-2 flex gap-1">
      {categories.map((category, index) => (
        <button
          key={category.id}
          type="button"
          title={t('photoCategoryHint', { name: category.name, key: index + 1 })}
          aria-pressed={category.id === current}
          onClick={() => commit((doc) => setPhotoCategory(doc, [id], category.id))}
          style={category.id === current ? { backgroundColor: category.color } : undefined}
          className={`flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition-colors ${
            category.id === current ? 'text-white' : 'text-white/70 hover:bg-white/15 hover:text-white'
          }`}
        >
          {category.id !== current && <span className="size-2 rounded-full" style={{ backgroundColor: category.color }} />}
          {category.name}
        </button>
      ))}
    </div>
  )
}

const ViewerScreen = ({ ids, id }: { ids: string[]; id: string }) => {
  const file = useLibrary((state) => state.byId.get(id))
  const meta = useDoc((state) => metaOf(state.doc, id))
  const base = useBaseThumb(id)
  const editing = useSettings((state) => state.editPanelOpen)
  const setEditing = (editPanelOpen: boolean) => useSettings.setState({ editPanelOpen })
  const [cropping, setCropping] = useState(false)
  const [comparing, setComparing] = useState(false)
  const index = ids.indexOf(id)
  const size = base?.url ? base.size : null

  const go = (delta: number) => {
    const next = ids[index + delta]
    if (next) useUi.setState({ viewer: { ...useUi.getState().viewer!, id: next } })
  }

  const archiveAndAdvance = () => {
    commit((doc) => setArchived(doc, [id], !meta.archived))
    const remaining = ids.filter((candidate) => candidate !== id)
    const next = remaining[Math.min(index, remaining.length - 1)]
    if (!next) return closeViewer()
    useUi.setState({ viewer: { ...useUi.getState().viewer!, ids: remaining, id: next } })
  }

  const copy = () => {
    if (!file) return
    copyPhoto(file, meta.edits, useSettings.getState().exportLongSide).then(
      () => toast(t('copiedPhoto', { number: index + 1 })),
      (error: unknown) => toast(t('copyFailed', { reason: String(error) })),
    )
  }

  useEffect(() => {
    const neighbours = [ids[index - 1], ids[index + 1]].flatMap((neighbour) => {
      const neighbourFile = neighbour ? useLibrary.getState().byId.get(neighbour) : undefined
      return neighbourFile ? [neighbourFile] : []
    })
    for (const neighbour of neighbours) void loadBitmap(neighbour)
  }, [ids, index])

  const onKeyDown = useEffectEvent((event: KeyboardEvent) => {
    if (event.target instanceof HTMLTextAreaElement || (event.target instanceof HTMLInputElement && event.target.type !== 'range')) return
    if (event.key === 'Escape') return cropping ? setCropping(false) : closeViewer()
    if (event.key === 'Enter' && cropping) return setCropping(false)
    if (event.ctrlKey || event.metaKey) {
      if (event.key.toLowerCase() === 'c') {
        event.preventDefault()
        copy()
      }
      return
    }
    if (event.key === 'ArrowLeft') go(-1)
    else if (event.key === 'ArrowRight') go(1)
    else if (event.key === 'a' || event.key === 'Delete') archiveAndAdvance()
    else if (event.key === 'c' || event.key === ' ') {
      event.preventDefault()
      commit((doc) => updateMeta(doc, [id], { checked: !metaOf(doc, id).checked }))
    } else if (event.key === 'e') setEditing(!editing)
    else if (event.key === 'r') {
      setEditing(true)
      setCropping((value) => !value)
    } else if (event.key === '\\') setComparing(true)
    else if (/^[1-9]$/.test(event.key)) {
      const category = useDoc.getState().doc.categories[Number(event.key) - 1]
      if (category) commit((doc) => setPhotoCategory(doc, [id], category.id))
    }
  })

  useEffect(() => {
    const up = (event: KeyboardEvent) => {
      if (event.key === '\\') setComparing(false)
    }
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', up)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', up)
    }
  }, [])

  return (
    <div role="dialog" aria-modal="true" className="fixed inset-0 z-50 flex flex-col bg-[#08090b] text-white">
      <header className="flex h-14 shrink-0 items-center gap-1 px-3">
        <IconButton tone="overlay" icon={X} label={t('close')} onClick={closeViewer} />
        <div className="ml-2 min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{id}</div>
          <div className="text-xs text-white/50 tabular-nums">
            {index + 1} / {ids.length}
          </div>
        </div>
        <CategoryPicker id={id} />
        <IconButton
          tone="overlay"
          icon={Check}
          label={meta.checked ? t('uncheck') : t('check')}
          active={meta.checked}
          onClick={() => commit((doc) => updateMeta(doc, [id], { checked: !meta.checked }))}
        />
        <IconButton tone="overlay" icon={meta.archived ? ArchiveRestore : Archive} label={meta.archived ? t('restore') : t('archive')} onClick={archiveAndAdvance} />
        <IconButton tone="overlay" icon={Copy} label={t('copyPhoto')} onClick={copy} />
        <button
          type="button"
          title={t('compare')}
          aria-label={t('compare')}
          onPointerDown={() => setComparing(true)}
          onPointerUp={() => setComparing(false)}
          onPointerLeave={() => setComparing(false)}
          className="grid size-8 place-items-center rounded-lg text-white/80 hover:bg-white/15 hover:text-white"
        >
          <Eye className="size-4" />
        </button>
        <IconButton tone="overlay" icon={SlidersHorizontal} label={t('editPanel')} active={editing} onClick={() => setEditing(!editing)} />
      </header>
      <div className="relative flex min-h-0 flex-1">
        <button
          type="button"
          aria-label={t('previous')}
          title={t('previous')}
          disabled={index === 0}
          onClick={() => go(-1)}
          className="absolute top-1/2 left-3 z-10 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white backdrop-blur transition hover:bg-white/20 disabled:opacity-0"
        >
          <ChevronLeft className="size-6" />
        </button>
        {file && size ? (
          <Stage key={id} id={id} file={file} size={size} edits={meta.edits} cropping={cropping && editing} comparing={comparing} />
        ) : (
          <div className="grid flex-1 place-items-center text-sm text-white/50">{t('loading')}</div>
        )}
        <button
          type="button"
          aria-label={t('next')}
          title={t('next')}
          disabled={index === ids.length - 1}
          onClick={() => go(1)}
          className={`absolute top-1/2 z-10 grid size-11 -translate-y-1/2 place-items-center rounded-full bg-white/10 text-white backdrop-blur transition hover:bg-white/20 disabled:opacity-0 ${editing ? 'right-[21.75rem]' : 'right-3'}`}
        >
          <ChevronRight className="size-6" />
        </button>
        {editing && size && <EditPanel id={id} size={size} edits={meta.edits} cropping={cropping} setCropping={setCropping} />}
      </div>
    </div>
  )
}

export const Viewer = () => {
  const viewer = useUi((state) => state.viewer)
  if (!viewer) return null
  return createPortal(<ViewerScreen ids={viewer.ids} id={viewer.id} />, document.body)
}
