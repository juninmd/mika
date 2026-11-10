import {
  estimateFormants,
  loudnessToOpenness,
  rmsLoudness,
  SILENT_VOWELS,
  type VowelWeights,
  vowelWeights,
} from './lip-sync-vowels'

export interface WordBoundary {
  word: string
  offset_ms: number
  duration_ms: number
}

const WORD_OPEN_LEVEL = 0.28
const MAX_WORD_TIMERS = 160

export interface LipSyncSourceOptions {
  ctx: AudioContext
  analyser: AnalyserNode
  volumeRef: { current: number }

  vowelsRef?: { current: VowelWeights }
}

export class LipSyncSource {
  private readonly ctx: AudioContext
  private readonly analyser: AnalyserNode
  private readonly volumeRef: { current: number }
  private rafRef = 0
  private running = false
  private playing = false
  private wordEnvelope = 0
  private wordTimers: ReturnType<typeof setTimeout>[] = []
  private freqData: Uint8Array<ArrayBuffer> | null = null
  private timeData: Float32Array<ArrayBuffer> | null = null
  private readonly vowelsRef: { current: VowelWeights } | null
  private readonly mediaSources = new WeakMap<HTMLMediaElement, MediaElementAudioSourceNode>()

  constructor({ ctx, analyser, volumeRef, vowelsRef }: LipSyncSourceOptions) {
    this.ctx = ctx
    this.analyser = analyser
    this.volumeRef = volumeRef
    this.vowelsRef = vowelsRef ?? null
  }

  attachBufferSource(source: AudioBufferSourceNode): GainNode {
    const gain = this.ctx.createGain()
    source.connect(gain)
    gain.connect(this.analyser)
    return gain
  }

  /** Um `MediaElementSource` por elemento: o segundo fallback era ignorado em silêncio, sem lipsync nem roteamento. */
  attachMediaElement(el: HTMLMediaElement): void {
    if (this.mediaSources.has(el)) return
    const source = this.ctx.createMediaElementSource(el)
    source.connect(this.analyser)
    this.mediaSources.set(el, source)
  }

  private setManagedTimeout(callback: () => void, delay: number): void {
    const timerId = setTimeout(() => {
      callback()
      this.wordTimers = this.wordTimers.filter((id) => id !== timerId)
    }, delay)
    this.wordTimers.push(timerId)
  }

  private clearWordTimers(): void {
    for (const timerId of this.wordTimers) clearTimeout(timerId)
    this.wordTimers = []
  }

  setWordBoundaries(words: WordBoundary[]): void {
    this.clearWordTimers()
    if (!words.length) return

    const sampled =
      words.length > MAX_WORD_TIMERS / 2
        ? words.filter((_, index) => index % 2 === 0).slice(0, MAX_WORD_TIMERS / 2)
        : words

    for (const w of sampled) {
      this.setManagedTimeout(() => {
        this.wordEnvelope = WORD_OPEN_LEVEL
      }, w.offset_ms)
      this.setManagedTimeout(
        () => {
          this.wordEnvelope = 0
        },
        w.offset_ms + w.duration_ms + 40,
      )
    }
  }

  private tick = (): void => {
    if (!this.playing) {
      this.volumeRef.current = 0
      this.wordEnvelope = 0
      if (this.vowelsRef) this.vowelsRef.current = { ...SILENT_VOWELS }
      return
    }
    if (!this.timeData || this.timeData.length !== this.analyser.fftSize) {
      this.timeData = new Float32Array(this.analyser.fftSize)
    }

    this.analyser.getFloatTimeDomainData(this.timeData)
    const openness = loudnessToOpenness(rmsLoudness(this.timeData))

    this.volumeRef.current = Math.max(openness, this.wordEnvelope)

    if (this.vowelsRef) {
      if (!this.freqData || this.freqData.length !== this.analyser.frequencyBinCount) {
        this.freqData = new Uint8Array(this.analyser.frequencyBinCount)
      }
      this.analyser.getByteFrequencyData(this.freqData)
      const formants = estimateFormants(this.freqData, this.ctx.sampleRate, this.analyser.fftSize)
      this.vowelsRef.current = vowelWeights(formants, openness)
    }

    this.rafRef = requestAnimationFrame(this.tick)
  }

  setPlaying(playing: boolean): void {
    this.playing = playing
    if (playing) {
      if (!this.running) {
        this.running = true
        this.tick()
      }
      return
    }
    this.wordEnvelope = 0
    this.clearWordTimers()
    cancelAnimationFrame(this.rafRef)
    this.running = false
    this.volumeRef.current = 0
    if (this.vowelsRef) this.vowelsRef.current = { ...SILENT_VOWELS }
  }

  dispose(): void {
    this.setPlaying(false)
  }
}
