// Pure base64/decode helpers for TTS audio payloads. No React import.

export function isLikelyBase64(input: string): boolean {
  if (!input) return false
  const normalized = input.replace(/\s+/g, '')
  if (normalized.length < 8 || normalized.length % 4 !== 0) return false
  return /^[A-Za-z0-9+/=]+$/.test(normalized)
}

export function normalizeBase64(audioBase64: string): string {
  const raw = audioBase64.includes(',') ? (audioBase64.split(',')[1] ?? '') : audioBase64
  return raw.replace(/\s+/g, '')
}

// Indexed loop instead of Uint8Array.from(str, cb), which called the callback per byte on hundred-KB payloads.
export async function decodeBase64Audio(ctx: AudioContext, normalizedBase64: string): Promise<AudioBuffer> {
  const binary = atob(normalizedBase64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
  return ctx.decodeAudioData(bytes.buffer)
}
