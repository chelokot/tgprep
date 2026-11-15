import { expect, test } from '@playwright/test'
import { cachedThumbs, drag, images, library, libraryTile, openFolder, pointIn, posts, postsView, savedState } from './fixtures'

test('composes posts by dragging, undoes and persists them', async ({ page }) => {
  await openFolder(page, images(4))
  const view = postsView(page, 0)

  await drag(page, libraryTile(page, 'a.jpg'), await pointIn(view, 0.5, 0.3))
  await expect(posts(view)).toHaveCount(1)

  const firstTile = posts(view).first().locator('[data-photo="a.jpg"]')
  await drag(page, libraryTile(page, 'b.jpg'), await pointIn(firstTile, 0.15, 0.5))
  await expect(posts(view).first().locator('[data-photo]').nth(0)).toHaveAttribute('data-photo', 'b.jpg')
  await expect(posts(view).first().locator('[data-photo]').nth(1)).toHaveAttribute('data-photo', 'a.jpg')

  const below = await pointIn(posts(view).first(), 0.5, 1)
  await drag(page, libraryTile(page, 'c.jpg'), { x: below.x, y: below.y + 40 })
  await expect(posts(view)).toHaveCount(2)

  await page.keyboard.press('Control+z')
  await expect(posts(view)).toHaveCount(1)

  await expect.poll(async () => (await savedState(page))?.posts.map((post) => post.items)).toEqual([['b.jpg', 'a.jpg']])
})

test('allows a photo once per category and moves it within a category', async ({ page }) => {
  await openFolder(page, images(3))
  const friends = postsView(page, 0)
  const family = postsView(page, 1)
  await page.getByRole('button', { name: 'Маме' }).nth(1).click()

  await drag(page, libraryTile(page, 'a.jpg'), await pointIn(friends, 0.5, 0.3))
  await drag(page, libraryTile(page, 'a.jpg'), await pointIn(family, 0.5, 0.3))
  await expect(libraryTile(page, 'a.jpg').getByTitle('Show this post')).toHaveCount(2)

  await drag(page, libraryTile(page, 'b.jpg'), await pointIn(posts(family).first().locator('[data-photo="a.jpg"]'), 0.85, 0.5))
  const familyPost = await pointIn(posts(family).first(), 0.5, 1)
  await drag(page, libraryTile(page, 'a.jpg'), { x: familyPost.x, y: familyPost.y + 40 })

  await expect.poll(async () => (await savedState(page))?.posts.map((post) => [post.category, post.items])).toEqual([
    ['friends', ['a.jpg']],
    ['family', ['b.jpg']],
    ['family', ['a.jpg']],
  ])
})

test('categorizes photos in the viewer and filters the library by category', async ({ page }) => {
  await openFolder(page, images(3))
  const pane = page.locator('section', { has: library(page) })
  const shown = () => library(page).locator('[data-photo]').evaluateAll((tiles) => tiles.map((tile) => tile.getAttribute('data-photo')))

  await libraryTile(page, 'b.jpg').click()
  const dialog = page.getByRole('dialog')
  await dialog.getByRole('button', { name: 'Маме' }).click()
  await expect(dialog.getByRole('button', { name: 'Маме' })).toHaveAttribute('aria-pressed', 'true')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('2')
  await page.keyboard.press('Escape')

  await pane.getByRole('button', { name: 'Друзьям' }).click()
  await expect.poll(shown).toEqual(['a.jpg'])
  await pane.getByRole('button', { name: 'Маме' }).click()
  await expect.poll(shown).toEqual(['b.jpg', 'c.jpg'])

  await libraryTile(page, 'c.jpg').click()
  await page.keyboard.press('1')
  await page.keyboard.press('Escape')
  await expect.poll(shown).toEqual(['b.jpg'])
  await expect.poll(async () => Object.entries((await savedState(page))?.photos ?? {}).map(([id, meta]) => [id, meta.category])).toEqual([['b.jpg', 'family']])
})

