import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassToggle } from '@mika/sdk-ui/settings'
import { ShieldAlert } from 'lucide-react'
import { useEffect, useState } from 'react'
import { getElectronApi } from '../../src/lib/shell-api'

type T = (key: keyof TranslationDict) => string

/** The privacy keys that actually gate capture in Rust (`vision/screen_capture.rs`, `vision/clipboard_watch.rs`). */
export type VisionPrivacy = {
  screenCapture: boolean
  clipboardLogging: boolean
  readClipboardImages: boolean
}

export function visionBlockReason(p: VisionPrivacy): keyof TranslationDict | null {
  if (!p.screenCapture) return 'settings.vision.readiness.screenBlocked'
  if (p.readClipboardImages && !p.clipboardLogging) return 'settings.vision.readiness.clipboardBlocked'
  return null
}

/**
 * "Ligado" in Vision does not mean "capturing": `privacy.screenCapture` (default off) is the real
 * gate. Without this banner the page read ATIVO while the monitor sat on "Nenhum sinal" forever.
 */
export function VisionReadinessBanner({
  t,
  privacy,
  onSave,
}: {
  t: T
  privacy: VisionPrivacy
  onSave: (path: string, value: boolean) => Promise<void>
}) {
  const reason = visionBlockReason(privacy)
  if (!reason) return null
  return (
    <div
      role="alert"
      className="mt-3 flex flex-col gap-3 rounded-xl border border-warn-rim bg-warn-soft p-3 sm:flex-row sm:items-center"
    >
      <div className="flex min-w-0 flex-1 items-start gap-2.5">
        <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-warn-rim bg-bg-surface text-warn">
          <ShieldAlert size={15} aria-hidden="true" />
        </span>
        <div className="min-w-0">
          <p className="text-caption font-black uppercase tracking-[0.12em] text-warn">
            {t('settings.vision.readiness.title')}
          </p>
          <p className="mt-0.5 text-caption leading-snug text-text-main">{t(reason)}</p>
        </div>
      </div>
      <div className="flex shrink-0 flex-col gap-2 sm:w-64">
        {!privacy.screenCapture && (
          <GlassToggle
            label={t('settings.vision.readiness.allowScreen')}
            configPath="privacy.screenCapture"
            value={privacy.screenCapture}
            onSave={onSave}
          />
        )}
        {privacy.readClipboardImages && !privacy.clipboardLogging && (
          <GlassToggle
            label={t('settings.vision.readiness.allowClipboard')}
            configPath="privacy.clipboardLogging"
            value={privacy.clipboardLogging}
            onSave={onSave}
          />
        )}
      </div>
    </div>
  )
}

/**
 * Sem sonda de janela em foco (Linux/Wayland) a captura PERIÓDICA fica desligada no Rust
 * (`vision/watcher.rs`): a blocklist de app em foco seria cega. Lê a mesma sonda que a tool `habits`
 * já expõe (`platformSupported`) para o aviso não depender de o usuário ler o log.
 */
export function VisionNoWindowProbeNotice({ t }: { t: T }) {
  const [unsupported, setUnsupported] = useState(false)
  useEffect(() => {
    let alive = true
    const api = getElectronApi()
    void Promise.resolve(api?.runtime?.executeTool?.('habits', { action: 'get' }))
      .then((result) => {
        const data = (result as { data?: { platformSupported?: boolean } } | undefined)?.data
        if (alive) setUnsupported(data?.platformSupported === false)
      })
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [])
  if (!unsupported) return null
  return (
    <p role="status" className="mt-3 text-caption leading-snug text-warn">
      {t('settings.vision.readiness.noWindowProbe')}
    </p>
  )
}
