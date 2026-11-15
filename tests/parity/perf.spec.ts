import { chromium, devices, expect, test, type BrowserContext, type Page } from '@playwright/test'
import { mkdirSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { join, relative } from 'node:path'

type Build = { name: string; url: string; dir: string }
type Samples = Record<string, Record<string, number[]>>
type Probe = 'thumbs' | 'indicator' | 'viewer' | 'drawn' | 'copied' | 'ready'

const builds: Build[] = (process.env.TGPREP_BUILDS ?? '').split(';').filter(Boolean).map((entry) => {
  const [name, url, dir] = entry.split('=')
  return { name, url, dir }
})
const runs = Number(process.env.TGPREP_RUNS ?? 5)
const cpu = Number(process.env.TGPREP_CPU ?? 1)
const output = `${process.env.HOME}/.cache/tgprep-parity/perf`
const profiles = `${process.env.HOME}/.cache/tgprep-perf-profiles`
const photoCount = 300
const postCount = 30
const photosPerPost = 4
const viewport = { width: 1600, height: 1000 }
const gpu = ['--enable-gpu', '--use-angle=vulkan']
const fastNetwork = { offline: false, latency: 60, downloadThroughput: 9_000_000 / 8, uploadThroughput: 1_500_000 / 8 }

const name = (index: number) => `photo-${String(index).padStart(3, '0')}.jpg`
const state = {
  app: 'tgprep',
  version: 1,
  categories: [
    { id: 'friends', name: 'Друзьям', color: '#3390ec' },
    { id: 'family', name: 'Маме', color: '#e5487d' },
  ],
  posts: Array.from({ length: postCount }, (_, post) => ({
    id: `p${post}`,
    category: 'friends',
    items: Array.from({ length: photosPerPost }, (_, item) => name(post * photosPerPost + item)),
    caption: `Post ${post}`,
    sent: false,
  })),
  photos: {},
}

const instrument = () => {
  const perf = {
    lastDraw: 0,
    lastMove: 0,
    lastUp: 0,
    armedAt: 0,
    result: null as Promise<number> | null,
    snapshot: '',
    minimum: 0,
  }
  for (const method of ['drawArrays', 'drawElements'] as const) {
    const original = WebGL2RenderingContext.prototype[method] as (...args: unknown[]) => void
    WebGL2RenderingContext.prototype[method] = function (this: WebGL2RenderingContext, ...args: unknown[]) {
      perf.lastDraw = performance.now()
      return original.apply(this, args)
    } as never
  }
  addEventListener('pointermove', (event) => (perf.lastMove = event.timeStamp), { capture: true })
  addEventListener('pointerup', (event) => (perf.lastUp = event.timeStamp), { capture: true })
  window.showDirectoryPicker = async () => (await navigator.storage.getDirectory()).getDirectoryHandle('photos')

  const library = () => document.querySelector<HTMLElement>('[data-drop="library"]')
  const visibleTiles = () => {
    const scroller = library()
    if (!scroller) return []
    const bounds = scroller.getBoundingClientRect()
    return [...scroller.querySelectorAll<HTMLElement>('[data-photo]')].filter((tile) => {
      const rect = tile.getBoundingClientRect()
      return rect.bottom > bounds.top && rect.top < Math.min(bounds.bottom, innerHeight) && rect.width > 0
    })
  }
  const loaded = (tile: HTMLElement) => {
    const image = tile.querySelector('img')
    return image !== null && image.complete && image.naturalWidth > 0
  }
  const indicator = () =>
    [...document.querySelectorAll<HTMLElement>('.fixed.inset-0.z-\\[60\\] .ring-accent\\/70, .fixed.inset-0.z-\\[60\\] .bg-accent')]
      .map((element) => element.style.cssText)
      .join('|')
  const status = () => [...document.querySelectorAll('[role="status"]')].map((element) => element.textContent).join('|')
  const predicates: Record<string, () => boolean> = {
    ready: () => [...document.querySelectorAll('button')].some((button) => button.textContent?.includes('Open folder')),
    thumbs: () => {
      const tiles = visibleTiles()
      return tiles.length >= perf.minimum && tiles.length > 0 && tiles.every(loaded)
    },
    indicator: () => indicator() !== perf.snapshot && indicator() !== '',
    viewer: () => {
      const canvas = document.querySelector<HTMLCanvasElement>('[role="dialog"] canvas')
      return canvas !== null && getComputedStyle(canvas).visibility === 'visible' && perf.lastDraw > perf.armedAt
    },
    drawn: () => perf.lastDraw > perf.armedAt,
    copied: () => status() !== perf.snapshot && status().includes('copied'),
  }
  const frameAfter = (predicate: () => boolean) =>
    new Promise<number>((resolve, reject) => {
      const probe = document.createElement('div')
      probe.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;visibility:hidden;pointer-events:none'
      const deadline = performance.now() + 60_000
      let done = false
      let wide = false
      const finish = (settle: () => void) => {
        done = true
        observer.disconnect()
        probe.remove()
        settle()
      }
      const observer = new ResizeObserver(() => {
        if (done || !predicate()) return
        done = true
        scheduler.postTask(() => finish(() => resolve(performance.now())), { priority: 'user-blocking' })
      })
      const tick = () => {
        if (done) return
        if (!probe.isConnected) {
          document.documentElement.append(probe)
          observer.observe(probe)
        }
        if (performance.now() > deadline) return finish(() => reject(new Error('no frame within 60 s')))
        wide = !wide
        probe.style.width = wide ? '2px' : '1px'
        requestAnimationFrame(tick)
      }
      requestAnimationFrame(tick)
    })
  const ready = frameAfter(predicates.ready)
  Object.assign(window, {
    perf,
    arm: (probe: string, minimum = 0) => {
      perf.armedAt = performance.now()
      perf.minimum = minimum
      perf.snapshot = probe === 'indicator' ? indicator() : status()
      perf.result = frameAfter(predicates[probe])
    },
    collect: async (start: 'armed' | 'move' | 'up') => {
      const end = await perf.result!
      return end - { armed: perf.armedAt, move: perf.lastMove, up: perf.lastUp }[start]
    },
    visibleCount: () => visibleTiles().length,
    ready: () => ready,
  })
}

declare global {
  interface Window {
    perf: { armedAt: number }
    arm: (probe: Probe, minimum?: number) => void
    collect: (start: 'armed' | 'move' | 'up') => Promise<number>
    visibleCount: () => number
    ready: () => Promise<number>
  }
  const scheduler: { postTask: (callback: () => void, options: { priority: string }) => Promise<void> }
}

const record = (samples: Samples, metric: string, build: string, value: number) => {
  console.log(`${build} ${metric}: ${Math.round(value)}`)
  samples[metric] ??= {}
  ;(samples[metric][build] ??= []).push(value)
}

const report = (file: string, samples: Samples) => {
  mkdirSync(output, { recursive: true })
  writeFileSync(join(output, `${file}.json`), JSON.stringify(samples, null, 2))
  const summary = (values: number[]) => {
    const sorted = [...values].sort((a, b) => a - b)
    return `${Math.round(sorted[0])} / ${Math.round(sorted[Math.floor(sorted.length / 2)])}`
  }
  const lines = [`| ${file} (min / median ms) | ${builds.map((build) => build.name).join(' | ')} |`, `|---|${builds.map(() => '---|').join('')}`]
  for (const [metric, byBuild] of Object.entries(samples)) {
    lines.push(`| ${metric} | ${builds.map((build) => (byBuild[build.name] ? summary(byBuild[build.name]) : '')).join(' | ')} |`)
  }
  writeFileSync(join(output, `${file}.md`), `${lines.join('\n')}\n`)
  console.log(lines.join('\n'))
}

const throttle = async (context: BrowserContext, page: Page, network: boolean) => {
  const session = await context.newCDPSession(page)
  await session.send('Emulation.setCPUThrottlingRate', { rate: cpu })
  if (network) await session.send('Network.emulateNetworkConditions', fastNetwork)
}

const prepareFolder = (page: Page) =>
  page.evaluate(
    async ([count, state]) => {
      const root = await navigator.storage.getDirectory()
      const folder = await root.getDirectoryHandle('photos', { create: true })
      const present = new Set<string>()
      for await (const key of folder.keys()) present.add(key)
      const bases: Blob[] = []
      for (let index = 0; index < count; index++) {
        const file = `photo-${String(index).padStart(3, '0')}.jpg`
        if (present.has(file)) continue
        const variant = index % 6
        if (!bases[variant]) {
          const canvas = new OffscreenCanvas(6000, 4000)
          const context = canvas.getContext('2d')!
          const gradient = context.createLinearGradient(0, 0, 6000, 4000)
          gradient.addColorStop(0, `hsl(${variant * 60}, 70%, 35%)`)
          gradient.addColorStop(1, `hsl(${variant * 60 + 40}, 60%, 70%)`)
          context.fillStyle = gradient
          context.fillRect(0, 0, 6000, 4000)
          for (let circle = 0; circle < 40; circle++) {
            context.fillStyle = `hsla(${(variant * 60 + circle * 23) % 360}, 80%, 55%, 0.5)`
            context.beginPath()
            context.arc((circle * 977) % 6000, (circle * 613) % 4000, 150 + ((circle * 37) % 400), 0, Math.PI * 2)
            context.fill()
          }
          context.fillStyle = '#fff'
          context.font = '900px sans-serif'
          context.fillText(String(variant), 2400, 2400)
          bases[variant] = await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.8 })
        }
        const writable = await (await folder.getFileHandle(file, { create: true })).createWritable()
        await writable.write(bases[variant])
        await writable.close()
      }
      const writable = await (await folder.getFileHandle('.tgprep.json', { create: true })).createWritable()
      await writable.write(JSON.stringify(state))
      await writable.close()
      await new Promise((resolve) => {
        const request = indexedDB.deleteDatabase('tgprep-thumbs')
        request.onsuccess = request.onerror = request.onblocked = resolve
      })
    },
    [photoCount, state] as const,
  )

