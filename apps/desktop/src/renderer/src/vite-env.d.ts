/// <reference types="vite/client" />

declare interface Window {
  electronAPI?: import('../../shared/electron-api-types').ElectronShellApi
}
