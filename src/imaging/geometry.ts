import { type Aspect, type Crop, type Edits } from '../model/types'

export type Size = { w: number; h: number }

const EPSILON = 1e-6
const SEARCH_STEPS = 24
const MIN_CROP_PX = 24

export const frameSize = (source: Size, quarter: Edits['quarter']): Size =>
  quarter % 2 === 1 ? { w: source.h, h: source.w } : source

export const outputSize = (source: Size, edits: Edits): Size => {
  const frame = frameSize(source, edits.quarter)
  return { w: edits.crop.w * frame.w, h: edits.crop.h * frame.h }
}

export const fitWithin = (size: Size, longSide: number): Size => {
  const scale = Math.min(1, longSide / Math.max(size.w, size.h))
  return { w: Math.max(1, Math.round(size.w * scale)), h: Math.max(1, Math.round(size.h * scale)) }
}

const quarterInverse = (source: Size, quarter: Edits['quarter']) => {
  if (quarter === 1) return new DOMMatrix([0, -1, 1, 0, 0, source.h])
  if (quarter === 2) return new DOMMatrix([-1, 0, 0, -1, source.w, source.h])
  if (quarter === 3) return new DOMMatrix([0, 1, -1, 0, source.w, 0])
  return new DOMMatrix()
}

export const frameToSourceUv = (source: Size, edits: Edits) => {
  const frame = frameSize(source, edits.quarter)
  const matrix = new DOMMatrix().scaleSelf(1 / source.w, 1 / source.h).multiplySelf(quarterInverse(source, edits.quarter))
  if (edits.flip) matrix.translateSelf(frame.w, 0).scaleSelf(-1, 1)
  return matrix
    .translateSelf(frame.w / 2, frame.h / 2)
    .rotateSelf(-edits.angle)
    .translateSelf(-frame.w / 2, -frame.h / 2)
}

export const outputToSourceUv = (source: Size, edits: Edits, crop: Crop) => {
  const frame = frameSize(source, edits.quarter)
  return frameToSourceUv(source, edits)
    .translateSelf(crop.x * frame.w, crop.y * frame.h)
    .scaleSelf(crop.w * frame.w, crop.h * frame.h)
}

export const cropFits = (source: Size, edits: Edits, crop: Crop) => {
  const frame = frameSize(source, edits.quarter)
  const toSource = frameToSourceUv(source, edits)
  const corners = [
    [crop.x, crop.y],
    [crop.x + crop.w, crop.y],
    [crop.x, crop.y + crop.h],
    [crop.x + crop.w, crop.y + crop.h],
  ]
  const minimum = MIN_CROP_PX - EPSILON
  if (crop.w * frame.w < minimum || crop.h * frame.h < minimum) return false
  return corners.every(([x, y]) => {
    const point = toSource.transformPoint(new DOMPoint(x * frame.w, y * frame.h))
    return point.x >= -EPSILON && point.x <= 1 + EPSILON && point.y >= -EPSILON && point.y <= 1 + EPSILON
  })
}

const lerpCrop = (from: Crop, to: Crop, t: number): Crop => ({
  x: from.x + (to.x - from.x) * t,
  y: from.y + (to.y - from.y) * t,
  w: from.w + (to.w - from.w) * t,
  h: from.h + (to.h - from.h) * t,
})

const furthestFitting = (fits: (crop: Crop) => boolean, from: Crop, to: Crop) => {
  if (fits(to)) return to
  let low = 0
  let high = 1
  for (let step = 0; step < SEARCH_STEPS; step++) {
    const middle = (low + high) / 2
    if (fits(lerpCrop(from, to, middle))) low = middle
    else high = middle
  }
  return lerpCrop(from, to, low)
}

export const constrainCrop = (source: Size, edits: Edits, from: Crop, to: Crop) =>
  furthestFitting((crop) => cropFits(source, edits, crop), from, to)

export const moveCrop = (source: Size, edits: Edits, from: Crop, dx: number, dy: number) => {
  const horizontal = constrainCrop(source, edits, from, { ...from, x: from.x + dx })
  return constrainCrop(source, edits, horizontal, { ...horizontal, y: horizontal.y + dy })
}

const centered = (centerX: number, centerY: number, w: number, h: number): Crop => ({ x: centerX - w / 2, y: centerY - h / 2, w, h })

const largestCentered = (source: Size, edits: Edits, centerX: number, centerY: number, w: number, h: number) =>
  furthestFitting(
    (crop) => cropFits(source, edits, crop),
    centered(centerX, centerY, w * EPSILON, h * EPSILON),
    centered(centerX, centerY, w, h),
  )

export const aspectRatio = (aspect: Aspect, source: Size): number | null => {
  if (aspect === 'free') return null
  if (aspect === 'original') return source.w / source.h
  const [w, h] = aspect.split(':').map(Number)
  return w / h
}

export const fitCrop = (source: Size, edits: Edits, base: Crop, ratio: number | null): Crop => {
  const frame = frameSize(source, edits.quarter)
  const widthPx = ratio === null ? base.w * frame.w : Math.min(frame.w, frame.h * ratio)
  const heightPx = ratio === null ? base.h * frame.h : widthPx / ratio
  const centerX = ratio === null ? base.x + base.w / 2 : 0.5
  const centerY = ratio === null ? base.y + base.h / 2 : 0.5
  const fitted = largestCentered(source, edits, centerX, centerY, widthPx / frame.w, heightPx / frame.h)
  return cropFits(source, edits, fitted) ? fitted : largestCentered(source, edits, 0.5, 0.5, widthPx / frame.w, heightPx / frame.h)
}

export type Handle = { x: -1 | 0 | 1; y: -1 | 0 | 1 }

export const resizeCrop = (source: Size, edits: Edits, start: Crop, handle: Handle, pointerX: number, pointerY: number, ratio: number | null) => {
  const frame = frameSize(source, edits.quarter)
  const left = handle.x === -1 ? Math.min(pointerX, start.x + start.w) : start.x
  const right = handle.x === 1 ? Math.max(pointerX, start.x) : start.x + start.w
  const top = handle.y === -1 ? Math.min(pointerY, start.y + start.h) : start.y
  const bottom = handle.y === 1 ? Math.max(pointerY, start.y) : start.y + start.h
  if (ratio === null) return constrainCrop(source, edits, start, { x: left, y: top, w: right - left, h: bottom - top })

  const normalizedRatio = (ratio * frame.h) / frame.w
  const wide = handle.y === 0 || (handle.x !== 0 && (right - left) / (bottom - top) > normalizedRatio)
  const w = wide ? right - left : (bottom - top) * normalizedRatio
  const h = wide ? (right - left) / normalizedRatio : bottom - top
  const anchorX = handle.x === 1 ? start.x : handle.x === -1 ? start.x + start.w : start.x + start.w / 2
  const anchorY = handle.y === 1 ? start.y : handle.y === -1 ? start.y + start.h : start.y + start.h / 2
  const x = handle.x === 1 ? anchorX : handle.x === -1 ? anchorX - w : anchorX - w / 2
  const y = handle.y === 1 ? anchorY : handle.y === -1 ? anchorY - h : anchorY - h / 2
  return constrainCrop(source, edits, start, { x, y, w, h })
}
