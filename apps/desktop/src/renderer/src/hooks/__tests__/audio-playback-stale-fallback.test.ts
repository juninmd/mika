import { afterEach, describe, expect, it, vi } from 'vitest'
import { playViaAudioElement } from '../audio-playback-audio-element'
import { tryPlayBufferSource } from '../audio-playback-buffer-source'
import type { PlaybackRefs } from '../audio-playback-types'

// Regression: a stop()/barge-in that landed while decodeAudioData was pending bumped `playbackTokenRef`, but the
// decode failure still fell through to the <audio> fallback, which then spoke the interrupted phrase aloud.
function staleRefs(): PlaybackRefs {
  return {
    token: 1,
    playbackTokenRef: { current: 2 },
    playbackResolveRef: { current: null },
    stopPlaybackRef: { current: null },
    setIsPlaying: vi.fn(),
    finishPlayback: vi.fn(),
  }
}

const lipSync = { attachMediaElement: vi.fn(), setPlaying: vi.fn(), setWordBoundaries: vi.fn() }

afterEach(() => {
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

describe('stale playback token vs. the <audio> fallback', () => {
  it('a decode failure for an already-cancelled playback is "handled" (no fallback requested)', async () => {
    const ctx = {} as AudioContext
    const decoded = await tryPlayBufferSource(
      ctx,
      lipSync as never,
      'AAAA',
      Promise.resolve(null),
      undefined,
      1,
      staleRefs(),
    )
    expect(decoded).toBe(true)
  })

  it('a live playback whose decode fails still asks for the fallback', async () => {
    const refs = staleRefs()
    refs.playbackTokenRef.current = refs.token
    const decoded = await tryPlayBufferSource(
      {} as AudioContext,
      lipSync as never,
      'AAAA',
      Promise.resolve(null),
      undefined,
      1,
      refs,
    )
    expect(decoded).toBe(false)
  })

  it('playViaAudioElement does nothing for a cancelled playback', async () => {
    const AudioCtor = vi.fn()
    vi.stubGlobal('Audio', AudioCtor)
    const refs = staleRefs()

    await playViaAudioElement('audio/wav', 'AAAA', 1, lipSync as never, 'default', undefined, refs)

    expect(AudioCtor).not.toHaveBeenCalled()
    expect(refs.setIsPlaying).not.toHaveBeenCalled()
    expect(refs.stopPlaybackRef.current).toBeNull()
  })
})
