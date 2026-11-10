/** Regenerates the VRM skin thumbnails (`public/vrm/mika/preview-<look>.png`) with the real adapter. */
// Roster-card portraits for the Skin screen's character select: bust framing at a 3/4 angle
// (`frameShowcase`), the same studio lights as the live stage (`mountShowcaseStage`), a light smile,
// and a transparent background so the card's own themed gradient shows through. The full-body
// front shots before this were a speck of face in a tall card. Math.random is seeded, so blink and
// idle never land mid-frame differently between runs: same input, same PNG.
import { readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { extname, resolve } from 'node:path'
import { build } from 'esbuild'
import { chromium } from 'playwright'

const PUBLIC_DIR = resolve('apps/desktop/src/renderer/public')
// 4:5 like the roster card, ~2x its on-screen size for HiDPI.
const WIDTH = 512
const HEIGHT = 640
/** 3/4 view, turned toward the viewer's left — the same rest angle as the live stage. */
const YAW = 0.42
/** Warm violet rim, close to the default theme accent; the PNG cannot follow a runtime theme. */
const RIM = 0xb48cff

const bundle = await build({
  stdin: {
    contents: `
      import { VrmAvatar } from './apps/desktop/src/renderer/src/components/avatar/vrm-adapter';
      import { HARU_OUTFITS, VRM_LOOKS, parseVrmModelPath, vrmModelPath } from './apps/desktop/src/renderer/src/components/avatar/vrm-look';
      import { mountShowcaseStage } from './apps/desktop/src/renderer/src/components/avatar/vrm-showcase';
      import { frameShowcase } from './apps/desktop/src/renderer/src/components/avatar/vrm-showcase-framing';
      window.looks = [...VRM_LOOKS, ...HARU_OUTFITS].map((look) => look.id);
      window.renderLook = async (id) => {
        globalThis.__reseed();
        document.querySelector('canvas')?.remove();
        const canvas = document.body.appendChild(document.createElement('canvas'));
        canvas.style.cssText = 'width:${WIDTH}px;height:${HEIGHT}px;display:block';
        const avatar = new VrmAvatar(canvas, ${WIDTH}, ${HEIGHT});
        avatar.setIdleEnabled(false);
        const { url, look } = parseVrmModelPath(vrmModelPath('/vrm/mika/mika.vrm', id));
        await avatar.load(url, look);
        avatar.setExpression('happy', 0.35);
        // Let the expression spring settle (~0.3 s) before the shot.
        for (let i = 0; i < 40; i++) avatar.update(1 / 60);
        // Measured on the settled pose: the rest pose leans the head away from where load left it.
        const { bounds, head } = avatar.modelFrame();
        mountShowcaseStage(avatar.scene, bounds.min.y, bounds.max.x - bounds.min.x, ${RIM});
        frameShowcase(avatar.camera, bounds, 'bust', ${YAW}, head);
        avatar.render();
      };
    `,
    resolveDir: process.cwd(),
  },
  bundle: true,
  format: 'esm',
  write: false,
  logLevel: 'silent',
})

const TYPES: Record<string, string> = { '.vrm': 'model/gltf-binary', '.png': 'image/png', '.js': 'text/javascript' }
const html = '<body style="margin:0;background:transparent"><script type="module" src="/previews.js"></script></body>'
const server = createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url ?? '/', 'http://x').pathname)
  if (path === '/previews.js')
    return res.writeHead(200, { 'Content-Type': TYPES['.js'] }).end(bundle.outputFiles[0].contents)
  if (path === '/favicon.ico') return res.writeHead(204).end()
  if (path === '/') return res.end(html)
  const file = resolve(PUBLIC_DIR, `.${path}`)
  if (!file.startsWith(PUBLIC_DIR)) return res.writeHead(403).end()
  try {
    res.setHeader('Content-Type', TYPES[extname(file)] ?? 'application/octet-stream')
    res.end(await readFile(file))
  } catch {
    res.writeHead(404).end()
  }
})
await new Promise<void>((done) => server.listen(0, '127.0.0.1', done))
const address = server.address()
if (!address || typeof address === 'string') throw new Error('preview server has no TCP port')

// `pnpm vrm:previews msedge|chrome` uses an installed browser when Playwright's Chromium was not downloaded;
// a third argument (`seifuku,maid`) renders only those looks.
const only = process.argv[3]?.split(',')
// A path instead of a channel (`pnpm vrm:previews /opt/pw-browsers/chromium`) launches that binary —
// for a machine whose Chromium does not match Playwright's pinned build.
const browserArg = process.argv[2] || undefined
const browser = await chromium.launch({
  headless: true,
  ...(browserArg?.startsWith('/') ? { executablePath: browserArg } : { channel: browserArg }),
})
const errors: string[] = []
try {
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 })
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text())
  })
  await page.addInitScript('globalThis.__name = (fn) => fn')
  // mulberry32: deterministic blink/idle timing, so a re-run reproduces the same thumbnails.
  await page.addInitScript(`{
    let seed = 0;
    globalThis.__reseed = () => { seed = 0x6d696b61; };
    Math.random = () => {
      seed = (seed + 0x6d2b79f5) | 0;
      let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }`)
  await page.goto(`http://127.0.0.1:${address.port}`)
  await page
    .waitForFunction(() => 'renderLook' in window)
    .catch((error: Error) => {
      throw new Error([error.message, ...errors].join('\n'))
    })
  const looks = await page.evaluate(() => (window as unknown as { looks: string[] }).looks)
  for (const id of looks.filter((look) => !only || only.includes(look))) {
    await page.evaluate((look) => (window as unknown as { renderLook(id: string): Promise<void> }).renderLook(look), id)
    const out = resolve(PUBLIC_DIR, `vrm/mika/preview-${id}.png`)
    const png = await page.locator('canvas').screenshot({ path: out, omitBackground: true })
    // An opaque corner means the black background is back (first post-processed frame).
    const corner = await page.evaluate(async (b64) => {
      const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob())
      const ctx = new OffscreenCanvas(bitmap.width, bitmap.height).getContext('2d')
      ctx?.drawImage(bitmap, 0, 0)
      return ctx?.getImageData(2, 2, 1, 1).data[3]
    }, png.toString('base64'))
    if (corner !== 0) throw new Error(`background not transparent for ${id} (alpha ${corner})`)
    process.stdout.write(`${out}\n`)
  }
  if (errors.length) throw new Error(errors.join('\n'))
} finally {
  await browser.close()
  await new Promise<void>((done) => server.close(() => done()))
}
