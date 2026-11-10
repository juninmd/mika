import { useCallback, useRef } from 'react'
import { getElectronApi } from '../lib/shell-api'

export function useDockActions() {
  const eAPI = useRef(getElectronApi()).current

  const openSettings = useCallback(() => {
    return eAPI.shell.openSettings?.()
  }, [eAPI.shell.openSettings])

  return {
    openSettings,
  }
}