const openFolder = async (page: Page) => {
  await page.goto('/')
  await page.evaluate(() => window.ready())
  await page.evaluate(() => {
    window.arm('thumbs', 1)
    ;[...document.querySelectorAll('button')].find((button) => button.textContent?.includes('Open folder'))!.click()
  })
  return page.evaluate(() => window.collect('armed'))
}

const tileCenter = async (page: Page, selector: string) => {
  const element = page.locator(selector).first()
  await element.scrollIntoViewIfNeeded()
  const box = (await element.boundingBox())!
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
}

const interactions = async (page: Page, build: string, samples: Samples) => {
  await page.goto('/favicon.svg')
  await prepareFolder(page)
  record(samples, 'open folder → visible thumbnails (not cached)', build, await openFolder(page))
  record(samples, 'open folder → visible thumbnails (cached)', build, await openFolder(page))
  await page.waitForTimeout(8000)

  const library = page.locator('[data-drop="library"]')
  for (let step = 0; step < 5; step++) {
    const visible = await page.evaluate(() => window.visibleCount())
    await page.evaluate((minimum) => {
      window.arm('thumbs', Math.floor(minimum * 0.8))
      const scroller = document.querySelector<HTMLElement>('[data-drop="library"]')!
      scroller.scrollTop += scroller.clientHeight
    }, visible)
    record(samples, 'scroll one screen → thumbnails', build, await page.evaluate(() => window.collect('armed')))
    await page.waitForTimeout(150)
  }
  await library.evaluate((scroller) => (scroller.scrollTop = scroller.scrollHeight))
  await page.waitForTimeout(8000)

  const posts = page.locator('[data-drop="posts"]').first()
  const firstPost = posts.locator('[data-post-index]').first()
  const before = await firstPost.locator('[data-photo]').nth(0).boundingBox()
  const after = await firstPost.locator('[data-photo]').nth(2).boundingBox()
  const source = await tileCenter(page, '[data-drop="library"] [data-photo] >> nth=-1')
  await page.mouse.move(source.x, source.y)
  await page.mouse.down()
  await page.mouse.move(source.x + 10, source.y + 10, { steps: 3 })
  await page.mouse.move(before!.x + 4, before!.y + before!.height / 2, { steps: 4 })
  await page.waitForTimeout(200)
  for (let move = 0; move < 12; move++) {
    const target = move % 2 === 0 ? after! : before!
    await page.evaluate(() => window.arm('indicator'))
    await page.mouse.move(target.x + (move % 2 === 0 ? target.width - 4 : 4), target.y + target.height / 2)
    record(samples, 'drag pointer move → drop highlight', build, await page.evaluate(() => window.collect('move')))
    await page.waitForTimeout(60)
  }
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await library.evaluate((scroller) => (scroller.scrollTop = 0))
  await page.waitForTimeout(500)

  for (const index of [0, 5, 10, 15, 20]) {
    const tile = await tileCenter(page, `[data-drop="library"] [data-photo] >> nth=${index}`)
    await page.evaluate(() => window.arm('viewer'))
    await page.mouse.click(tile.x, tile.y)
    record(samples, 'viewer open → full-resolution frame', build, await page.evaluate(() => window.collect('up')))
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog')).toHaveCount(0)
  }

  const tile = await tileCenter(page, '[data-drop="library"] [data-photo] >> nth=25')
  await page.mouse.click(tile.x, tile.y)
  await expect(page.locator('[role="dialog"] canvas')).toBeVisible()
  if ((await page.locator('[role="dialog"] input[type="range"]').count()) === 0) await page.keyboard.press('e')
  await page.waitForTimeout(1500)
  for (let step = 1; step <= 12; step++) {
    await page.evaluate((value) => {
      const input = document.querySelector<HTMLInputElement>('[role="dialog"] input[type="range"]')!
      window.arm('drawn')
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, String(value))
      input.dispatchEvent(new Event('input', { bubbles: true }))
    }, (step % 2 === 0 ? 1 : -1) * step * 4)
    record(samples, 'edit slider → frame', build, await page.evaluate(() => window.collect('armed')))
    await page.waitForTimeout(60)
  }
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)

  await page.getByRole('button', { name: 'Send', exact: true }).click()
  await page.waitForTimeout(500)
  for (let item = 0; item < 3; item++) {
    await expect(page.locator('[role="status"]')).toHaveCount(0, { timeout: 15_000 })
    const photo = await tileCenter(page, `[data-drop="posts"] >> nth=0 >> [data-post-index] >> nth=${item} >> [data-photo] >> nth=0`)
    await page.evaluate(() => window.arm('copied'))
    await page.mouse.click(photo.x, photo.y)
    record(samples, 'send mode click → photo copied', build, await page.evaluate(() => window.collect('up')))
  }
}

