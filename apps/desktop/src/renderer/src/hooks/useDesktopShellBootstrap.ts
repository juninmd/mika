import { toErrorMessage } from '@mika/sdk/utils/object'
import type { Dispatch, SetStateAction } from 'react'
import { useEffect, useRef } from 'react'
import { DesktopHealthStatus } from '../../../shared/desktop-health-status'
import type { HealthCapabilities } from '../components/app/app-shell.types'
import { DEFAULT_CAPABILITIES } from '../components/app/app-shell.types'
import { resolveLive2DModelPath } from '../components/live2d/model-path'
import { rendererLog } from '../lib/renderer-logger'
import { isSensitivePath } from '../lib/sensitive-paths'
import { getElectronApi } from '../lib/shell-api'
import { useChatStore } from '../store/useChatStore'
import { useTrackingStore } from '../store/useTrackingStore'

const HEALTH_RETRY_MS = 2_000
const HEALTH_FAST_RETRIES = 30
const BACKEND_UP: ReadonlySet<DesktopHealthStatus> = new Set([DesktopHealthStatus.Ok, DesktopHealthStatus.Degraded])

function resolveBackgroundSolidColor(raw: unknown): string {
  return typeof raw === 'string' && /^#(?:[0-9a-fA-F]{3}){1,2}$/.test(raw) ? raw : '#050914'
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? (value as Record<string, unknown>) : null
}

type UseDesktopShellBootstrapProps = {
  setBackgroundMode: Dispatch<SetStateAction<'transparent' | 'semi-transparent' | 'solid'>>
  setBackgroundSolidColor: Dispatch<SetStateAction<string>>
  setCapabilities: Dispatch<SetStateAction<HealthCapabilities>>
  setLive2dModelPath: Dispatch<SetStateAction<string>>
  setMikaReady: Dispatch<SetStateAction<boolean>>
}

