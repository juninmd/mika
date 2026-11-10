import { useEffect } from 'react'
import type { ElectronShellApi } from '../../../shared/electron-api-types'
import { rendererLog } from '../lib/renderer-logger'

function getElectronApi() {
  return (window as unknown as { electronAPI?: ElectronShellApi }).electronAPI
}

// `onPlaySound`/`onBrowserNavigating` were removed with `Events.PLAY_SOUND`/`BROWSER_NAVIGATING`
// (`check:events` audit, 2026-09-22): `entertainment:play-sound` has no Rust publisher yet — the
// gamer module's own test pins it staying an unclaimed event, not an invoke, until that lands
// (`modules/gamer/mod.rs`) — and `browser:navigating` never will again: the Rust `browser` module
// deliberately has no navigable engine (`reqwest`+`scraper` fetch-only, `modules/browser/mod.rs`
// "Decisão de produto: sem motor de browser"), so nothing can ever "navigate" to report.
export function useRuntimeEvents(onTelemetryStatus: (status: boolean) => void): void {
  useEffect(() => {
    const eAPI = getElectronApi()

    if (eAPI?.runtime?.getTelemetryStatus) {
      eAPI.runtime
        .getTelemetryStatus()
        .then((status) => {
          if (!status.enabled && !status.optInShown) onTelemetryStatus(true)
        })
        .catch(() => rendererLog.warn('[useRuntimeEvents] telemetry status failed'))
    }
  }, [onTelemetryStatus])
}
