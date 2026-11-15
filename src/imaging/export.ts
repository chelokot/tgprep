import { EXPORT_DIR } from '../model/persistence'
import type { Edits, PhotoFile, Post } from '../model/types'
import { fitWithin, outputSize } from './geometry'
import { Renderer } from './renderer'

const PNG_CACHE_LIMIT = 24

let renderer: Renderer | null = null
let chain: Promise<unknown> = Promise.resolve()

const serialize = <T>(job: () => Promise<T>) => {
  const result = chain.then(job)
  chain = result.catch(() => null)
  return result
}

export const renderPhoto = (file: PhotoFile, edits: Edits, longSide: number | null, type: 'image/png' | 'image/jpeg') =>
  serialize(async () => {
    renderer ??= new Renderer(new OffscreenCanvas(1, 1))
    const original = await createImageBitmap(await file.handle.getFile())
    const output = outputSize({ w: original.width, h: original.height }, edits)
    const target = fitWithin(output, Math.min(longSide ?? Infinity, renderer.maxTextureSize))
    const scale = Math.min(1, target.w / output.w, renderer.maxTextureSize / Math.max(original.width, original.height))
    const source =
      scale < 1
        ? await createImageBitmap(original, {
            resizeWidth: Math.round(original.width * scale),
            resizeHeight: Math.round(original.height * scale),
            resizeQuality: 'high',
          })
        : original
    renderer.render(source, edits, edits.crop, target)
    const blob = await (renderer.canvas as OffscreenCanvas).convertToBlob({ type, quality: 0.93 })
    renderer.forget(source)
    source.close()
    original.close()
    return blob
  })

const pngCache = new Map<string, Promise<Blob>>()

export const preparePng = (file: PhotoFile, edits: Edits, longSide: number | null) => {
  const key = `${file.id}|${file.lastModified}|${longSide}|${JSON.stringify(edits)}`
  const cached = pngCache.get(key)
  if (cached) {
    pngCache.delete(key)
    pngCache.set(key, cached)
    return cached
  }
  const rendering = renderPhoto(file, edits, longSide, 'image/png')
  pngCache.set(key, rendering)
  rendering.catch(() => pngCache.delete(key))
  for (const stale of [...pngCache.keys()].slice(0, Math.max(0, pngCache.size - PNG_CACHE_LIMIT))) pngCache.delete(stale)
  return rendering
}

export const copyPhoto = (file: PhotoFile, edits: Edits, longSide: number | null) =>
  navigator.clipboard.write([new ClipboardItem({ 'image/png': preparePng(file, edits, longSide) })])

export const copyText = (text: string) => navigator.clipboard.writeText(text)

export const exportPost = async (
  root: FileSystemDirectoryHandle,
  post: Post,
  number: number,
  photos: { file: PhotoFile; edits: Edits }[],
  longSide: number | null,
) => {
  const exportRoot = await root.getDirectoryHandle(EXPORT_DIR, { create: true })
  const name = `post-${String(number).padStart(3, '0')}`
  await exportRoot.removeEntry(name, { recursive: true }).catch(() => null)
  const folder = await exportRoot.getDirectoryHandle(name, { create: true })
  const write = async (fileName: string, data: Blob | string) => {
    const writable = await (await folder.getFileHandle(fileName, { create: true })).createWritable()
    await writable.write(data)
    await writable.close()
  }
  await Promise.all(
    photos.map(async ({ file, edits }, index) =>
      write(`${String(index + 1).padStart(2, '0')}.jpg`, await renderPhoto(file, edits, longSide, 'image/jpeg')),
    ),
  )
  if (post.caption.trim() !== '') await write('caption.txt', post.caption)
  return `${EXPORT_DIR}/${name}`
}
