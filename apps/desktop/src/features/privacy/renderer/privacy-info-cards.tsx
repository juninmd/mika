import { GlassCard } from '@mika/sdk-ui/settings'
import { FolderOpen } from 'lucide-react'
import { useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { revealAppFolder } from '../../../renderer/src/lib/shell-links'

// `app.path().app_data_dir()` do Tauri = pasta de dados do SO + `identifier` (`tauri.conf.json`).
// O texto antigo mostrava o caminho do Electron no Windows (`%APPDATA%\Mika\desktop`) em todo SO.
const DATA_DIR_BY_PLATFORM: Record<string, string> = {
  win32: '%APPDATA%\\com.mika.desktop',
  darwin: '~/Library/Application Support/com.mika.desktop',
  linux: '~/.local/share/com.mika.desktop',
}

function dataDirFor(platform: string): string {
  return DATA_DIR_BY_PLATFORM[platform] ?? DATA_DIR_BY_PLATFORM.linux ?? ''
}

export function PrivacyDataLocationCard() {
  const { t } = useI18n()
  // The real path replaces the per-OS template once Rust has resolved it (profile override included).
  const [resolved, setResolved] = useState<string | null>(null)
  const dataDir = resolved ?? dataDirFor(getElectronApi().platform)
  const open = async () => {
    const path = await revealAppFolder('data', t('settings.privacy.dataFolder.error'))
    if (path) setResolved(path)
  }
  return (
    <GlassCard span="full">
      <div className="flex flex-wrap items-start gap-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-border-subtle bg-surface-raise text-text-muted">
          <FolderOpen size={18} aria-hidden="true" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-micro font-black uppercase tracking-[0.14em] text-text-muted">
            {t('settings.privacy.panel.dataLocationTitle')}
          </p>
          <h4 className="mt-0.5 text-sm font-bold text-text-main">{t('settings.privacy.panel.dataLocationHeading')}</h4>
          <p className="mt-1 text-xs leading-relaxed text-text-muted">
            {t('settings.privacy.panel.dataLocationText1')}{' '}
            <code className="break-all rounded-md border border-border-subtle bg-surface-raise px-1.5 py-0.5 font-mono text-caption text-text-main">
              {dataDir}
            </code>
            .
          </p>
          <p className="mt-1 text-xs leading-relaxed text-text-muted">
            {t('settings.privacy.panel.dataLocationDeleteHint')}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void open()}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2 text-xs font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <FolderOpen size={13} aria-hidden="true" />
          {t('settings.privacy.dataFolder.open')}
        </button>
      </div>
    </GlassCard>
  )
}
