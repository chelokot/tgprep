import { test } from '@playwright/test'
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'

const root = `${process.env.HOME}/.cache/tgprep-parity/screens`
const output = process.env.TGPREP_COMPARE_OUT ?? `${root}/compare`

test('compares the TypeScript and Rust screenshots', async ({ page }) => {
  mkdirSync(output, { recursive: true })
  const report: string[] = []
  for (const name of readdirSync(`${root}/ts`).sort()) {
    const load = (label: string) => `data:image/png;base64,${readFileSync(`${root}/${label}/${name}`).toString('base64')}`
    const result = await page.evaluate(
      async ([ts, rust]) => {
        const image = async (source: string) => {
          const element = new Image()
          element.src = source
          await element.decode()
          return element
        }
        const [left, right] = await Promise.all([image(ts), image(rust)])
        const { width, height } = left
        const pixels = (element: HTMLImageElement) => {
          const canvas = new OffscreenCanvas(width, height)
          const context = canvas.getContext('2d')!
          context.drawImage(element, 0, 0)
          return context.getImageData(0, 0, width, height).data
        }
        const [a, b] = [pixels(left), pixels(right)]
        const diff = new OffscreenCanvas(width, height)
        const diffContext = diff.getContext('2d')!
        const diffData = diffContext.createImageData(width, height)
        let changed = 0
        for (let index = 0; index < a.length; index += 4) {
          const delta = Math.max(Math.abs(a[index] - b[index]), Math.abs(a[index + 1] - b[index + 1]), Math.abs(a[index + 2] - b[index + 2]))
          const gray = (a[index] + a[index + 1] + a[index + 2]) / 12
          if (delta > 8) changed++
          diffData.data.set(delta > 8 ? [255, 0, 64, 255] : [gray, gray, gray, 255], index)
        }
        diffContext.putImageData(diffData, 0, 0)
        const gap = 16
        const sheet = new OffscreenCanvas(width * 3 + gap * 2, height)
        const context = sheet.getContext('2d')!
        context.fillStyle = '#808080'
        context.fillRect(0, 0, sheet.width, sheet.height)
        context.drawImage(left, 0, 0)
        context.drawImage(right, width + gap, 0)
        context.drawImage(diff, (width + gap) * 2, 0)
        const blob = await sheet.convertToBlob({ type: 'image/png' })
        const bytes = new Uint8Array(await blob.arrayBuffer())
        let binary = ''
        for (const byte of bytes) binary += String.fromCharCode(byte)
        return { changed, total: width * height, png: btoa(binary) }
      },
      [load('ts'), load('rust')] as const,
    )
    writeFileSync(`${output}/${name}`, Buffer.from(result.png, 'base64'))
    report.push(`${name}\t${result.changed}\t${((result.changed / result.total) * 100).toFixed(3)}%`)
  }
  writeFileSync(`${output}/report.tsv`, report.join('\n') + '\n')
  console.log(report.join('\n'))
})
