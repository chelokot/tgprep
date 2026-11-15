export type ViewRequest = { key: string; file: File; maxSide: number }
export type ViewResponse = { key: string; bitmap: ImageBitmap }

const scope = self as unknown as Worker

scope.onmessage = async ({ data: { key, file, maxSide } }: MessageEvent<ViewRequest>) => {
  const original = await createImageBitmap(file)
  const scale = Math.min(1, maxSide / Math.max(original.width, original.height))
  const bitmap =
    scale === 1
      ? original
      : await createImageBitmap(original, { resizeWidth: Math.round(original.width * scale), resizeHeight: Math.round(original.height * scale), resizeQuality: 'high' })
  if (bitmap !== original) original.close()
  scope.postMessage({ key, bitmap } satisfies ViewResponse, [bitmap])
}
