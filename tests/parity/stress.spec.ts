import { expect, test, type Locator, type Page } from '@playwright/test'
import { mkdirSync, writeFileSync } from 'node:fs'
import { drag, library, libraryTile, pointIn, posts, postsView } from '../e2e/fixtures'

const output = `${process.env.HOME}/.cache/tgprep-parity/stress/${process.env.TGPREP_LABEL ?? 'rust'}`
const photoCount = 300
const postSize = 10
const viewports = [
  { name: '1366', width: 1366, height: 768 },
  { name: '1600', width: 1600, height: 1000 },
  { name: '820', width: 820, height: 1180 },
]

const photoName = (index: number) => `photo-${String(index).padStart(3, '0')}.jpg`

const prepareFolder = (page: Page) =>
  page.evaluate(async (count) => {
    const root = await navigator.storage.getDirectory()
    for await (const name of root.keys()) await root.removeEntry(name, { recursive: true })
    const folder = await root.getDirectoryHandle('photos', { create: true })
    for (let index = 0; index < count; index++) {
      const portrait = index % 7 === 3
      const canvas = new OffscreenCanvas(portrait ? 800 : 1200, portrait ? 1200 : 800)
      const context = canvas.getContext('2d')!
      context.fillStyle = `hsl(${(index * 37) % 360}, 65%, 45%)`
      context.fillRect(0, 0, canvas.width, canvas.height)
      context.fillStyle = '#fff'
      context.font = '160px sans-serif'
      context.fillText(String(index), 80, 400)
      const writable = await (await folder.getFileHandle(`photo-${String(index).padStart(3, '0')}.jpg`, { create: true })).createWritable()
      await writable.write(await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.8 }))
      await writable.close()
    }
    window.showDirectoryPicker = async () => folder
  }, photoCount)

const layoutProblems = (page: Page) =>
  page.evaluate(() => {
    const problems: string[] = []
    const root = document.documentElement
    if (root.scrollWidth > innerWidth || root.scrollHeight > innerHeight) problems.push(`page scrolls: ${root.scrollWidth}×${root.scrollHeight} in ${innerWidth}×${innerHeight}`)
    const label = (element: Element) => `${element.tagName.toLowerCase()}${element.getAttribute('aria-label') ? `[${element.getAttribute('aria-label')}]` : ''} "${(element.textContent ?? '').trim().slice(0, 40)}" .${String(element.getAttribute('class')).slice(0, 60)}`
    const clippedBy = (element: Element) => {
      const rect = element.getBoundingClientRect()
      for (let parent = element.parentElement; parent; parent = parent.parentElement) {
        const style = getComputedStyle(parent)
        if (style.overflowX === 'visible' && style.overflowY === 'visible') continue
        const bounds = parent.getBoundingClientRect()
        const scrolls = /auto|scroll/.test(style.overflowY + style.overflowX)
        if (!scrolls && (rect.left < bounds.left - 1 || rect.right > bounds.right + 1 || rect.top < bounds.top - 1 || rect.bottom > bounds.bottom + 1)) return parent
        if (scrolls) return null
      }
      return null
    }
    const scroller = (element: Element) => {
      for (let parent = element.parentElement; parent; parent = parent.parentElement) {
        const style = getComputedStyle(parent)
        if (/auto|scroll/.test(style.overflowY) && parent.scrollHeight > parent.clientHeight) {
          const bounds = parent.getBoundingClientRect()
          return bounds.bottom <= innerHeight + 1 && bounds.right <= innerWidth + 1 ? parent : null
        }
      }
      return null
    }
    for (const element of document.querySelectorAll('button, input, select, textarea, [role="status"], h1, h2, label, [title]')) {
      const rect = element.getBoundingClientRect()
      if (rect.width === 0 || rect.height === 0 || getComputedStyle(element).visibility === 'hidden') continue
      if (rect.right > innerWidth + 1 || rect.bottom > innerHeight + 1 || rect.left < -1 || rect.top < -1) {
        if (!scroller(element)) problems.push(`outside the window: ${label(element)} at ${Math.round(rect.left)},${Math.round(rect.top)} ${Math.round(rect.width)}×${Math.round(rect.height)}`)
        continue
      }
      const parent = clippedBy(element)
      if (parent && !element.closest('[data-photo]')) problems.push(`clipped by ${label(parent)}: ${label(element)}`)
    }
    for (const element of document.querySelectorAll('[data-scroll]')) {
      if (element.scrollWidth > element.clientWidth) problems.push(`scrolls sideways by ${element.scrollWidth - element.clientWidth}px: ${label(element)}`)
    }
    for (const element of document.querySelectorAll('body *')) {
      const style = getComputedStyle(element)
      if (style.overflowX !== 'hidden' || style.textOverflow === 'ellipsis' || element.children.length > 0) continue
      if (element.scrollWidth > element.clientWidth + 1 && (element.textContent ?? '').trim() !== '') problems.push(`text cut: ${label(element)}`)
    }
    return problems
  })

