import { useEffect, useState } from 'react'
import { getElectronApi } from '../lib/shell-api'

function readPath(config: Record<string, unknown>, path: string): unknown {
  return path.split('.').reduce<unknown>((node, key) => (node as Record<string, unknown> | undefined)?.[key], config)
}

/** A boolean setting kept live: read once, then followed through `onConfigChanged`. */
export function useConfigFlag(path: string, fallback: boolean): boolean {
  const [on, setOn] = useState(fallback)
  useEffect(() => {
    const runtime = getElectronApi()?.runtime
    if (!runtime) return
    runtime
      .getSettingsConfig()
      .then(({ config }) => {
        const value = readPath(config as Record<string, unknown>, path)
        setOn(typeof value === 'boolean' ? value : fallback)
      })
      .catch(() => {})
    return runtime.onConfigChanged((change) => {
      if (change.path === path) setOn(change.value === true)
    })
  }, [path, fallback])
  return on
}

export function saveConfigFlag(path: string, on: boolean): void {
  void getElectronApi()?.runtime.saveSetting(path, on)
}