const launch = (build: Build) =>
  chromium.launchPersistentContext(join(profiles, build.name), {
    ...devices['Desktop Chrome'],
    viewport,
    baseURL: build.url,
    locale: 'en-US',
    permissions: ['clipboard-read', 'clipboard-write'],
    args: gpu,
  })

test.use({ launchOptions: { args: gpu } })
test.describe.configure({ mode: 'serial' })
test.setTimeout(3_600_000)

test('bundle', async ({ request }) => {
  const sizes: Samples = {}
  for (const build of builds) {
    const files = (directory: string): string[] =>
      readdirSync(directory).flatMap((entry) => (statSync(join(directory, entry)).isDirectory() ? files(join(directory, entry)) : [join(directory, entry)]))
    for (const file of files(build.dir)) {
      const path = `/${relative(build.dir, file)}`
      const kind = path.endsWith('.wasm') ? 'wasm' : path.endsWith('.js') ? 'js' : path.endsWith('.css') ? 'css' : 'other'
      for (const encoding of ['identity', 'gzip', 'br']) {
        const response = await request.get(`${build.url}${path}`, { headers: { 'accept-encoding': encoding } })
        const bytes = Number(response.headers()['content-length'] ?? (await response.body()).length)
        record(sizes, `${kind} ${encoding}`, build.name, bytes)
        record(sizes, `total ${encoding}`, build.name, bytes)
      }
    }
  }
  const lines = [`| bytes | ${builds.map((build) => build.name).join(' | ')} |`, `|---|${builds.map(() => '---:|').join('')}`]
  for (const [metric, byBuild] of Object.entries(sizes).sort(([a], [b]) => a.localeCompare(b))) {
    lines.push(`| ${metric} | ${builds.map((build) => (byBuild[build.name] ?? []).reduce((sum, value) => sum + value, 0).toLocaleString('en-US')).join(' | ')} |`)
  }
  mkdirSync(output, { recursive: true })
  writeFileSync(join(output, 'bundle.md'), `${lines.join('\n')}\n`)
  console.log(lines.join('\n'))
})

