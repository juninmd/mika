import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import type { Page } from '@playwright/test'
import { launchCaptureApp } from '../lib/tauri-capture-app.mts'

const ROOT = resolve(import.meta.dirname, '..', '..')
const OUT_DIR = join(ROOT, 'apps', 'desktop', 'build')
const OUT_ICO = join(OUT_DIR, 'icon.ico')
const OUT_PNG = join(OUT_DIR, 'icon.png')

const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

const HEAD_HEIGHT_RATIO = 0.3
const HEAD_TOP_PADDING_RATIO = 0.015

function buildIco(pngs: { size: number; data: Buffer }[]): Buffer {
  const header = Buffer.alloc(6)
  header.writeUInt16LE(0, 0)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(pngs.length, 4)

  const entries: Buffer[] = []
  let offset = 6 + pngs.length * 16

  for (const { size, data } of pngs) {
    const entry = Buffer.alloc(16)
    entry.writeUInt8(size >= 256 ? 0 : size, 0)
    entry.writeUInt8(size >= 256 ? 0 : size, 1)
    entry.writeUInt8(0, 2)
    entry.writeUInt8(0, 3)
    entry.writeUInt16LE(1, 4)
    entry.writeUInt16LE(32, 6)
    entry.writeUInt32LE(data.length, 8)
    entry.writeUInt32LE(offset, 12)
    entries.push(entry)
    offset += data.length
  }

  return Buffer.concat([header, ...entries, ...pngs.map((p) => p.data)])
}

type HeadCut = { png: string; layers: string[]; body: [number, number]; crop: [number, number] }

/** Trim, head crop and every icon size, drawn by the WebView's own canvas (no image library). */
function cutHead(page: Page, raw: Buffer): Promise<HeadCut> {
  return page.evaluate(
    async ({ base64, sizes, headRatio, topRatio }) => {
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0))
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }))
      const canvas = (w: number, h: number) => Object.assign(document.createElement('canvas'), { width: w, height: h })
      const full = canvas(bitmap.width, bitmap.height)
      const fullCtx = full.getContext('2d', { willReadFrequently: true })
      if (!fullCtx) throw new Error('canvas 2d indisponível')
      fullCtx.drawImage(bitmap, 0, 0)
      const { data } = fullCtx.getImageData(0, 0, full.width, full.height)
      let [minX, minY, maxX, maxY] = [full.width, full.height, -1, -1]
      for (let y = 0; y < full.height; y++)
        for (let x = 0; x < full.width; x++)
          if ((data[(y * full.width + x) * 4 + 3] ?? 0) > 1) {
            minX = Math.min(minX, x)
            maxX = Math.max(maxX, x)
            minY = Math.min(minY, y)
            maxY = Math.max(maxY, y)
          }
      if (maxX < 0) throw new Error('recorte vazio — o avatar não renderizou')
      const [bodyW, bodyH] = [maxX - minX + 1, maxY - minY + 1]
      const side = Math.round(bodyH * headRatio)
      const top = Math.round(bodyH * topRatio)
      const left = Math.max(0, Math.round(bodyW / 2 - side / 2))
      const [cropW, cropH] = [Math.min(side, bodyW - left), Math.min(side, bodyH - top)]
      const encode = async (source: CanvasImageSource, sw: number, sh: number, size: number, contain: boolean) => {
        const out = canvas(size, size)
        const ctx = out.getContext('2d')
        if (!ctx) throw new Error('canvas 2d indisponível')
        ctx.imageSmoothingQuality = 'high'
        const scale = contain ? Math.min(size / sw, size / sh) : Math.max(size / sw, size / sh)
        const [dw, dh] = [sw * scale, sh * scale]
        ctx.drawImage(source, (size - dw) / 2, (size - dh) / 2, dw, dh)
        const blob = await new Promise<Blob | null>((done) => out.toBlob(done, 'image/png'))
        if (!blob) throw new Error('toBlob falhou')
        let text = ''
        for (const byte of new Uint8Array(await blob.arrayBuffer())) text += String.fromCharCode(byte)
        return btoa(text)
      }
      const head = canvas(cropW, cropH)
      head.getContext('2d')?.drawImage(full, minX + left, minY + top, cropW, cropH, 0, 0, cropW, cropH)
      const png = await encode(head, cropW, cropH, 512, true)
      const square = await createImageBitmap(new Blob([Uint8Array.from(atob(png), (c) => c.charCodeAt(0))]))
      const layers = await Promise.all(sizes.map((size) => encode(square, 512, 512, size, false)))
      return { png, layers, body: [bodyW, bodyH] as [number, number], crop: [cropW, cropH] as [number, number] }
    },
    {
      base64: raw.toString('base64'),
      sizes: ICO_SIZES,
      headRatio: HEAD_HEIGHT_RATIO,
      topRatio: HEAD_TOP_PADDING_RATIO,
    },
  )
}

async function captureHead(): Promise<HeadCut> {
  const userDataDir = mkdtempSync(join(tmpdir(), 'mika-icon-'))
  const app = await launchCaptureApp(userDataDir, {
    setup: { wizardCompleted: true },
    telemetry: { optInShown: true },
    voice: { ttsEnabled: false, sttEnabled: false },
    vision: { enabled: false },
    brain: { enabled: false },
  })
  try {
    await app.setSize(1400, 1600)
    await app.page.locator('canvas').first().waitFor({ state: 'visible', timeout: 30_000 })
    await app.page.waitForTimeout(6_000)
    const raw = await app.page.screenshot({ omitBackground: true, type: 'png' })
    return await cutHead(app.page, raw)
  } finally {
    await app.close()
    rmSync(userDataDir, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 })
  }
}

const cut = await captureHead()
mkdirSync(OUT_DIR, { recursive: true })
writeFileSync(OUT_PNG, Buffer.from(cut.png, 'base64'))
writeFileSync(
  OUT_ICO,
  buildIco(cut.layers.map((b64, i) => ({ size: ICO_SIZES[i] ?? 0, data: Buffer.from(b64, 'base64') }))),
)
console.log(`✅ ícone gerado a partir do avatar renderizado (${cut.body.join('x')} -> ${cut.crop.join('x')})`)
console.log(`   ${OUT_ICO} (${ICO_SIZES.join(', ')} px)`)
console.log(`   ${OUT_PNG} (512 px, para macOS/Linux e para conferir o enquadramento)`)