test.use({ launchOptions: { ignoreDefaultArgs: ['--hide-scrollbars'] } })

for (const viewport of viewports) {
  test.describe(`at ${viewport.width}×${viewport.height}`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height } })
    test.setTimeout(300_000)

    test('prepares 300 photos into 30 posts', async ({ page }) => {
      mkdirSync(output, { recursive: true })
      const report: Record<string, string[]> = {}
      const check = async (state: string) => {
        await page.waitForTimeout(400)
        await page.screenshot({ path: `${output}/${viewport.name}-${state}.png` })
        report[state] = await layoutProblems(page)
      }

      await page.goto('/')
      await check('welcome')
      await prepareFolder(page)
      await page.getByRole('button', { name: 'Open folder' }).click()
      await expect(library(page).locator('[data-photo] img').first()).toBeVisible()
      await check('opened')

      await page.getByRole('button', { name: 'In posts' }).click()
      const view = postsView(page, 0)
      const select = async (first: number, count: number) => {
        await library(page).evaluate((scroller) => (scroller.scrollTop = 0))
        await libraryTile(page, photoName(first)).click({ modifiers: ['ControlOrMeta'] })
        const last = libraryTile(page, photoName(first + count - 1))
        while ((await last.count()) === 0) await library(page).evaluate((scroller) => (scroller.scrollTop += scroller.clientHeight / 2))
        await last.scrollIntoViewIfNeeded()
        await last.click({ modifiers: ['Shift'] })
        await expect(page.getByText(`${count} selected`)).toBeVisible()
        return last
      }
      const items = (post: number) => posts(view).nth(post).locator('[data-photo]').evaluateAll((tiles) => tiles.map((tile) => tile.getAttribute('data-photo')))
      const names = (first: number, count: number) => Array.from({ length: count }, (_, index) => photoName(first + index))
      const centered = async (target: Locator, fx: number, fy: number) => {
        await target.evaluate((element) => element.scrollIntoView({ block: 'center' }))
        return pointIn(target, fx, fy)
      }
      const between = (upper: number) =>
        view.evaluate((scroller, upper) => {
          const cards = scroller.querySelectorAll('[data-post-index]')
          const gap = () => (cards[upper].getBoundingClientRect().bottom + cards[upper + 1].getBoundingClientRect().top) / 2
          const pane = scroller.getBoundingClientRect()
          scroller.scrollTop += gap() - (pane.top + pane.height / 2)
          return { x: pane.left + pane.width / 2, y: gap() }
        }, upper)

      await drag(page, libraryTile(page, photoName(0)), await pointIn(view, 0.5, 0.3))
      for (let index = 1; index < postSize; index++) {
        const target = await centered(posts(view).first().locator('[data-photo]').last(), 0.85, 0.5)
        await drag(page, libraryTile(page, photoName(index)), target, index === postSize - 1 ? () => check('drag-into-post') : undefined)
      }
      expect(await items(0)).toEqual(names(0, postSize))
      await check('first-post')

      await select(postSize, postSize)
      const below = await pointIn(posts(view).first(), 0.5, 1)
      await drag(page, libraryTile(page, photoName(2 * postSize - 1)), { x: below.x, y: below.y + 40 }, () => check('drag-selection'))
      await expect(page.getByText(`${postSize} selected`)).toHaveCount(0)
      expect(await items(1)).toEqual(names(postSize, postSize))

      await library(page).evaluate((scroller) => (scroller.scrollTop = 0))
      await drag(page, libraryTile(page, photoName(2 * postSize)), await between(0), () => check('drag-between-posts'))
      await expect(posts(view)).toHaveCount(3)
      const dropped = await select(2 * postSize + 1, postSize - 1)
      await drag(page, dropped, await centered(posts(view).nth(1).locator('[data-photo]').first(), 0.15, 0.5))
      expect(await items(1)).toEqual([...names(2 * postSize + 1, postSize - 1), photoName(2 * postSize)])

      for (let post = 3; post < photoCount / postSize; post++) {
        await select(post * postSize, postSize)
        await page.getByRole('button', { name: 'New post from selection' }).click()
        await expect(page.getByText(`${postSize} selected`)).toHaveCount(0)
      }
      await expect(library(page).locator('[data-photo]')).toHaveCount(0)
      await page.getByRole('button', { name: 'In posts' }).click()
      await expect(page.getByText(`30 posts · ${photoCount} of ${photoCount} photos used`)).toBeAttached()
      await check('placed')

      const captions = view.getByPlaceholder('Caption…')
      for (let index = 0; index < 3; index++) {
        await captions.nth(index).fill(`Day ${index + 1}: ${'a long walk along the river and dinner with friends '.repeat(index + 1)}`)
      }
      await view.evaluate((scroller) => (scroller.scrollTop = 0))
      const firstPost = posts(view).first()
      await expect(firstPost).toContainText('#1')
      await drag(page, firstPost.locator('[data-photo]').first(), await pointIn(firstPost.locator('[data-photo]').nth(3), 0.85, 0.5))
      await expect(firstPost.locator('[data-photo]').nth(3)).toHaveAttribute('data-photo', photoName(0))
      await check('captions')

      await firstPost.locator('[data-photo]').nth(1).click()
      const dialog = page.getByRole('dialog')
      await expect(dialog.locator('canvas')).toBeVisible()
      if ((await dialog.getByRole('slider', { name: 'Exposure' }).count()) === 0) await page.keyboard.press('e')
      await page.getByRole('button', { name: 'Rotate right' }).click()
      await dialog.getByRole('slider', { name: 'Exposure' }).fill('30')
      await dialog.getByRole('slider', { name: 'Straighten' }).fill('7')
      await check('viewer-edit')
      await page.keyboard.press('r')
      await check('viewer-crop')
      await page.keyboard.press('r')
      await page.keyboard.press('ArrowRight')
      await page.keyboard.press('a')
      await page.keyboard.press('Escape')
      await expect(dialog).toHaveCount(0)

      await page.getByRole('button', { name: 'Show archived photos' }).click()
      await check('archived')
      await page.getByRole('button', { name: 'Show archived photos' }).click()

      await page.getByRole('button', { name: 'Send', exact: true }).click()
      await firstPost.locator('[data-photo]').first().click()
      await expect(page.locator('[role="status"]')).toContainText('copied')
      await firstPost.getByRole('button', { name: 'Mark as sent' }).click()
      await check('send')

      writeFileSync(`${output}/${viewport.name}.json`, JSON.stringify(report, null, 2))
      expect(Object.entries(report).filter(([, problems]) => problems.length > 0)).toEqual([])
    })
  })
}
