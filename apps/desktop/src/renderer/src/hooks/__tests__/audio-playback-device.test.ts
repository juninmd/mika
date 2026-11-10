import { afterEach, describe, expect, it, vi } from 'vitest'

const store = new Map<string, string>()
vi.mock('../../lib/safe-local-storage', () => ({
  readLocalStorage: (key: string) => store.get(key) ?? null,
}))
vi.mock('../../lib/renderer-logger', () => ({
  rendererLog: { debug: vi.fn(), warn: vi.fn(), error: vi.fn(), info: vi.fn() },
}))

import { AUDIO_OUTPUT_DEVICE_KEY } from '../../lib/audio-storage-keys'
import { rendererLog } from '../../lib/renderer-logger'
import { applyOutputDevice, getConfiguredOutputDeviceId } from '../audio-playback-device'

afterEach(() => {
  store.clear()
  vi.clearAllMocks()
})

describe('getConfiguredOutputDeviceId — resolves the saved output sink', () => {
  it('returns empty string when nothing was saved', () => {
    expect(getConfiguredOutputDeviceId()).toBe('')
  })

  it('returns empty string for the sentinel "default" device', () => {
    store.set(AUDIO_OUTPUT_DEVICE_KEY, 'default')
    expect(getConfiguredOutputDeviceId()).toBe('')
  })

  it('returns the saved device id otherwise', () => {
    store.set(AUDIO_OUTPUT_DEVICE_KEY, 'speaker-42')
    expect(getConfiguredOutputDeviceId()).toBe('speaker-42')
  })
})

describe('applyOutputDevice — routes audio to the chosen sink', () => {
  it('calls setSinkId on a capable target', async () => {
    const setSinkId = vi.fn().mockResolvedValue(undefined)
    await applyOutputDevice({ setSinkId } as unknown as AudioContext, 'sink-1')
    expect(setSinkId).toHaveBeenCalledWith('sink-1')
  })

  it('warns and no-ops when the target has no setSinkId support', async () => {
    await applyOutputDevice({} as AudioContext, 'sink-1')
    expect(rendererLog.warn).toHaveBeenCalled()
  })

  it('warns instead of throwing when setSinkId rejects', async () => {
    const setSinkId = vi.fn().mockRejectedValue(new Error('no such device'))
    await expect(applyOutputDevice({ setSinkId } as unknown as AudioContext, 'missing')).resolves.toBeUndefined()
    expect(rendererLog.warn).toHaveBeenCalled()
  })
})