export function useDesktopShellBootstrap({
  setBackgroundMode,
  setBackgroundSolidColor,
  setCapabilities,
  setLive2dModelPath,
  setMikaReady,
}: UseDesktopShellBootstrapProps) {
  const setMessages = useChatStore((s) => s.setMessages)
  const setMouseTracking = useTrackingStore((s) => s.setMouseTracking)
  const lastCapabilitiesRef = useRef('')

  useEffect(() => {
    const startedAt = performance.now()
    let eAPI: ReturnType<typeof getElectronApi>
    try {
      eAPI = getElectronApi()
    } catch (err) {
      rendererLog.warn('[Shell] electronAPI unavailable during bootstrap', { error: toErrorMessage(err) })
      setMikaReady(false)
      setCapabilities(DEFAULT_CAPABILITIES)
      return
    }
    let isMounted = true

    const refreshModelPathFromConfig = () => {
      void eAPI.runtime
        .getSettingsConfig()
        .then(({ config }) => {
          if (!isMounted) return
          const char = (config.character as Record<string, unknown>) || {}
          const next = resolveLive2DModelPath({
            modelName: String(char.modelName || 'default'),
            modelPath: String(char.modelPath || char.live2dModelPath || ''),
          })
          setLive2dModelPath(next)
        })
        .catch((err) => rendererLog.warn('[Shell] failed to refresh character config', { error: toErrorMessage(err) }))
    }

    const configChangeHandlers: Record<string, (value: unknown) => void> = {
      'character.modelName': refreshModelPathFromConfig,
      'character.modelPath': refreshModelPathFromConfig,
      'character.backgroundMode': (value) =>
        setBackgroundMode((value as 'transparent' | 'semi-transparent' | 'solid') || 'transparent'),
      'character.backgroundSolidColor': (value) => setBackgroundSolidColor(resolveBackgroundSolidColor(value)),
      'voice.mouseTracking': (value) => setMouseTracking(value !== false),
    }

    const unsubscribe = eAPI.runtime.onConfigChanged((change) => {
      rendererLog.info('Config change', {
        path: change.path,
        value: isSensitivePath(change.path) ? '<REDACTED>' : change.value,
      })
      configChangeHandlers[change.path]?.(change.value)
    })

    // Resolves whether the backend answered as up. `degraded` = some feature failed to register, but the core
    // answers: the BootHealthBanner reports it, the shell must not sit on "connecting" forever (the Rust side
    // reports degraded over ok once any feature failed).
    const pingHealth = async (): Promise<boolean> => {
      try {
        const health = await eAPI.runtime.getHealth()
        if (!isMounted) return true
        const ready = BACKEND_UP.has(health.status)
        setMikaReady(ready)
        const next = { ...health.capabilities, tools: health.tools }
        const serialized = JSON.stringify(next)
        if (lastCapabilitiesRef.current === serialized) return ready
        lastCapabilitiesRef.current = serialized
        setCapabilities(next)
        return ready
      } catch (err) {
        if (!isMounted) return true
        rendererLog.warn('Health ping failed', { error: toErrorMessage(err) })
        setMikaReady(false)
        lastCapabilitiesRef.current = ''
        setCapabilities((prev) => (Object.is(prev, DEFAULT_CAPABILITIES) ? prev : DEFAULT_CAPABILITIES))
        return false
      }
    }

    // The window loads hidden, before the backend reveals the avatar: the first answer is usually `starting`.
    // Re-asking only every minute left the "connecting" state on screen for up to 60 s after the reveal.
    let retryTimer: ReturnType<typeof setTimeout> | undefined
    let fastRetries = 0
    const pingUntilReady = async () => {
      clearTimeout(retryTimer)
      if (await pingHealth()) {
        fastRetries = 0
        return
      }
      // Bounded: a backend that keeps failing falls back to the minute cadence instead of logging every 2 s.
      if (fastRetries++ < HEALTH_FAST_RETRIES) retryTimer = setTimeout(() => void pingUntilReady(), HEALTH_RETRY_MS)
    }

    const hydrateChatHistory = (
      historyResult: PromiseSettledResult<Awaited<ReturnType<typeof eAPI.runtime.getChatHistory>>>,
    ) => {
      if (historyResult.status !== 'fulfilled') {
        rendererLog.warn('[Shell] failed to hydrate chat history', { error: toErrorMessage(historyResult.reason) })
        return
      }
      if (!isMounted) return
      const historyMapped = historyResult.value.messages.map((message) => ({
        ...message,
        role: message.role as 'user' | 'assistant',
      }))
      // Mescla em vez de substituir: se uma mensagem já chegou pelo composer/voz/evento
      // enquanto este fetch estava em voo, ela não pode ser apagada por uma resposta obsoleta.
      setMessages((prev) => {
        if (prev.length === 0) return historyMapped
        const historyIds = new Set(historyMapped.map((m) => m.id))
        const appendedSinceFetch = prev.filter((m) => !historyIds.has(m.id))
        return [...historyMapped, ...appendedSinceFetch]
      })
    }

    const hydrateCharacterConfig = (character: Record<string, unknown>) => {
      const next = resolveLive2DModelPath({
        modelName: String(character.modelName || 'default'),
        modelPath: String(character.modelPath || character.live2dModelPath || ''),
      })
      setLive2dModelPath(next)
      setBackgroundMode((character.backgroundMode as 'transparent' | 'semi-transparent' | 'solid') || 'transparent')
      setBackgroundSolidColor(resolveBackgroundSolidColor(character.backgroundSolidColor))
    }

    const hydrateSettingsConfig = (
      configResult: PromiseSettledResult<Awaited<ReturnType<typeof eAPI.runtime.getSettingsConfig>>>,
    ) => {
      if (configResult.status !== 'fulfilled') {
        rendererLog.warn('[Shell] failed to hydrate config', { error: toErrorMessage(configResult.reason) })
        return
      }
      if (!isMounted) return
      const { config } = configResult.value
      const character = asRecord(config.character)
      const voice = asRecord(config.voice)
      if (character) hydrateCharacterConfig(character)
      if (voice) setMouseTracking(voice.mouseTracking !== false)
    }

    const bootstrapData = async () => {
      const [historyResult, configResult] = await Promise.allSettled([
        eAPI.runtime.getChatHistory(),
        eAPI.runtime.getSettingsConfig(),
      ])

      hydrateChatHistory(historyResult)
      hydrateSettingsConfig(configResult)
    }

    void pingUntilReady()
    void bootstrapData().finally(() => {
      const elapsedMs = Math.round(performance.now() - startedAt)
      rendererLog.info(`Bootstrap renderer concluído {mode: "default", elapsedMs: ${elapsedMs}}`)
    })

    const healthTimer = setInterval(() => {
      void pingUntilReady()
    }, 60000)

    return () => {
      isMounted = false
      clearTimeout(retryTimer)
      clearInterval(healthTimer)
      unsubscribe()
    }
  }, [
    setBackgroundMode,
    setBackgroundSolidColor,
    setCapabilities,
    setLive2dModelPath,
    setMouseTracking,
    setMessages,
    setMikaReady,
  ])
}
