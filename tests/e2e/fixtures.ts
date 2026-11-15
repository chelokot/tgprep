import { expect, type Locator, type Page } from '@playwright/test'

export type TestImage = { name: string; width: number; height: number; color: string }

export const images = (count: number, width = 400, height = 300): TestImage[] =>
  Array.from({ length: count }, (_, index) => ({
    name: `${String.fromCharCode(97 + index)}.jpg`,
    width,
    height,
    color: `hsl(${index * 70}, 70%, 45%)`,
  }))

export const openFolder = async (page: Page, files: TestImage[], state: unknown = null, thumbs = files.length) => {
  await page.goto('/')
  await page.evaluate(async ([files, state]) => {
    const root = await navigator.storage.getDirectory()
    for await (const name of root.keys()) await root.removeEntry(name, { recursive: true })
    const folder = await root.getDirectoryHandle('photos', { create: true })
    for (const file of files) {
      const canvas = new OffscreenCanvas(file.width, file.height)
      const context = canvas.getContext('2d')!
      context.fillStyle = file.color
      context.fillRect(0, 0, file.width, file.height)
      context.fillStyle = '#fff'
      context.font = `${Math.round(file.height / 3)}px sans-serif`
      context.fillText(file.name, file.width * 0.1, file.height * 0.6)
      const writable = await (await folder.getFileHandle(file.name, { create: true })).createWritable()
      await writable.write(await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.9 }))
      await writable.close()
    }
    if (state !== null) {
      const writable = await (await folder.getFileHandle('.tgprep.json', { create: true })).createWritable()
      await writable.write(JSON.stringify(state))
      await writable.close()
    }
    window.showDirectoryPicker = async () => folder
  }, [files, state] as const)
  await page.getByRole('button', { name: 'Open folder' }).click()
  await expect(library(page).locator('[data-photo] img')).toHaveCount(thumbs)
}

export const library = (page: Page) => page.locator('[data-drop="library"]')
export const postsView = (page: Page, index: 0 | 1) => page.locator('[data-drop="posts"]').nth(index)
export const libraryTile = (page: Page, name: string) => library(page).locator(`[data-photo="${name}"]`)
export const posts = (view: Locator) => view.locator('[data-post-index]')

export const drag = async (page: Page, from: Locator, to: { x: number; y: number }) => {
  const box = (await from.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2 + 10, { steps: 3 })
  await page.mouse.move(to.x, to.y, { steps: 12 })
  await page.waitForTimeout(80)
  await page.mouse.up()
}

export const pointIn = async (locator: Locator, fx: number, fy: number) => {
  const box = (await locator.boundingBox())!
  return { x: box.x + box.width * fx, y: box.y + box.height * fy }
}

export const savedState = (page: Page) =>
  page.evaluate(async () => {
    const root = await navigator.storage.getDirectory()
    const folder = await root.getDirectoryHandle('photos')
    const handle = await folder.getFileHandle('.tgprep.json').catch(() => null)
    if (!handle) return null
    return JSON.parse(await (await handle.getFile()).text()) as {
      posts: { category: string; items: string[]; caption: string }[]
      photos: Record<string, { category: string | null; edits: { quarter: number } }>
    }
  })
