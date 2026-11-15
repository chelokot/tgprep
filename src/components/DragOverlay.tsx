import { createPortal } from 'react-dom'
import { useDrag, type Box } from '../dnd/drag'
import { t } from '../i18n'
import { useThumbs } from '../imaging/thumbs'
import { useDoc } from '../model/store'

const boxStyle = (box: Box) => ({ left: box.left, top: box.top, width: box.width, height: box.height })

const Ghost = () => {
  const payload = useDrag((state) => state.payload)
  const pointer = useDrag((state) => state.pointer)
  const postNumber = useDoc((state) => (payload?.kind === 'post' ? state.doc.posts.findIndex((post) => post.id === payload.postId) + 1 : 0))
  if (!payload) return null
  const style = { transform: `translate(${pointer.x + 14}px, ${pointer.y + 14}px)` }
  if (payload.kind === 'post')
    return (
      <div style={style} className="fixed top-0 left-0 rounded-xl bg-accent px-3 py-2 text-sm font-semibold text-white shadow-2xl">
        #{postNumber}
      </div>
    )
  const thumbs = useThumbs.getState()
  const urls = payload.ids.slice(0, 3).map((id) => thumbs[id]?.url ?? null)
  return (
    <div style={style} className="fixed top-0 left-0">
      {urls.map((url, index) => (
        <div
          key={index}
          className="absolute size-20 overflow-hidden rounded-xl border-2 border-white bg-raised shadow-2xl"
          style={{ transform: `rotate(${(index - 1) * 6}deg) translate(${index * 6}px, ${index * 4}px)`, zIndex: 3 - index }}
        >
          {url && <img src={url} alt="" className="size-full object-cover" />}
        </div>
      ))}
      {payload.ids.length > 1 && (
        <span className="absolute -top-2 left-16 z-10 rounded-full bg-accent px-2 py-0.5 text-xs font-bold text-white shadow">{payload.ids.length}</span>
      )}
    </div>
  )
}

const Indicator = () => {
  const target = useDrag((state) => state.target)
  const color = useDoc((state) => (target?.kind === 'gap' ? state.doc.categories.find((category) => category.id === target.category)?.color : undefined))
  if (!target) return null
  if (target.kind === 'library')
    return (
      <div style={boxStyle(target.ring)} className="fixed grid place-items-center rounded-lg bg-danger/10 ring-2 ring-danger ring-inset">
        <span className="rounded-full bg-danger px-3 py-1.5 text-sm font-semibold text-white shadow-lg">{t('removeHere')}</span>
      </div>
    )
  if (target.kind === 'gap')
    return (
      <div style={{ ...boxStyle(target.line), backgroundColor: color }} className="fixed rounded-full shadow-[0_0_0.75rem_var(--color-accent)]">
        <span style={{ backgroundColor: color }} className="absolute top-1/2 left-1/2 -translate-1/2 rounded-full px-2.5 py-1 text-xs font-semibold whitespace-nowrap text-white shadow-lg">
          + {t('newPostHere')}
        </span>
      </div>
    )
  return (
    <>
      <div style={boxStyle(target.ring)} className={`fixed rounded-2xl ring-2 ring-inset ${target.valid ? 'ring-accent/70' : 'bg-danger/10 ring-danger'}`}>
        {!target.valid && (
          <span className="absolute top-2 right-2 rounded-full bg-danger px-2.5 py-1 text-xs font-semibold text-white shadow">{t('maxPhotos')}</span>
        )}
      </div>
      {target.line && <div style={boxStyle(target.line)} className="fixed rounded-full bg-accent shadow-[0_0_0.75rem_var(--color-accent)]" />}
    </>
  )
}

export const DragOverlay = () =>
  createPortal(
    <div className="pointer-events-none fixed inset-0 z-[60]">
      <Indicator />
      <Ghost />
    </div>,
    document.body,
  )
