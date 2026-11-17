import { describe, expect, it, vi } from 'vitest'
import { createPartialTranscriber } from '../speech-partials'

const pcm = () => new Uint8Array(4)
const flush = () => new Promise((resolve) => setTimeout(resolve, 0))

function setup(enabled = true) {
  const resolvers: Array<(text: string) => void> = []
  const transcribe = vi.fn(
    () =>
      new Promise<{ success: boolean; data: { text: string } }>((r) =>
        resolvers.push((text) => r({ success: true, data: { text } })),
      ),
  )
  const publish = vi.fn()
  const partials = createPartialTranscriber({ transcribe, publish, enabled: () => enabled, intervalMs: 1000 })
  return { partials, transcribe, publish, resolvers }
}

describe('createPartialTranscriber', () => {
  // The chat shows the "typing" bubble the moment speech is confirmed, before any text exists.
  it('announces the utterance with an empty partial, then streams the growing transcript', async () => {
    const { partials, transcribe, publish, resolvers } = setup()
    partials.tick(0, true, pcm)
    expect(publish).toHaveBeenLastCalledWith('')
    partials.tick(500, true, pcm)
    expect(transcribe).not.toHaveBeenCalled()
    partials.tick(1000, true, pcm)
    resolvers[0]('oi Mika')
    await flush()
    expect(publish).toHaveBeenLastCalledWith('oi Mika')
  })

  // Whisper on CPU takes ~0.9 s: a second request while one runs would queue behind the final transcript.
  it('keeps a single partial request in flight', () => {
    const { partials, transcribe } = setup()
    partials.tick(0, true, pcm)
    partials.tick(1000, true, pcm)
    partials.tick(2500, true, pcm)
    expect(transcribe).toHaveBeenCalledTimes(1)
  })

  // Once the user pauses, the early/final transcript is about to run: partials must not compete with it.
  it('does not request partials during silence', () => {
    const { partials, transcribe } = setup()
    partials.tick(0, true, pcm)
    partials.tick(1500, false, pcm)
    expect(transcribe).not.toHaveBeenCalled()
  })

  it('drops a partial that resolves after its utterance ended', async () => {
    const { partials, publish, resolvers } = setup()
    partials.tick(0, true, pcm)
    partials.tick(1000, true, pcm)
    partials.endUtterance()
    resolvers[0]('texto velho')
    await flush()
    expect(publish).not.toHaveBeenCalledWith('texto velho')
  })

  // Chat closed: nobody reads partials, so no STT is spent on them.
  it('does nothing while nobody is watching', () => {
    const { partials, transcribe, publish } = setup(false)
    partials.tick(0, true, pcm)
    partials.tick(1000, true, pcm)
    expect(publish).not.toHaveBeenCalled()
    expect(transcribe).not.toHaveBeenCalled()
  })
})
