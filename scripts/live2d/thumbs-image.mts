import type { Page } from '@playwright/test'

export const THUMB_MAX_WIDTH = 272

export const THUMB_MIN_WIDTH = THUMB_MAX_WIDTH

export const CAPTURE_WINDOW = { width: 1400, height: 2240 }

export const THUMB_ASPECT = 3 / 4

export type Box = { x: number; y: number; width: number; height: number }

export async function measurePaintedBox(page: Page, frame: Buffer, origin: Box): Promise<Box | null> {
  const bbox = await page.evaluate(async (base64: string) => {
    function decodeBase64(text: string): Uint8Array {
      const binary = atob(text)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
      return bytes
    }

    function widenBounds(
      bounds: { minX: number; minY: number; maxX: number; maxY: number },
      x: number,
      y: number,
    ): void {
      if (x < bounds.minX) bounds.minX = x
      if (x > bounds.maxX) bounds.maxX = x
      if (y < bounds.minY) bounds.minY = y
      if (y > bounds.maxY) bounds.maxY = y
    }

    function findPaintedBounds(data: Uint8ClampedArray, width: number, height: number) {
      const bounds = { minX: width, minY: height, maxX: -1, maxY: -1 }
      const pixelCount = width * height
      for (let pixel = 0; pixel < pixelCount; pixel++) {
        if ((data[pixel * 4 + 3] ?? 0) <= 48) continue
        widenBounds(bounds, pixel % width, Math.floor(pixel / width))
      }
      return bounds
    }

    const bytes = decodeBase64(base64)
    const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }))
    const scratch = document.createElement('canvas')
    scratch.width = bitmap.width
    scratch.height = bitmap.height
    const ctx = scratch.getContext('2d')
    if (!ctx) return null
    ctx.drawImage(bitmap, 0, 0)
    const { data } = ctx.getImageData(0, 0, scratch.width, scratch.height)
    const { minX, minY, maxX, maxY } = findPaintedBounds(data, scratch.width, scratch.height)
    if (maxX < 0) return null
    return { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 }
  }, frame.toString('base64'))

  return bbox ? { ...bbox, x: origin.x + bbox.x, y: origin.y + bbox.y } : null
}

export async function shrinkToThumb(page: Page, frame: Buffer): Promise<Buffer> {
  const resized = await page.evaluate(
    async ({ base64, maxWidth }: { base64: string; maxWidth: number }) => {
      const binary = atob(base64)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }))
      if (bitmap.width <= maxWidth) return null

      const scratch = document.createElement('canvas')
      scratch.width = maxWidth
      scratch.height = Math.round((bitmap.height * maxWidth) / bitmap.width)
      const ctx = scratch.getContext('2d')
      if (!ctx) return null
      ctx.imageSmoothingQuality = 'high'
      ctx.drawImage(bitmap, 0, 0, scratch.width, scratch.height)
      const blob = await new Promise<Blob | null>((resolve) => scratch.toBlob(resolve, 'image/png'))
      if (!blob) return null
      const out = new Uint8Array(await blob.arrayBuffer())
      let text = ''
      for (const byte of out) text += String.fromCharCode(byte)
      return btoa(text)
    },
    { base64: frame.toString('base64'), maxWidth: THUMB_MAX_WIDTH },
  )
  return resized ? Buffer.from(resized, 'base64') : frame
}

function readPngWidth(png: Buffer): number {
  return png.readUInt32BE(16)
}

export function assertThumbWidth(model: { label: string }, png: Buffer): void {
  const width = readPngWidth(png)
  if (width >= THUMB_MIN_WIDTH) return
  throw new Error(
    `a miniatura de ${model.label} saiu com ${width}px de largura, abaixo do alvo de ${THUMB_MIN_WIDTH}px — ` +
      `ela seria ampliada pelo navegador no card da aba Skin e sairia borrada. ` +
      `Provável causa: a janela de captura (${CAPTURE_WINDOW.width}x${CAPTURE_WINDOW.height}) foi clampada, ` +
      `ou o palco/layout do avatar mudou.`,
  )
}

export function faceClip(painted: Box, viewport: { width: number; height: number }): Box {
  const pad = painted.width * 0.12
  const width = Math.min(painted.width + pad * 2, viewport.width)
  const height = Math.min(painted.height + pad, width / THUMB_ASPECT, viewport.height)
  const x = Math.max(0, Math.min(painted.x - pad, viewport.width - width))
  const y = Math.max(0, Math.min(painted.y - pad * 0.5, viewport.height - height))
  return { x, y, width, height }
}