test('viewer navigates, archives to the next photo and highlights the last one on close', async ({ page }) => {
  await openFolder(page, images(4))
  await libraryTile(page, 'a.jpg').click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('1 / 4')
  await page.keyboard.press('ArrowRight')
  await expect(dialog).toContainText('b.jpg')
  await page.keyboard.press('a')
  await expect(dialog).toContainText('c.jpg')
  await expect(dialog).toContainText('2 / 3')
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)
  await expect(libraryTile(page, 'b.jpg')).toHaveCount(0)
  await expect(libraryTile(page, 'c.jpg')).toHaveClass(/animate-glow/)
})

test('crop handles stay clear of the photo navigation buttons', async ({ page }) => {
  await openFolder(page, images(3, 1500, 1000))
  await libraryTile(page, 'b.jpg').click()
  const dialog = page.getByRole('dialog')
  if ((await dialog.getByRole('slider', { name: 'Straighten' }).count()) === 0) await page.keyboard.press('e')
  await dialog.getByRole('button', { name: 'Crop and rotate (R)' }).click()
  await expect(dialog.locator('canvas')).toBeVisible()
  for (const viewport of [
    { width: 820, height: 1180 },
    { width: 1366, height: 768 },
    { width: 1600, height: 1000 },
  ]) {
    await page.setViewportSize(viewport)
    const overlaps = await dialog.evaluate(async (dialog) => {
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      const handles = [...dialog.querySelectorAll<HTMLElement>('[style*="resize"]')].map((handle) => handle.getBoundingClientRect())
      const buttons = ['Previous', 'Next'].map((label) => dialog.querySelector(`[aria-label^="${label}"]`)!.getBoundingClientRect())
      const apart = (a: DOMRect, b: DOMRect) => a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top
      return { handles: handles.length, overlapping: buttons.flatMap((button) => handles.filter((handle) => !apart(button, handle))).length }
    })
    expect(overlaps, `${viewport.width}×${viewport.height}`).toEqual({ handles: 8, overlapping: 0 })
  }
})

test('moving to the next photo with a slider focused leaves the next photo unedited', async ({ page }) => {
  await openFolder(page, images(3))
  await libraryTile(page, 'a.jpg').click()
  const dialog = page.getByRole('dialog')
  if ((await dialog.getByRole('slider', { name: 'Straighten' }).count()) === 0) await page.keyboard.press('e')
  await dialog.getByRole('slider', { name: 'Straighten' }).fill('7')
  await page.keyboard.press('ArrowRight')
  await expect(dialog).toContainText('b.jpg')
  await expect(dialog.getByRole('slider', { name: 'Straighten' })).toHaveValue('0')
  await expect.poll(async () => Object.keys((await savedState(page))?.photos ?? {})).toEqual(['a.jpg'])
})

test('edits are stored as settings and shown in thumbnails', async ({ page }) => {
  await openFolder(page, images(1, 400, 200))
  const view = postsView(page, 0)
  await drag(page, libraryTile(page, 'a.jpg'), await pointIn(view, 0.5, 0.3))
  const tile = posts(view).first().locator('[data-photo="a.jpg"]')
  const wide = (await tile.boundingBox())!
  expect(wide.width).toBeGreaterThan(wide.height * 1.8)

  await tile.click()
  await page.keyboard.press('e')
  await page.getByRole('button', { name: 'Rotate right' }).click()
  await page.keyboard.press('Escape')
  await expect.poll(async () => (await tile.boundingBox())!.height > (await tile.boundingBox())!.width * 1.8).toBe(true)
  await expect(tile.getByTitle('Edited')).toBeVisible()
  await expect.poll(async () => (await savedState(page))?.photos['a.jpg']?.edits.quarter).toBe(1)

  await tile.click()
  await expect(page.getByRole('button', { name: 'Reset all edits' })).toBeVisible()
  await page.getByRole('button', { name: 'Reset all edits' }).click()
  await page.keyboard.press('Escape')
  await expect.poll(async () => (await tile.boundingBox())!.width > (await tile.boundingBox())!.height * 1.8).toBe(true)
})

