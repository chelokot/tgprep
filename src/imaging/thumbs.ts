import { createStore, get, set } from 'idb-keyval'
import { useEffect, useState } from 'react'
import { create } from 'zustand'
import { useDoc, useLibrary } from '../model/store'
import { isEdited, metaOf, type Edits, type PhotoFile } from '../model/types'
import { fitWithin, outputSize, type Size } from './geometry'
import { Renderer } from './renderer'
import type { ThumbRequest, ThumbResponse } from './thumb.worker'
import ThumbWorker from './thumb.worker?worker'

const THUMB_LONG_SIDE = 720
const EDITED_DELAY_MS = 250
const WORKER_COUNT = Math.max(1, Math.min(4, navigator.hardwareConcurrency - 1))

export type Thumb = { url: string; blob: Blob; size: Size } | { url: null }

export const useThumbs = create<Record<string, Thumb>>(() => ({}))

const cache = createStore('tgprep-thumbs', 'thumbs')
const cacheKey = (file: PhotoFile) => `${file.id}|${file.size}|${file.lastModified}`

const requested = new Set<string>()
const queue: PhotoFile[] = []
const idle: Worker[] = []
const inFlight = new Map<string, (response: ThumbResponse['result']) => void>()
let pendingUpdates: Record<string, Thumb> = {}
let flushScheduled = false

const publish = (id: string, thumb: Thumb) => {
  pendingUpdates[id] = thumb
  if (flushScheduled) return
  flushScheduled = true
  requestAnimationFrame(() => {
    useThumbs.setState(pendingUpdates)
    pendingUpdates = {}
    flushScheduled = false
  })
}

const store = (id: string, result: ThumbResponse['result']) =>
  publish(id, result ? { url: URL.createObjectURL(result.blob), blob: result.blob, size: { w: result.w, h: result.h } } : { url: null })

const pump = () => {
  while (idle.length > 0 && queue.length > 0) {
    const file = queue.pop()!
    const worker = idle.pop()!
    const key = cacheKey(file)
    inFlight.set(key, (result) => {
      idle.push(worker)
      store(file.id, result)
      if (result) void set(key, result, cache)
      pump()
    })
    void file.handle.getFile().then((data) => worker.postMessage({ key, file: data, longSide: THUMB_LONG_SIDE } satisfies ThumbRequest))
  }
}

for (let index = 0; index < WORKER_COUNT; index++) {
  const worker = new ThumbWorker()
  worker.onmessage = ({ data }: MessageEvent<ThumbResponse>) => {
    const resolve = inFlight.get(data.key)!
    inFlight.delete(data.key)
    resolve(data.result)
  }
  idle.push(worker)
}

const requestThumb = async (file: PhotoFile) => {
  if (requested.has(file.id)) return
  requested.add(file.id)
  const cached = await get<ThumbResponse['result']>(cacheKey(file), cache)
  if (cached) return store(file.id, cached)
  queue.push(file)
  pump()
}

export const useBaseThumb = (id: string): Thumb | undefined => {
  const file = useLibrary((state) => state.byId.get(id))
  useEffect(() => {
    if (file) void requestThumb(file)
  }, [file])
  return useThumbs((state) => state[id])
}

let thumbRenderer: Renderer | null = null
const editedUrls = new Map<string, string>()
let renderChain = Promise.resolve()

const renderEditedThumb = (blob: Blob, size: Size, edits: Edits, key: string) => {
  renderChain = renderChain.then(async () => {
    if (editedUrls.has(key)) return
    thumbRenderer ??= new Renderer(new OffscreenCanvas(1, 1))
    const bitmap = await createImageBitmap(blob)
    thumbRenderer.render(bitmap, edits, edits.crop, fitWithin(outputSize(size, edits), THUMB_LONG_SIDE))
    const rendered = await (thumbRenderer.canvas as OffscreenCanvas).convertToBlob({ type: 'image/webp', quality: 0.85 })
    thumbRenderer.forget(bitmap)
    bitmap.close()
    editedUrls.set(key, URL.createObjectURL(rendered))
  })
  return renderChain.then(() => editedUrls.get(key)!)
}

export type DisplayThumb = { url: string | null; aspect: number; edited: boolean }

export const useDisplayThumb = (id: string): DisplayThumb | null => {
  const base = useBaseThumb(id)
  const edits = useDoc((state) => metaOf(state.doc, id).edits)
  const edited = isEdited(edits)
  const key = `${id}|${JSON.stringify(edits)}`
  const [renderedUrl, setRenderedUrl] = useState<string | null>(null)
  const editedUrl = editedUrls.get(key) ?? renderedUrl

  useEffect(() => {
    if (!edited || !base?.url || editedUrls.has(key)) return
    let cancelled = false
    const timer = window.setTimeout(
      () =>
        void renderEditedThumb(base.blob, base.size, edits, key).then((url) => {
          if (!cancelled) setRenderedUrl(url)
        }),
      EDITED_DELAY_MS,
    )
    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [edited, base, edits, key])

  if (!base) return null
  if (!base.url) return { url: null, aspect: 1, edited }
  const output = outputSize(base.size, edits)
  return { url: edited ? editedUrl : base.url, aspect: output.w / output.h, edited }
}

export const resetThumbs = () => {
  requested.clear()
  queue.length = 0
  useThumbs.setState({}, true)
}
