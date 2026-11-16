import { Maximize } from 'lucide-react'
import { useEffect, useState } from 'react'
import { saveConfigFlag, useConfigFlag } from '../../hooks/useConfigFlag'
import { useI18n } from '../../hooks/useI18n'
import { setMiniModeStyle, useMiniMode, useMiniModeStyle } from '../../hooks/useMiniMode'
import { isDockStyle, parseMiniModeStyle } from '../../lib/mini-mode'
import { getElectronApi } from '../../lib/shell-api'
import { subscribeFullscreen } from '../../lib/window-fullscreen'
import { GlassSection, GlassSelect, GlassToggle, StatusPill } from './ds'

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
  const style = useMiniModeStyle()
  const dockAutoHide = useConfigFlag('app.dockAutoHide', true)
  const dockDiscreet = useConfigFlag('app.dockDiscreet', true)
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
        <GlassSelect
          label={t('settings.window.style.label')}
          description={t('settings.window.style.hint')}
          value={style}
          onChange={(next) => setMiniModeStyle(parseMiniModeStyle(next))}
          variant="dropdown"
          options={[
            { value: 'head', label: t('settings.window.style.head') },
            { value: 'dock-right', label: t('settings.window.style.dockRight') },
            { value: 'dock-left', label: t('settings.window.style.dockLeft') },
          ]}
        />
        {isDockStyle(style) && (
          <>
            <GlassToggle
              label={t('settings.window.dockAutoHide.label')}
              description={t('settings.window.dockAutoHide.hint')}
              value={dockAutoHide}
              onChange={() => saveConfigFlag('app.dockAutoHide', !dockAutoHide)}
            />
            <GlassToggle
              label={t('settings.window.dockDiscreet.label')}
              description={t('settings.window.dockDiscreet.hint')}
              value={dockDiscreet}
              onChange={() => saveConfigFlag('app.dockDiscreet', !dockDiscreet)}
            />
          </>
        )}
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
