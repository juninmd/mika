import { Maximize } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useMiniMode } from '../../hooks/useMiniMode'
import { getElectronApi } from '../../lib/shell-api'
import { subscribeFullscreen } from '../../lib/window-fullscreen'
import { GlassSection, GlassToggle, StatusPill } from './ds'

const STATE_LABEL = {
  windowed: 'settings.window.state.windowed',
  fullscreen: 'settings.window.state.fullscreen',
  mini: 'settings.window.state.mini',
} as const

/** Tela cheia e modo mini da janela do avatar: cada interruptor, o atalho (F11), o menu e a paleta chamam o mesmo canal do Rust. */
export function WindowModeCard() {
  const { t } = useI18n()
  const [fullscreen, setFullscreen] = useState<boolean | null>(null)
  const mini = useMiniMode()
  const windowApi = getElectronApi().shell.window
  const state = mini ? 'mini' : fullscreen ? 'fullscreen' : 'windowed'

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
        <GlassToggle
          label={t('settings.window.mini.label')}
          description={t('settings.window.mini.hint')}
          value={mini === true}
          disabled={!windowApi?.setMiniMode}
          onChange={() => void windowApi?.setMiniMode?.(!mini)}
        />
        <div role="status" aria-live="polite" className="flex flex-wrap items-center gap-2">
          <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.window.stateLabel')}
          </span>
          {fullscreen === null || mini === null ? (
            <StatusPill variant="loading" />
          ) : (
            <StatusPill variant={state === 'windowed' ? 'neutral' : 'accent'} label={t(STATE_LABEL[state])} dot />
          )}
        </div>
      </div>
    </GlassSection>
  )
}
