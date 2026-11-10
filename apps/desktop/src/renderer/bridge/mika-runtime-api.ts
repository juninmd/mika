import type { MikaRuntimeApi } from '../../shared/electron-api-types'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { createChatRuntimeApi } from './mika-runtime-chat'
import { createEventsRuntimeApi } from './mika-runtime-events'
import { createSettingsRuntimeApi } from './mika-runtime-settings'
import { createSystemRuntimeApi } from './mika-runtime-system'

export { sendRendererLog } from './mika-runtime-channel'

ipcRenderer.setMaxListeners(20)

export function createMikaRuntimeApi(): MikaRuntimeApi {
  return {
    ...createChatRuntimeApi(),
    ...createSettingsRuntimeApi(),
    ...createEventsRuntimeApi(),
    ...createSystemRuntimeApi(),
  }
}
