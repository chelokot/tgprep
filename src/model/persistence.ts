import { createStore, get, set } from 'idb-keyval'
import { resetThumbs } from '../imaging/thumbs'
import { resetDoc, useDoc, useLibrary } from './store'
import { docSchema, EMPTY_DOC, type Doc, type PhotoFile } from './types'

const STATE_FILE = '.tgprep.json'
export const EXPORT_DIR = 'tgprep-export'
const IMAGE_PATTERN = /\.(jpe?g|png|webp|avif|gif|bmp)$/i
const RECENT_LIMIT = 6
const SAVE_DELAY_MS = 400

const handles = createStore('tgprep', 'handles')
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' })

export type RecentFolder = { name: string; handle: FileSystemDirectoryHandle }

export const loadRecentFolders = async () => (await get<RecentFolder[]>('recent', handles)) ?? []

const rememberFolder = async (handle: FileSystemDirectoryHandle) => {
  const recent = await loadRecentFolders()
  const others = await Promise.all(recent.map(async (folder) => ((await folder.handle.isSameEntry(handle)) ? null : folder)))
  const next = [{ name: handle.name, handle }, ...others.filter((folder) => folder !== null)].slice(0, RECENT_LIMIT)
  await set('recent', next, handles)
}

const scan = async (directory: FileSystemDirectoryHandle, prefix: string): Promise<PhotoFile[]> => {
  const pending: Promise<PhotoFile[]>[] = []
  for await (const entry of directory.values()) {
    if (entry.name.startsWith('.') || entry.name === EXPORT_DIR) continue
    if (entry.kind === 'directory') pending.push(scan(entry, `${prefix}${entry.name}/`))
    else if (IMAGE_PATTERN.test(entry.name))
      pending.push(
        entry.getFile().then((file) => [
          { id: `${prefix}${entry.name}`, name: entry.name, handle: entry, size: file.size, lastModified: file.lastModified },
        ]),
      )
  }
  return (await Promise.all(pending)).flat()
}

const readDoc = async (root: FileSystemDirectoryHandle): Promise<Doc> => {
  const handle = await root.getFileHandle(STATE_FILE).catch(() => null)
  if (!handle) return EMPTY_DOC
  const parsed = docSchema.safeParse(JSON.parse(await (await handle.getFile()).text()))
  return parsed.success ? parsed.data : EMPTY_DOC
}

let stopSaving = () => {}

const startSaving = (root: FileSystemDirectoryHandle) => {
  let timer = 0
  let saved = useDoc.getState().doc
  let queue = Promise.resolve()
  const write = async (doc: Doc) => {
    const writable = await (await root.getFileHandle(STATE_FILE, { create: true })).createWritable()
    await writable.write(JSON.stringify(doc))
    await writable.close()
  }
  const unsubscribe = useDoc.subscribe(({ doc }) => {
    if (doc === saved) return
    window.clearTimeout(timer)
    timer = window.setTimeout(() => {
      saved = doc
      queue = queue.then(() => write(doc))
    }, SAVE_DELAY_MS)
  })
  return () => {
    window.clearTimeout(timer)
    unsubscribe()
  }
}

export const openFolder = async (root: FileSystemDirectoryHandle) => {
  if ((await root.queryPermission({ mode: 'readwrite' })) !== 'granted' && (await root.requestPermission({ mode: 'readwrite' })) !== 'granted')
    return false
  stopSaving()
  const [files, doc] = await Promise.all([scan(root, ''), readDoc(root)])
  files.sort((a, b) => collator.compare(a.id, b.id))
  resetThumbs()
  useLibrary.setState({ root, files, byId: new Map(files.map((file) => [file.id, file])) })
  resetDoc(doc)
  stopSaving = startSaving(root)
  await rememberFolder(root)
  return true
}

export const pickFolder = async () => openFolder(await window.showDirectoryPicker({ id: 'tgprep', mode: 'readwrite' }))