test('keeps showing a photo while its edited thumbnail renders', async ({ page }) => {
  await openFolder(page, images(1))
  const image = libraryTile(page, 'a.jpg').locator('img')
  const original = (await image.getAttribute('src'))!
  await libraryTile(page, 'a.jpg').click()
  await page.keyboard.press('e')
  await page.getByRole('button', { name: 'Rotate right' }).click()
  expect(await page.evaluate(() => [...document.querySelectorAll('[data-photo]')].map((tile) => tile.querySelector('img')?.getAttribute('src') ?? 'placeholder'))).not.toContain('placeholder')
  await expect(image).not.toHaveAttribute('src', original)
})

test('send mode copies the rendered photo and the caption', async ({ page }) => {
  await openFolder(page, images(1, 400, 200))
  const view = postsView(page, 0)
  await drag(page, libraryTile(page, 'a.jpg'), await pointIn(view, 0.5, 0.3))
  await posts(view).first().locator('[data-photo="a.jpg"]').click()
  await page.keyboard.press('e')
  await page.getByRole('button', { name: 'Rotate right' }).click()
  await page.keyboard.press('Escape')
  await posts(view).first().getByPlaceholder('Caption…').fill('Hello **Telegram**')

  await page.getByRole('button', { name: 'Send' }).click()
  await posts(view).first().locator('[data-photo="a.jpg"]').click()
  await expect(page.getByRole('status')).toHaveText('Photo 1 copied')
  const size = await page.evaluate(async () => {
    const [item] = await navigator.clipboard.read()
    const bitmap = await createImageBitmap(await item.getType('image/png'))
    return [bitmap.width, bitmap.height]
  })
  expect(size).toEqual([200, 400])
  await expect(posts(view).first().locator('[data-photo="a.jpg"]').getByRole('button', { name: 'Unmark' })).toBeVisible()

  await posts(view).first().getByTitle('Copy caption').click()
  await expect(page.getByRole('status')).toHaveText('Caption copied')
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('Hello **Telegram**')
})

test.describe('on a high density screen', () => {
  test.use({ deviceScaleFactor: 2 })

  test('thumbnails have enough pixels for the largest tiles', async ({ page }) => {
    await openFolder(page, [
      { name: 'a.jpg', width: 3000, height: 2000, color: 'hsl(20, 70%, 45%)' },
      { name: 'b.jpg', width: 2000, height: 3000, color: 'hsl(200, 70%, 45%)' },
    ])
    await page.getByRole('slider', { name: 'Thumbnail size' }).fill('24')
    await page.getByRole('slider', { name: 'Photo height' }).first().fill('20')
    const view = postsView(page, 0)
    await drag(page, libraryTile(page, 'a.jpg'), await pointIn(view, 0.5, 0.3))
    await expect(posts(view).first().locator('[data-photo="a.jpg"] img')).toBeVisible()

    const sharpness = () =>
      page.locator('[data-photo] img').evaluateAll((images) =>
        images.map((image) => {
          const element = image as HTMLImageElement
          return Math.min(element.naturalWidth / (element.clientWidth * devicePixelRatio), element.naturalHeight / (element.clientHeight * devicePixelRatio))
        }),
      )
    await expect.poll(async () => (await sharpness()).length).toBe(4)
    for (const ratio of await sharpness()) expect(ratio).toBeGreaterThanOrEqual(1)
  })
})

test('prepares thumbnails of the photos below the screen before scrolling', async ({ page }) => {
  const files = images(80)
  await openFolder(page, files, null, null)
  await expect(library(page).locator('[data-photo] img').first()).toBeVisible()
  await expect.poll(() => cachedThumbs(page), { timeout: 20_000 }).toBe(files.length)
  expect(await library(page).locator('[data-photo]').count()).toBeLessThan(files.length)
})

