/** PNG 8 bits sem entrelaçamento: basta para as texturas VRoid da base, sem dependência nativa. */
import { deflateSync, inflateSync } from 'node:zlib'

function crc32(bytes) {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0)
  }
  return (crc ^ 0xffffffff) >>> 0
}

function chunk(type, data) {
  const bytes = Buffer.concat([Buffer.from(type), data])
  const head = Buffer.alloc(4)
  const tail = Buffer.alloc(4)
  head.writeUInt32BE(data.length)
  tail.writeUInt32BE(crc32(bytes))
  return Buffer.concat([head, bytes, tail])
}

/** RGBA de 8 bits por canal. */
export function encodePng(width, height, rgba) {
  const rows = Buffer.alloc(height * (width * 4 + 1))
  for (let y = 0; y < height; y++) {
    rows.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1)
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header[8] = 8
  header[9] = 6
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(rows, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

export function texturePng(width, height, sample) {
  const rgba = new Uint8Array(width * height * 4)
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const color = sample((x + 0.5) / width, (y + 0.5) / height)
      const offset = (y * width + x) * 4
      for (let c = 0; c < 4; c++) rgba[offset + c] = Math.round(Math.max(0, Math.min(255, color[c] ?? 255)))
    }
  }
  return encodePng(width, height, rgba)
}

/** Devolve sempre RGBA, qualquer que seja o tipo de cor de 8 bits da origem. */
export function decodePng(png) {
  const width = png.readUInt32BE(16)
  const height = png.readUInt32BE(20)
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[png[25]]
  if (png[24] !== 8 || !channels || png[28] !== 0) throw new Error('PNG fora do subconjunto suportado')
  const parts = []
  for (let offset = 8; offset < png.length; ) {
    const length = png.readUInt32BE(offset)
    const type = png.toString('latin1', offset + 4, offset + 8)
    if (type === 'IDAT') parts.push(png.subarray(offset + 8, offset + 8 + length))
    offset += length + 12
  }
  const raw = inflateSync(Buffer.concat(parts))
  const stride = width * channels
  const pixels = new Uint8Array(height * stride)
  for (let y = 0; y < height; y++) unfilter(raw, pixels, y, stride, channels)
  const rgba = new Uint8Array(width * height * 4)
  for (let i = 0; i < width * height; i++) {
    const s = pixels.subarray(i * channels, (i + 1) * channels)
    const gray = channels <= 2
    rgba.set([s[0], gray ? s[0] : s[1], gray ? s[0] : s[2], channels % 2 === 0 ? s[channels - 1] : 255], i * 4)
  }
  return { width, height, rgba }
}

function unfilter(raw, pixels, y, stride, bpp) {
  const filter = raw[y * (stride + 1)]
  const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1))
  const out = y * stride
  for (let x = 0; x < stride; x++) {
    const a = x >= bpp ? pixels[out + x - bpp] : 0
    const b = y > 0 ? pixels[out - stride + x] : 0
    const c = x >= bpp && y > 0 ? pixels[out - stride + x - bpp] : 0
    const predictor = [0, a, b, (a + b) >> 1, paeth(a, b, c)][filter]
    pixels[out + x] = (line[x] + predictor) & 255
  }
}

function paeth(a, b, c) {
  const p = a + b - c
  const pa = Math.abs(p - a)
  const pb = Math.abs(p - b)
  const pc = Math.abs(p - c)
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
}

/** Re-encodes a PNG with this encoder, which packs the VRoid textures tighter than their export. */
export function recodePng(png) {
  const { width, height, rgba } = decodePng(png)
  return encodePng(width, height, rgba)
}
