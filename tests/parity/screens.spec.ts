import { expect, test, type Page } from '@playwright/test'
import { mkdirSync } from 'node:fs'
import { images, library, libraryTile, openFolder, pointIn, posts, postsView } from '../e2e/fixtures'

const label = process.env.TGPREP_LABEL ?? 'ts'
const directory = `${process.env.HOME}/.cache/tgprep-parity/screens/${label}`
mkdirSync(directory, { recursive: true })

const state = {
  app: 'tgprep',
  version: 1,
  categories: [
    { id: 'friends', name: 'Друзьям', color: '#3390ec' },
    { id: 'family', name: 'Маме', color: '#e5487d' },
  ],
  posts: [
    { id: 'p1', category: 'friends', items: ['a.jpg', 'b.jpg', 'c.jpg'], caption: 'Weekend in the hills', sent: false },
    { id: 'p2', category: 'family', items: ['d.jpg', 'e.jpg'], caption: '', sent: true },
    { id: 'p3', category: 'friends', items: ['f.jpg'], caption: 'One more', sent: false },
  ],
  photos: {
    'b.jpg': { checked: true, archived: false, category: 'family', edits: { crop: { x: 0, y: 0, w: 1, h: 1 }, aspect: 'free', angle: 0, quarter: 1, flip: false, adjust: { exposure: 20, brightness: 0, contrast: 10, highlights: 0, shadows: 0, temperature: 0, tint: 0, saturation: 0, vibrance: 0, sharpness: 0, vignette: 0, fade: 0, grain: 0 } } },
    'g.jpg': { checked: true, archived: false, category: null, edits: { crop: { x: 0, y: 0, w: 1, h: 1 }, aspect: 'free', angle: 0, quarter: 0, flip: false, adjust: { exposure: 0, brightness: 0, contrast: 0, highlights: 0, shadows: 0, temperature: 0, tint: 0, saturation: 0, vibrance: 0, sharpness: 0, vignette: 0, fade: 0, grain: 0 } } },
  },
}

const shot = async (page: Page, name: string) => {
  await page.evaluate(() => document.fonts.ready)
  await page.mouse.move(1, 1)
  await page.waitForTimeout(300)
  await page.screenshot({ path: `${directory}/${name}-${page.viewportSize()!.width}.png`, animations: 'disabled' })
}

for (const viewport of [{ width: 1600, height: 1000 }, { width: 820, height: 1180 }]) {
  test.describe(`${viewport.width}`, () => {
    test.use({ viewport })

    test('screens', async ({ page }) => {
      await page.goto('/')
      await shot(page, 'welcome')

      await openFolder(page, images(14, 1200, 800), state, viewport.width === 1600 ? 14 : 7)
      await expect(postsView(page, 0).locator('[data-photo] img')).toHaveCount(6)
      await shot(page, 'workspace')

      const target = await pointIn(posts(postsView(page, 0)).first().locator('[data-photo="b.jpg"]'), 0.8, 0.5)
      const box = (await libraryTile(page, 'g.jpg').boundingBox())!
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.down()
      await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2 + 10, { steps: 3 })
      await page.mouse.move(target.x, target.y, { steps: 12 })
      await page.waitForTimeout(200)
      await page.screenshot({ path: `${directory}/drag-${viewport.width}.png`, animations: 'disabled' })
      await page.keyboard.press('Escape')
      await page.mouse.up()

      await libraryTile(page, 'b.jpg').click()
      await page.keyboard.press('e')
      await expect(page.locator('[role="dialog"] canvas')).toBeVisible()
      await page.waitForTimeout(600)
      await shot(page, 'viewer')
      await page.keyboard.press('Escape')

      await page.getByRole('button', { name: 'Send', exact: true }).click()
      await expect(library(page)).toBeVisible()
      await shot(page, 'send')
    })
  })
}
