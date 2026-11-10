// Asymmetric int16 range is the PCM16 convention; clamping both sides to ±32767 loses a value.
export function float32ToInt16Sample(sample: number): number {
  const clamped = Math.max(-1, Math.min(1, sample))
  return Math.round(clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff)
}

/** Converts a Float32 buffer (expected range `[-1, 1]`) to PCM16, sample by sample. */
export function float32ArrayToInt16(input: Float32Array): Int16Array {
  const out = new Int16Array(input.length)
  for (let i = 0; i < input.length; i++) {
    out[i] = float32ToInt16Sample(input[i] ?? 0)
  }
  return out
}
