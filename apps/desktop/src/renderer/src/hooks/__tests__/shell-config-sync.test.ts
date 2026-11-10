import { describe, expect, it } from 'vitest'
import {
  needsNeuralVadFeed,
  parseAudioConfig,
  parseConfigChange,
  parsePrivacyConfig,
  parseVoiceConfig,
} from '../shell-config-sync'

describe('parseVoiceConfig — bulk config load só toca os campos presentes', () => {
  it('undefined config: patch vazio, nunca lança', () => {
    expect(parseVoiceConfig(undefined)).toEqual({})
  })

  it('campos ausentes ficam de fora do patch — evita sobrescrever estado já carregado com undefined', () => {
    expect(parseVoiceConfig({ asrLanguage: 'en-US' })).toEqual({ voiceLang: 'en-US' })
  })

  it('inputMode inválido é ignorado, não vira "ptt" nem "continuous" por acidente', () => {
    expect(parseVoiceConfig({ inputMode: 'walkie-talkie' })).toEqual({})
  })

  it('autoMuteDuringTts só é falso quando explicitamente false — undefined e null viram true', () => {
    expect(parseVoiceConfig({ autoMuteDuringTts: false })).toEqual({ autoMuteDuringTts: false })
    expect(parseVoiceConfig({ autoMuteDuringTts: null })).toEqual({ autoMuteDuringTts: true })
  })

  it('volume não numérico (string vinda de IPC malformado) não vira ttsVolume', () => {
    expect(parseVoiceConfig({ volume: '0.5' })).toEqual({})
  })

  it('lê todos os campos reconhecidos de uma vez', () => {
    expect(
      parseVoiceConfig({
        asrLanguage: 'pt-BR',
        asrProvider: 'whisper-local',
        sttEnabled: true,
        inputMode: 'ptt',
        autoMuteDuringTts: true,
        interruptionEnabled: false,
        voiceActivityThreshold: 0.7,
        vadSilenceMs: 1200,
        vadEndpointingEnabled: false,
        volume: 0.8,
      }),
    ).toEqual({
      voiceLang: 'pt-BR',
      asrProvider: 'whisper-local',
      sttEnabled: true,
      inputMode: 'ptt',
      autoMuteDuringTts: true,
      interruptionEnabled: false,
      vadThreshold: 0.7,
      vadSilenceMs: 1200,
      vadEndpointingEnabled: false,
      ttsVolume: 0.8,
    })
  })

  it('vadSilenceMs não numérico é ignorado; vadEndpointingEnabled só é falso quando explicitamente false', () => {
    expect(parseVoiceConfig({ vadSilenceMs: '1200' })).toEqual({})
    expect(parseVoiceConfig({ vadEndpointingEnabled: false })).toEqual({ vadEndpointingEnabled: false })
    expect(parseVoiceConfig({ vadEndpointingEnabled: null })).toEqual({ vadEndpointingEnabled: true })
  })
})

describe('parseAudioConfig / parsePrivacyConfig — namespaces menores seguem a mesma regra de presença', () => {
  it('inputDeviceId não-string é ignorado', () => {
    expect(parseAudioConfig({ inputDeviceId: 42 })).toEqual({})
  })

  it('inputDeviceId string válido vira patch', () => {
    expect(parseAudioConfig({ inputDeviceId: 'device-1' })).toEqual({ inputDeviceId: 'device-1' })
  })

  it('offlineMode ausente não força false — só espelha quando presente', () => {
    expect(parsePrivacyConfig({})).toEqual({})
    expect(parsePrivacyConfig({ offlineMode: 1 })).toEqual({ offlineMode: true })
  })
})

describe('parseConfigChange — cada path do evento onConfigChanged mapeia para exatamente um campo', () => {
  it('path desconhecido (setting novo que o shell ainda não escuta) não quebra, só ignora', () => {
    expect(parseConfigChange('voice.unknownField', 'x')).toEqual({})
  })

  it('voice.inputMode com valor inválido é descartado mesmo vindo de um evento live', () => {
    expect(parseConfigChange('voice.inputMode', 'invalid')).toEqual({})
  })

  it('audio.inputDeviceId roteia para o mesmo campo que o bulk load usa', () => {
    expect(parseConfigChange('audio.inputDeviceId', 'device-2')).toEqual({ inputDeviceId: 'device-2' })
  })

  it('privacy.offlineMode liga o modo offline a partir de um evento pontual', () => {
    expect(parseConfigChange('privacy.offlineMode', true)).toEqual({ offlineMode: true })
  })

  it('voice.voiceActivityThreshold ignora valor não numérico', () => {
    expect(parseConfigChange('voice.voiceActivityThreshold', 'high')).toEqual({})
  })

  it('voice.vadSilenceMs roteia para o hangover ao vivo do listening path principal', () => {
    expect(parseConfigChange('voice.vadSilenceMs', 1500)).toEqual({ vadSilenceMs: 1500 })
    expect(parseConfigChange('voice.vadSilenceMs', '1500')).toEqual({})
  })

  it('voice.vadEndpointingEnabled desligado suspende o auto-fim por silêncio', () => {
    expect(parseConfigChange('voice.vadEndpointingEnabled', false)).toEqual({ vadEndpointingEnabled: false })
  })
})

describe('voice.vadEngine — motor de VAD que decide se a fala recortada vai para o STT', () => {
  it('carga e evento pontual só aceitam os dois motores reais', () => {
    expect(parseVoiceConfig({ vadEngine: 'silero' })).toEqual({ vadEngine: 'silero' })
    expect(parseVoiceConfig({ vadEngine: 'webrtc' })).toEqual({})
    expect(parseConfigChange('voice.vadEngine', 'energy')).toEqual({ vadEngine: 'energy' })
  })

  it('com Silero o microfone alimenta o VAD neural mesmo sem barge-in; mic desligado nunca', () => {
    const off = { vadIgnoreTyping: false }
    expect(needsNeuralVadFeed({ ...off, interruptionEnabled: false, vadEngine: 'silero', micEnabled: true })).toBe(true)
    expect(needsNeuralVadFeed({ ...off, interruptionEnabled: false, vadEngine: 'energy', micEnabled: true })).toBe(
      false,
    )
    expect(needsNeuralVadFeed({ ...off, interruptionEnabled: true, vadEngine: 'energy', micEnabled: true })).toBe(true)
    expect(needsNeuralVadFeed({ ...off, interruptionEnabled: true, vadEngine: 'silero', micEnabled: false })).toBe(
      false,
    )
  })
})

describe('voice.vadIgnoreTyping — o VAD em Rust é quem reconhece tecla e descarta o trecho só-teclado', () => {
  it('ligado, o mic alimenta o VAD mesmo no motor de energia sem barge-in (senão o toggle seria morto)', () => {
    expect(
      needsNeuralVadFeed({ vadIgnoreTyping: true, interruptionEnabled: false, vadEngine: 'energy', micEnabled: true }),
    ).toBe(true)
    expect(
      needsNeuralVadFeed({ vadIgnoreTyping: true, interruptionEnabled: false, vadEngine: 'energy', micEnabled: false }),
    ).toBe(false)
  })

  it('carga e evento pontual: só falso quando explicitamente false (default do schema é ligado)', () => {
    expect(parseVoiceConfig({ vadIgnoreTyping: false })).toEqual({ vadIgnoreTyping: false })
    expect(parseVoiceConfig({ vadIgnoreTyping: null })).toEqual({ vadIgnoreTyping: true })
    expect(parseConfigChange('voice.vadIgnoreTyping', false)).toEqual({ vadIgnoreTyping: false })
  })
})