test('waits with thumbnails below the screen while the user is busy', async ({ page }) => {
  const files = images(80)
  await openFolder(page, files, null, null)
  await expect(library(page).locator('[data-photo] img').first()).toBeVisible()
  const busy = async (milliseconds: number) => {
    for (let elapsed = 0; elapsed < milliseconds; elapsed += 150) {
      await page.keyboard.press('Shift')
      await page.waitForTimeout(150)
    }
  }
  await busy(1000)
  const settled = await cachedThumbs(page)
  await busy(1500)
  expect(await cachedThumbs(page)).toBe(settled)
  expect(settled).toBeLessThan(files.length)
  await expect.poll(() => cachedThumbs(page), { timeout: 20_000 }).toBe(files.length)
})

test('waits with thumbnails below the screen while the viewer is open', async ({ page }) => {
  const files = images(80)
  await openFolder(page, files, null, null)
  await library(page).locator('[data-photo]').first().click()
  await expect(page.getByRole('dialog')).toBeVisible()
  await page.waitForTimeout(1000)
  const settled = await cachedThumbs(page)
  await page.waitForTimeout(1000)
  expect(await cachedThumbs(page)).toBe(settled)
  expect(settled).toBeLessThan(files.length)
  await page.keyboard.press('Escape')
  await expect.poll(() => cachedThumbs(page), { timeout: 20_000 }).toBe(files.length)
})

test('keeps making thumbnails after photos become unreadable', async ({ page }) => {
  await page.addInitScript(() => {
    const getFile = FileSystemFileHandle.prototype.getFile
    const listed = new Set<string>()
    FileSystemFileHandle.prototype.getFile = function (this: FileSystemFileHandle) {
      if (/^[a-e]\.jpg$/.test(this.name) && listed.has(this.name)) return Promise.reject(new DOMException('gone', 'NotFoundError'))
      listed.add(this.name)
      return getFile.call(this)
    }
  })
  await openFolder(page, images(10), null, 5)
  await expect(library(page).locator('[data-photo]:not(:has(img))')).toHaveCount(5)
  await expect(library(page).locator('[data-photo] .animate-pulse')).toHaveCount(0)
})

test.describe('on a tablet', () => {
  test.use({ viewport: { width: 820, height: 1180 } })
  test('fits every post control inside the post', async ({ page }) => {
    const state = {
      app: 'tgprep',
      version: 1,
      categories: [{ id: 'friends', name: 'Друзьям', color: '#3390ec' }],
      posts: [{ id: 'p1', category: 'friends', items: ['a.jpg', 'b.jpg'], caption: '', sent: false }],
      photos: {},
    }
    await openFolder(page, images(2), state)
    const view = postsView(page, 0)
    const post = (await posts(view).first().boundingBox())!
    for (const name of ['Mark as sent', 'Save as files', 'Delete post']) {
      const button = (await posts(view).first().getByRole('button', { name }).boundingBox())!
      expect(button.x + button.width).toBeLessThanOrEqual(post.x + post.width)
    }
    expect(await view.evaluate((scroller) => scroller.scrollWidth - scroller.clientWidth)).toBe(0)
  })


  test('pressing and dragging a photo over text selects none of it', async ({ page }) => {
    const state = {
      app: 'tgprep',
      version: 1,
      categories: [{ id: 'friends', name: 'Друзьям', color: '#3390ec' }],
      posts: [{ id: 'p1', category: 'friends', items: ['a.jpg', 'b.jpg', 'c.jpg'], caption: 'Weekend in the hills', sent: false }],
      photos: {},
    }
    await openFolder(page, images(8, 1200, 800), state, 7)
    const target = await pointIn(posts(postsView(page, 0)).first().locator('[data-photo="b.jpg"]'), 0.8, 0.5)
    const box = (await libraryTile(page, 'a.jpg').boundingBox())!
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    await page.mouse.down()
    await page.mouse.move(box.x + box.width / 2 + 3, box.y + box.height / 2 + 3)
    await page.mouse.move(target.x, target.y, { steps: 12 })
    await page.keyboard.press('Escape')
    await page.mouse.up()
    expect(await page.evaluate(() => getSelection()!.toString())).toBe('')
  })
})
