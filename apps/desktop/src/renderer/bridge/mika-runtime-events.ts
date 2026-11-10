import type { ActivityPayload, PowerProfileState } from '@mika/sdk/contracts'
import { DESKTOP_INVOKE_CHANNELS, DESKTOP_RENDERER_EVENT_CHANNELS, DESKTOP_SEND_CHANNELS } from '@mika/sdk/contracts'
import type { EventsRuntimeApi } from '../../shared/electron-api-types'
import { MIKA_EVENT_CHANNELS } from '../../shared/mika-event-channels'
import { ipcRenderer } from '../../shared/tauri-renderer-runtime'
import { onChannel } from './mika-runtime-channel'

export function createEventsRuntimeApi(): EventsRuntimeApi {
  return {
    // Main-reported activity, replacing the renderer's own reconstruction from indirect events (two disagreeing state machines).
    onActivity: (callback: (payload: ActivityPayload) => void) => onChannel(MIKA_EVENT_CHANNELS.activity, callback),
    onMoodChanged: (callback: (payload: { valence: number; arousal: number; label: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.moodChanged, callback),
    onListeningCommand: (callback: (payload: { listening: boolean }) => void) => {
      const offStart = onChannel(MIKA_EVENT_CHANNELS.listeningStarted, () => callback({ listening: true }))
      const offStop = onChannel(MIKA_EVENT_CHANNELS.listeningStopped, () => callback({ listening: false }))
      return () => {
        offStart()
        offStop()
      }
    },
    onSensesPaused: (callback: (payload: { paused: boolean }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.sensesPaused, callback),
    onForceStopTts: (callback: (payload: { reason?: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.forceStopTts, callback),
    onInterruption: (
      callback: (payload: {
        transcript: string
        is_interruption?: boolean
        timestamp?: number
        window_label?: string
      }) => void,
    ) => onChannel(MIKA_EVENT_CHANNELS.interruption, callback),
    onNotification: (
      callback: (payload: {
        message: string
        type?: 'info' | 'success' | 'warning' | 'error'
        source?: string
      }) => void,
    ) => onChannel(MIKA_EVENT_CHANNELS.notification, callback),
    onPersonCatalogUpdated: (callback: (payload: { personId: string; name?: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.personCatalogUpdated, callback),
    onPersonCatalogDeleted: (callback: (payload: { personId: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.personCatalogDeleted, callback),
    getPowerState: () =>
      ipcRenderer.invoke(DESKTOP_INVOKE_CHANNELS.GetPowerState) as Promise<
        { onBatteryPower: boolean } & Partial<PowerProfileState>
      >,
    onPowerProfile: (callback: (payload: PowerProfileState) => void) =>
      onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.PowerProfile, callback),

    onPowerModeChanged: (callback: (payload: { onBatteryPower: boolean }) => void) => {
      const offBattery = onChannel(MIKA_EVENT_CHANNELS.powerBattery, () => callback({ onBatteryPower: true }))
      const offAc = onChannel(MIKA_EVENT_CHANNELS.powerAc, () => callback({ onBatteryPower: false }))
      return () => {
        offBattery()
        offAc()
      }
    },
    sendMicState: (enabled: boolean) => {
      ipcRenderer.send(DESKTOP_SEND_CHANNELS.VoiceMicState, { enabled })
    },
    onMicState: (callback: (payload: { enabled: boolean }) => void) => {
      const off = onChannel(DESKTOP_RENDERER_EVENT_CHANNELS.HudMicState, callback)

      ipcRenderer.send(DESKTOP_SEND_CHANNELS.VoiceMicStateRequest)
      return off
    },
    onBrainProactiveThought: (callback: (payload: { type: string; message: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.brainProactiveThought, callback),
    onChatError: (callback: (payload: { message: string; type?: string }) => void) =>
      onChannel(MIKA_EVENT_CHANNELS.chatError, callback),
  }
}