test('load', async ({ browser }) => {
  const samples: Samples = {}
  for (let run = 0; run < runs; run++) {
    for (const build of builds) {
      for (const network of [false, true]) {
        const context = await browser.newContext({ ...devices['Desktop Chrome'], viewport, baseURL: build.url })
        const page = await context.newPage()
        await page.addInitScript(instrument)
        await throttle(context, page, network)
        const suffix = network ? ', 9 Mbit/s 60 ms' : ''
        for (const visit of ['first visit', 'second visit', 'third visit']) {
          await (visit === 'first visit' ? page.goto('/') : page.reload())
          record(samples, `${visit} → welcome painted${suffix}`, build.name, await page.evaluate(() => window.ready()))
          await page.waitForTimeout(500)
        }
        await context.close()
      }
    }
  }
  report(`load-cpu${cpu}`, samples)
})

test('interactions', async () => {
  const samples: Samples = {}
  const contexts = await Promise.all(builds.map(launch))
  const pages = await Promise.all(
    contexts.map(async (context) => {
      const page = context.pages()[0] ?? (await context.newPage())
      await page.addInitScript(instrument)
      await throttle(context, page, false)
      return page
    }),
  )
  for (let run = 0; run < runs; run++) {
    for (const [index, build] of builds.entries()) await interactions(pages[index], build.name, samples)
  }
  await Promise.all(contexts.map((context) => context.close()))
  report(`interactions-cpu${cpu}`, samples)
})
