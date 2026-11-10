import { Maximize } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import { subscribeFullscreen } from '../../lib/window-fullscreen'
import { GlassSection, GlassToggle, StatusPill } from './ds'

/** Tela cheia da janela do avatar: o interruptor, o atalho (F11) e a paleta chamam o mesmo canal do Rust. */
export function WindowModeCard() {
  const { t } = useI18n()
  const [fullscreen, setFullscreen] = useState<boolean | null>(null)
  const windowApi = getElectronApi().shell.window

  useEffect(() => (windowApi ? subscribeFullscreen(windowApi, setFullscreen) : undefined), [windowApi])

  return (
    <GlassSection title={t('settings.window.title')} icon={Maximize} description={t('settings.window.description')}>
      <div className="space-y-4">
        <GlassToggle
          label={t('settings.window.fullscreen.label')}
          description={t('settings.window.fullscreen.hint')}
          value={fullscreen === true}
          disabled={!windowApi?.toggleFullscreen}
          onChange={() => void windowApi?.toggleFullscreen?.()}
        />
        <div role="status" aria-live="polite" className="flex flex-wrap items-center gap-2">
          <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.window.stateLabel')}
          </span>
          {fullscreen === null ? (
            <StatusPill variant="loading" />
          ) : (
            <StatusPill
              variant={fullscreen ? 'accent' : 'neutral'}
              label={t(fullscreen ? 'settings.window.state.fullscreen' : 'settings.window.state.windowed')}
              dot
            />
          )}
        </div>
      </div>
    </GlassSection>
  )
}
