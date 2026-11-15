export type ThumbRequest = { key: string; file: File; longSide: number }
export type ThumbResponse = { key: string; result: { blob: Blob; w: number; h: number } | null }

const scope = self as unknown as Worker

scope.onmessage = async ({ data: { key, file, longSide } }: MessageEvent<ThumbRequest>) => {
  const respond = (result: ThumbResponse['result']) => scope.postMessage({ key, result } satisfies ThumbResponse)
  const original = await createImageBitmap(file).catch(() => null)
  if (!original) return respond(null)
  const scale = Math.min(1, longSide / Math.max(original.width, original.height))
  const w = Math.max(1, Math.round(original.width * scale))
  const h = Math.max(1, Math.round(original.height * scale))
  const small = await createImageBitmap(original, { resizeWidth: w, resizeHeight: h, resizeQuality: 'high' })
  const canvas = new OffscreenCanvas(w, h)
  canvas.getContext('2d')!.drawImage(small, 0, 0)
  respond({ blob: await canvas.convertToBlob({ type: 'image/webp', quality: 0.85 }), w: original.width, h: original.height })
  original.close()
  small.close()
}
