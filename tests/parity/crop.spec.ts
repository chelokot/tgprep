import { test } from '@playwright/test'
import { readFileSync, writeFileSync } from 'node:fs'

const root = `${process.env.HOME}/.cache/tgprep-parity/screens`

test('crops a region of both screenshots side by side', async ({ page }) => {
  const [name, x, y, width, height] = (process.env.TGPREP_CROP ?? '').split(',')
  const load = (label: string) => `data:image/png;base64,${readFileSync(`${root}/${label}/${name}.png`).toString('base64')}`
  const png = await page.evaluate(
    async ([ts, rust, region]) => {
      const [left, top, w, h] = region.map(Number)
      const sheet = new OffscreenCanvas(w * 2 + 8, h)
      const context = sheet.getContext('2d')!
      context.fillStyle = '#f0f'
      context.fillRect(0, 0, sheet.width, h)
      for (const [index, source] of [ts, rust].entries()) {
        const image = new Image()
        image.src = source
        await image.decode()
        context.drawImage(image, left, top, w, h, index * (w + 8), 0, w, h)
      }
      const bytes = new Uint8Array(await (await sheet.convertToBlob()).arrayBuffer())
      let binary = ''
      for (const byte of bytes) binary += String.fromCharCode(byte)
      return btoa(binary)
    },
    [load('ts'), load('rust'), [x, y, width, height]] as const,
  )
  writeFileSync(`${root}/crop.png`, Buffer.from(png, 'base64'))
})
