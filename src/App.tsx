import { FolderOpen, Redo2, Send, Undo2 } from 'lucide-react'
import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from 'react'
import { DragOverlay } from './components/DragOverlay'
import { Library } from './components/Library'
import { PostsView } from './components/PostsView'
import { IconButton, Segmented, Toasts } from './components/ui'
import { Viewer } from './components/Viewer'
import { Welcome } from './components/Welcome'
import { t } from './i18n'
import { placementsByPhoto } from './model/ops'
import { pickFolder } from './model/persistence'
import { TELEGRAM_LONG_SIDE, useSettings } from './model/settings'
import { clearSelection, redo, undo, useDoc, useLibrary, useUi, type Mode } from './model/store'

const MIN_PANE = 0.18

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== 'range') || target instanceof HTMLSelectElement

const TopBar = () => {
  const root = useLibrary((state) => state.root)
  const total = useLibrary((state) => state.files.length)
  const posts = useDoc((state) => state.doc.posts)
  const canUndo = useDoc((state) => state.past.length > 0)
  const canRedo = useDoc((state) => state.future.length > 0)
  const mode = useUi((state) => state.mode)
  const exportLongSide = useSettings((state) => state.exportLongSide)

  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b border-line bg-panel px-3">
      <div className="grid size-8 place-items-center rounded-lg bg-accent text-white">
        <Send className="size-4 -translate-x-px" />
      </div>
      <span className="font-semibold text-fg">{t('appName')}</span>
      <button
        type="button"
        title={t('changeFolder')}
        onClick={() => void pickFolder()}
        className="ml-1 flex h-8 items-center gap-1.5 rounded-lg px-2 text-sm text-muted transition hover:bg-raised hover:text-fg"
      >
        <FolderOpen className="size-4" /> {root?.name}
      </button>
      <span className="hidden text-xs text-faint lg:inline">
        {t('stats', { posts: posts.length, placed: placementsByPhoto(posts).size, total })}
      </span>
      <div className="flex-1" />
      <IconButton icon={Undo2} label={t('undo')} disabled={!canUndo} onClick={undo} />
      <IconButton icon={Redo2} label={t('redo')} disabled={!canRedo} onClick={redo} />
      <Segmented
        value={String(exportLongSide)}
        onChange={(value) => useSettings.setState({ exportLongSide: value === 'null' ? null : TELEGRAM_LONG_SIDE })}
        options={[
          { value: String(TELEGRAM_LONG_SIDE), label: t('exportTelegram'), title: t('exportSize') },
          { value: 'null', label: t('exportOriginal'), title: t('exportSize') },
        ]}
      />
      <Segmented<Mode>
        value={mode}
        onChange={(next) => useUi.setState({ mode: next })}
        options={[
          { value: 'compose', label: t('compose'), title: t('composeHint') },
          { value: 'send', label: t('send'), title: t('sendHint') },
        ]}
      />
    </header>
  )
}

const Splitter = ({ index, container }: { index: 0 | 1; container: React.RefObject<HTMLDivElement | null> }) => {
  const start = (event: ReactPointerEvent) => {
    event.preventDefault()
    const width = container.current!.getBoundingClientRect().width
    const origin = event.clientX
    const initial = useSettings.getState().paneWidths
    const move = (moveEvent: PointerEvent) => {
      const delta = (moveEvent.clientX - origin) / width
      const [first, second] = initial
      const next: [number, number] =
        index === 0
          ? [clamp(first + delta, MIN_PANE, first + second - MIN_PANE), first + second - clamp(first + delta, MIN_PANE, first + second - MIN_PANE)]
          : [first, clamp(second + delta, MIN_PANE, 1 - first - MIN_PANE)]
      useSettings.setState({ paneWidths: next })
    }
    const end = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', end)
      delete document.documentElement.dataset.resizing
    }
    document.documentElement.dataset.resizing = ''
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', end)
  }
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onPointerDown={start}
      onDoubleClick={() => useSettings.setState({ paneWidths: [0.34, 0.34] })}
      className="group relative z-10 w-px shrink-0 cursor-col-resize bg-line"
    >
      <div className="absolute inset-y-0 -right-1.5 -left-1.5 transition group-hover:bg-accent/30" />
    </div>
  )
}

const Workspace = () => {
  const container = useRef<HTMLDivElement>(null)
  const [left, middle] = useSettings((state) => state.paneWidths)
  return (
    <div className="flex h-dvh flex-col" onPointerDownCapture={() => useUi.getState().highlight && useUi.setState({ highlight: null })}>
      <TopBar />
      <div ref={container} className="flex min-h-0 flex-1">
        <div style={{ width: `${left * 100}%` }} className="min-w-0">
          <PostsView view="posts-a" />
        </div>
        <Splitter index={0} container={container} />
        <div style={{ width: `${middle * 100}%` }} className="min-w-0">
          <PostsView view="posts-b" />
        </div>
        <Splitter index={1} container={container} />
        <div className="min-w-0 flex-1">
          <Library />
        </div>
      </div>
      <Viewer />
      <DragOverlay />
      <Toasts />
    </div>
  )
}

export const App = () => {
  const root = useLibrary((state) => state.root)

  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (isTyping(event.target)) return
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
        event.preventDefault()
        if (event.shiftKey) redo()
        else undo()
      } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') {
        event.preventDefault()
        redo()
      } else if (event.key === 'Escape' && !useUi.getState().viewer) clearSelection()
    }
    window.addEventListener('keydown', down)
    return () => window.removeEventListener('keydown', down)
  }, [])

  return root ? <Workspace /> : <Welcome />
}
