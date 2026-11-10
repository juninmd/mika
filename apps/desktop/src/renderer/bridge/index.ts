import { contextBridge } from '../../shared/tauri-renderer-runtime'
import { createElectronShellApi } from './electron-shell-api'
import { createMikaRuntimeApi } from './mika-runtime-api'

const mikaRuntimeApi = createMikaRuntimeApi()

contextBridge.exposeInMainWorld('electronAPI', createElectronShellApi(mikaRuntimeApi))
