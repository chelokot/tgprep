import { expect, test } from '@playwright/test'
import { drag, images, library, libraryTile, openFolder, pointIn, posts, postsView, savedState } from './fixtures'

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
