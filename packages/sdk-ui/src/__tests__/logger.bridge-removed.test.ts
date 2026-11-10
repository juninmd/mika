import { afterEach, describe, expect, it, vi } from 'vitest'

function setElectronApi(api: unknown): void {
  ;(globalThis as { electronAPI?: unknown }).electronAPI = api
}

async function freshLogger() {
  vi.resetModules()
  return import('../logger')
}

describe('createUiLogger — bridge disappears after going live', () => {
  afterEach(() => {
    setElectronApi(undefined)
    vi.restoreAllMocks()
  })

  it('falls back to console when the bridge is torn down mid-session', async () => {
    const bridge = vi.fn().mockResolvedValue(true)
    setElectronApi({ runtime: { log: bridge } })
    const consoleInfo = vi.spyOn(console, 'info').mockImplementation(() => {})
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')

    log.info('goes through bridge')
    await vi.waitFor(() => expect(bridge).toHaveBeenCalledTimes(1))

    // Simulate the electronAPI surface being torn down (e.g. window reload mid-flight).
    setElectronApi(undefined)
    log.info('bridge is gone now')

    expect(consoleInfo).toHaveBeenCalledWith('[scope] bridge is gone now')
  })
})

describe('createUiLogger — pending queue caps at MAX_PENDING while probing', () => {
  afterEach(() => {
    setElectronApi(undefined)
    vi.restoreAllMocks()
  })

  it('drops overflow entries instead of growing unbounded, but delivers the ones it kept once live', async () => {
    let resolveBridge: (value: boolean) => void = () => {}
    const bridge = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          resolveBridge = resolve
        }),
    )
    setElectronApi({ runtime: { log: bridge } })
    const { createUiLogger } = await freshLogger()
    const log = createUiLogger('scope')

    // MAX_PENDING is 200; push well past it while the probe is still in flight.
    for (let i = 0; i < 250; i++) log.info(`entry-${i}`)
    expect(bridge).toHaveBeenCalledTimes(1)

    resolveBridge(true)
    // 1 probe call + 199 drained (200 kept in the pending queue minus the probe itself, which
    // was removed before draining so it is not delivered twice).
    await vi.waitFor(() => expect(bridge).toHaveBeenCalledTimes(200))
  })
})
