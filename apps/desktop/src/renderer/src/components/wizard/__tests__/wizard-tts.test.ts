import { describe, expect, it } from 'vitest'
import { readSavedTts, ttsSettingsToSave } from '../wizard-tts'

const finishWithoutTouching = (config: Record<string, unknown>) => ttsSettingsToSave(readSavedTts(config))

describe('wizard TTS step on an already configured install', () => {
  // Reopening the wizard and clicking "finish" used to write voice.ttsEnabled=false: Mika answered in silence.
  it('keeps the voice on when the user finishes without changing anything', () => {
    const saves = finishWithoutTouching({ voice: { ttsEnabled: true, ttsProvider: 'supertonic' } })
    expect(saves).toEqual([
      ['voice.ttsEnabled', true],
      ['voice.ttsProvider', 'supertonic'],
    ])
  })

  it('does not overwrite a provider the wizard does not offer', () => {
    expect(finishWithoutTouching({ voice: { ttsEnabled: true, ttsProvider: 'piper' } })).toEqual([])
  })

  it('keeps the voice off when the user had turned it off', () => {
    expect(finishWithoutTouching({ voice: { ttsEnabled: false, ttsProvider: 'kokoro' } })).toEqual([
      ['voice.ttsEnabled', false],
    ])
  })

  it('writes nothing when there is no saved voice config to reflect', () => {
    expect(finishWithoutTouching({})).toEqual([])
  })

  it('turns the voice off only when "not now" is chosen explicitly', () => {
    expect(ttsSettingsToSave('none')).toEqual([['voice.ttsEnabled', false]])
  })
})
