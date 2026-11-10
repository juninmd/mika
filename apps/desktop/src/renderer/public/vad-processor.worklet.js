const VAD_SAMPLE_RATE = 16000
const SPEECH_TIMEOUT_MS = 500
// Fala só começa depois de 30 ms acima do piso: um clique de mouse não abre o microfone.
const SPEECH_ONSET_MS = 30
// Piso de energia absoluto; o piso efetivo sobe com o ruído ambiente (ver noiseFloor).
const ENERGY_THRESHOLD_MIN = 0.02
const NOISE_FLOOR_RATIO = 2.5
// O piso de ruído segue quedas rápido (ambiente ficou quieto) e subidas devagar (fala não vira "ruído").
const NOISE_FALL_ALPHA = 0.2
const NOISE_RISE_ALPHA = 0.005

const CHUNK_SAMPLES = (VAD_SAMPLE_RATE * 20) / 1000

const MAX_QUANTUM_SAMPLES = 4096

const ANTI_ALIAS_CUTOFF_HZ = 7000

class VADProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super(options)
    this.isSpeaking = false
    const inputRate = options?.processorOptions?.sampleRate || sampleRate || 48000
    this.inputRate = inputRate
    this.step = inputRate / VAD_SAMPLE_RATE
    this.readPos = 0
    this.chunk = new Float32Array(CHUNK_SAMPLES)
    this.chunkFill = 0

    // Contadores em AMOSTRAS, não em quanta: um quantum são 128 amostras (2,7 ms a 48 kHz), e contar
    // quanta como se fossem quadros de 30 ms fechava a fala depois de ~40 ms de pausa — toda vírgula
    // virava fim de frase.
    this.silenceSamples = 0
    this.voicedSamples = 0
    this.silenceLimit = (SPEECH_TIMEOUT_MS / 1000) * inputRate
    this.onsetSamples = (SPEECH_ONSET_MS / 1000) * inputRate
    this.noiseFloor = ENERGY_THRESHOLD_MIN

    const dt = 1 / inputRate
    const rc = 1 / (2 * Math.PI * ANTI_ALIAS_CUTOFF_HZ)
    this.lpAlpha = dt / (rc + dt)
    this.lpState = 0
    this.filtered = new Float32Array(MAX_QUANTUM_SAMPLES)
  }

  stream(channelData) {
    const len = channelData.length
    const filtered = this.filtered
    let state = this.lpState
    const alpha = this.lpAlpha
    for (let i = 0; i < len; i++) {
      state += alpha * (channelData[i] - state)
      filtered[i] = state
    }
    this.lpState = state

    while (this.readPos < len) {
      this.chunk[this.chunkFill++] = filtered[Math.floor(this.readPos)] ?? 0
      if (this.chunkFill === CHUNK_SAMPLES) {
        this.port.postMessage({ type: 'audio', audio: this.chunk.slice() })
        this.chunkFill = 0
      }
      this.readPos += this.step
    }
    this.readPos -= len
  }

  rms(channelData) {
    let energy = 0
    for (let i = 0; i < channelData.length; i++) energy += channelData[i] * channelData[i]
    return Math.sqrt(energy / channelData.length)
  }

  updateNoiseFloor(energy) {
    if (this.isSpeaking) return
    const alpha = energy < this.noiseFloor ? NOISE_FALL_ALPHA : NOISE_RISE_ALPHA
    this.noiseFloor += alpha * (energy - this.noiseFloor)
  }

  threshold() {
    return Math.max(ENERGY_THRESHOLD_MIN, this.noiseFloor * NOISE_FLOOR_RATIO)
  }

  detect(channelData) {
    const energy = this.rms(channelData)
    this.updateNoiseFloor(energy)
    const isSpeech = energy > this.threshold()
    const n = channelData.length

    if (isSpeech) {
      this.silenceSamples = 0
      if (!this.isSpeaking) {
        this.voicedSamples += n
        if (this.voicedSamples >= this.onsetSamples) {
          this.isSpeaking = true
          this.port.postMessage({ type: 'speech-start' })
        }
      }
      return
    }

    this.voicedSamples = 0
    if (!this.isSpeaking) return
    this.silenceSamples += n
    if (this.silenceSamples >= this.silenceLimit) {
      this.isSpeaking = false
      this.silenceSamples = 0
      this.port.postMessage({ type: 'speech-end' })
    }
  }

  process(inputs) {
    const input = inputs[0]
    if (!input || input.length === 0) return true

    const channelData = input[0]
    if (!channelData || channelData.length === 0) return true

    this.stream(channelData)
    this.detect(channelData)
    return true
  }
}

registerProcessor('vad-processor', VADProcessor)
