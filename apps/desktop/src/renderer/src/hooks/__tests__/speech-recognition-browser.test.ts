import { describe, expect, it } from 'vitest'
import { isFatalSpeechError, speechErrorMessageKey } from '../shell-commands'
import { describeMicOpenFailure } from '../speech-recognition-browser'

// `getUserMedia` rejects with a DOMException whose human `message` ("Permission denied") used to be what
// reached the shell, so the fatal-token match never fired and the mic stayed "on" with a dead stream.
function domError(name: string, message: string): Error {
  const err = new Error(message)
  err.name = name
  return err
}

describe('describeMicOpenFailure → mic is switched off for permission/device failures', () => {
  it.each([
    ['NotAllowedError', 'Permission denied'],
    ['SecurityError', 'Only secure origins are allowed'],
  ])('%s becomes the "not-allowed" code the shell treats as fatal', (name, message) => {
    const code = describeMicOpenFailure(domError(name, message))
    expect(code).toBe('not-allowed')
    expect(isFatalSpeechError(code)).toBe(true)
  })

  it.each([
    ['NotFoundError', 'Requested device not found'],
    ['OverconstrainedError', 'Constraint deviceId not satisfied'],
  ])('%s keeps its name (a fatal token) instead of its free-text message', (name, message) => {
    const code = describeMicOpenFailure(domError(name, message))
    expect(code).toBe(name)
    expect(isFatalSpeechError(code)).toBe(true)
  })

  it('a busy/interrupted device stays transient and does not disable the mic', () => {
    const code = describeMicOpenFailure(domError('NotReadableError', 'Could not start audio source'))
    expect(code).toBe('microphone-transient-NotReadableError')
    expect(isFatalSpeechError(code)).toBe(false)
  })

  it('reads `name` from DOMException-like objects that are not Error instances', () => {
    expect(describeMicOpenFailure({ name: 'NotAllowedError', message: 'Permission denied' })).toBe('not-allowed')
  })

  it('keeps the message for other errors and flags a non-error rejection as unavailable', () => {
    expect(describeMicOpenFailure(new Error('boom'))).toBe('boom')
    expect(describeMicOpenFailure(undefined)).toBe('media-recorder-unavailable')
  })
})

describe('speechErrorMessageKey → localized toast per failure class', () => {
  it.each([
    ['not-allowed', 'shell.composer.micDenied'],
    ['NotAllowedError', 'shell.composer.micDenied'],
    ['NotFoundError', 'shell.composer.micNotFound'],
    ['OverconstrainedError', 'shell.composer.micNotFound'],
    ['audio-capture', 'shell.composer.micNotFound'],
    ['offline-mode-blocks-browser-stt', 'shell.voice.offlineBlocksBrowserStt'],
    ['browser-stt-unavailable-in-electron', 'shell.voice.browserSttUnavailable'],
  ])('%s → %s', (code, key) => {
    expect(speechErrorMessageKey(code)).toBe(key)
  })

  it('unknown codes have no dedicated message (the caller appends the raw detail)', () => {
    expect(speechErrorMessageKey('decoder exploded')).toBeUndefined()
  })
})
