import { toDisplayError } from '@mika/sdk-ui'
import { useUIStore } from '../store/useUIStore'
import { getElectronApi } from './shell-api'

/**
 * Settings → OS shell, both allowlisted in Rust (`shell_actions.rs`): an https link to a named host,
 * or the app's data/logs folder by NAME (never a path). Failures surface as a toast, never silently;
 * `openExternalLink` also resolves `false` so a window without the toast layer (the wizard) can say it inline.
 */
export async function openExternalLink(url: string, errorMessage: string): Promise<boolean> {
  try {
    const open = getElectronApi().shell.system?.openExternal
    if (!open) throw new Error(errorMessage)
    await open(url)
    return true
  } catch (err) {
    useUIStore.getState().addNotification('error', toDisplayError(err, errorMessage))
    return false
  }
}

export async function revealAppFolder(kind: 'data' | 'logs', errorMessage: string): Promise<string | null> {
  try {
    const reveal = getElectronApi().shell.system?.revealPath
    if (!reveal) throw new Error(errorMessage)
    return (await reveal(kind)).path
  } catch (err) {
    useUIStore.getState().addNotification('error', toDisplayError(err, errorMessage))
    return null
  }
}
