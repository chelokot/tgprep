import { Archive, ArchiveRestore, Check, ImageOff, SlidersHorizontal } from 'lucide-react'
import type { CSSProperties, PointerEvent, ReactNode } from 'react'
import { t } from '../i18n'
import type { DisplayThumb } from '../imaging/thumbs'
import { setArchived, updateMeta } from '../model/ops'
import { commit, useDoc, useUi, type ViewKey } from '../model/store'
import { metaOf } from '../model/types'

const stop = (event: PointerEvent) => event.stopPropagation()

export const CornerButton = ({ label, onClick, className, children }: { label: string; onClick: () => void; className: string; children: ReactNode }) => (
  <button
    type="button"
    title={label}
    aria-label={label}
    onPointerDown={stop}
    onClick={(event) => {
      event.stopPropagation()
      onClick()
    }}
    className={`absolute grid size-6 place-items-center rounded-full transition ${className}`}
  >
    {children}
  </button>
)

export const Tile = ({
  id,
  view,
  thumb,
  fit,
  onPointerDown,
  actions,
  className = '',
  style,
  children,
}: {
  id: string
  view: ViewKey
  thumb: DisplayThumb | null
  fit: 'contain' | 'cover'
  onPointerDown: (event: PointerEvent) => void
  actions: boolean
  className?: string
  style?: CSSProperties
  children?: ReactNode
}) => {
  const meta = useDoc((state) => metaOf(state.doc, id))
  const selected = useUi((state) => state.selection.has(id))
  const highlighted = useUi((state) => state.highlight !== null && state.highlight.id === id && state.highlight.view === view)

  return (
    <div
      data-photo={id}
      onPointerDown={onPointerDown}
      style={style}
      className={`group/tile relative cursor-pointer select-none overflow-hidden rounded-lg bg-raised ${
        selected ? 'ring-2 ring-accent ring-offset-2 ring-offset-panel' : highlighted ? 'animate-glow ring-2 ring-accent/70' : ''
      } ${className}`}
    >
      {thumb?.url ? (
        <img src={thumb.url} alt="" draggable={false} decoding="async" className={`size-full ${fit === 'contain' ? 'object-contain' : 'object-cover'}`} />
      ) : (
        <div className={`grid size-full place-items-center text-faint ${thumb ? '' : 'animate-pulse'}`}>{thumb && <ImageOff className="size-5" />}</div>
      )}
      {thumb?.edited && (
        <div title={t('edited')} className="absolute right-1.5 bottom-1.5 grid size-5 place-items-center rounded-full bg-black/55 text-white backdrop-blur">
          <SlidersHorizontal className="size-3" strokeWidth={2.5} />
        </div>
      )}
      {children}
      <CornerButton
        label={meta.checked ? t('uncheck') : t('check')}
        onClick={() => commit((doc) => updateMeta(doc, [id], { checked: !meta.checked }))}
        className={`top-1.5 right-1.5 ${
          meta.checked
            ? 'bg-ok text-white shadow'
            : 'border-2 border-white/90 bg-black/20 text-transparent opacity-0 shadow group-hover/tile:opacity-100 hover:bg-black/40'
        }`}
      >
        <Check className="size-3.5" strokeWidth={3.5} />
      </CornerButton>
      {actions && (
        <CornerButton
          label={meta.archived ? t('restore') : t('archive')}
          onClick={() => commit((doc) => setArchived(doc, [id], !meta.archived))}
          className="top-1.5 left-1.5 bg-black/55 text-white opacity-0 backdrop-blur group-hover/tile:opacity-100 hover:bg-danger"
        >
          {meta.archived ? <ArchiveRestore className="size-3.5" /> : <Archive className="size-3.5" />}
        </CornerButton>
      )}
    </div>
  )
}
