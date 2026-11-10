import type { PowerProfileState } from '@mika/sdk/contracts'
import type { EffectCeiling } from './effect-quality'
import { getElectronApi } from './shell-api'

/** Energia vista pelo renderer: bateria e o orçamento do governador (Rust `modules/system/governor`). */
export type PowerSnapshot = { onBattery: boolean; ceiling: EffectCeiling; profile: PowerProfileState | null }

let snapshot: PowerSnapshot = { onBattery: false, ceiling: 'full', profile: null }
const subscribers = new Set<(value: PowerSnapshot) => void>()
let unsubscribeAll: (() => void) | null = null

function publish(next: Partial<PowerSnapshot>): void {
  snapshot = { ...snapshot, ...next }
  for (const notify of subscribers) notify(snapshot)
}

function applyProfile(profile: PowerProfileState): void {
  publish({ profile, ceiling: profile.ceiling })
}

function connect(): () => void {
  const api = getElectronApi().runtime
  void api
    .getPowerState?.()
    .then((state) => {
      publish({ onBattery: state.onBatteryPower === true })
      // A `power-profile` event that already landed is newer than this one-shot read.
      if (!snapshot.profile && state.profile && state.state && state.ceiling) applyProfile(state as PowerProfileState)
    })
    .catch(() => {
      // Denied/unavailable (plugin windows): the events below still drive the snapshot.
    })
  const offMode = api.onPowerModeChanged?.((payload) => publish({ onBattery: payload.onBatteryPower }))
  const offProfile = api.onPowerProfile?.(applyProfile)
  return () => {
    offMode?.()
    offProfile?.()
  }
}

export function getPowerSnapshot(): PowerSnapshot {
  return snapshot
}

/** Uma assinatura IPC por janela, compartilhada por avatar, efeitos e a leitura da aba Resumo. */
export function subscribePower(notify: (value: PowerSnapshot) => void): () => void {
  subscribers.add(notify)
  if (subscribers.size === 1) {
    try {
      unsubscribeAll = connect()
    } catch {
      unsubscribeAll = null
    }
  }
  return () => {
    subscribers.delete(notify)
    if (subscribers.size === 0) {
      unsubscribeAll?.()
      unsubscribeAll = null
    }
  }
}
