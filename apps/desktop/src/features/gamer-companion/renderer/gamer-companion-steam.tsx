import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, GlassCard, SectionDivider, SectionHeader } from '@mika/sdk-ui/settings'
import { CircleHelp, ExternalLink, FolderSearch } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { callPluginRpc } from '../../../renderer/src/lib/plugin-rpc'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { openExternalLink } from '../../../renderer/src/lib/shell-links'
import { useUIStore } from '../../../renderer/src/store/useUIStore'

// Where Steam installs by default on each OS — the same candidates `resolve_steam_root` probes.
const STEAM_PATH_EXAMPLE: Record<string, string> = {
  win32: 'C:\\Program Files (x86)\\Steam',
  darwin: '~/Library/Application Support/Steam',
  linux: '~/.steam/steam',
}
const STEAM_ID_HELP_URL = 'https://help.steampowered.com/en/faqs/view/2816-BE67-5B69-0FEC'

type SteamRoot = { resolved: string | null; detected: string | null }

function useSteamRoot(steamPath: unknown): SteamRoot | null {
  const [root, setRoot] = useState<SteamRoot | null>(null)
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-resolve when the configured folder changes
  useEffect(() => {
    let alive = true
    callPluginRpc<SteamRoot>(getElectronApi(), 'mika.gamer-companion', 'steam:root')
      .then((next) => alive && setRoot(next))
      .catch(() => alive && setRoot(null))
    return () => {
      alive = false
    }
  }, [steamPath])
  return root
}

const OPEN_VIEW_CLS =
  'flex w-full items-center justify-center gap-2 rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-caption font-semibold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'

async function openView(viewId: string, errorMessage: string): Promise<void> {
  try {
    await getElectronApi().shell.pluginViews?.open({ pluginId: 'mika.gamer-companion', viewId })
  } catch (err) {
    useUIStore.getState().addNotification('error', toDisplayError(err, errorMessage))
  }
}

export function GamerCompanionSteam({
  t,
  config,
  onSave,
}: {
  t: Translate
  config: FeatureComponentProps['config']
  onSave: FeatureComponentProps['onSave']
}) {
  const platform = getElectronApi().platform
  const root = useSteamRoot((config['gamer-companion'] as Record<string, unknown> | undefined)?.steamPath)
  return (
    <GlassCard span="full">
      <SectionHeader title={t('settings.gamerCompanion.panel.sectionSteam')} className="mb-3" />
      <div className="grid gap-4 sm:grid-cols-2">
        <AuroraField
          label={t('settings.gamerCompanion.panel.steamIdLabel')}
          description={t('settings.gamerCompanion.panel.steamIdDescription')}
          configPath="gamer-companion.steamId64"
          config={config}
          onSave={onSave}
          placeholder="76561198XXXXXXXXX"
        />
        <AuroraField
          label={t('settings.gamerCompanion.panel.steamPathLabel')}
          description={t('settings.gamerCompanion.panel.steamPathDescription')}
          configPath="gamer-companion.steamPath"
          config={config}
          onSave={onSave}
          placeholder={STEAM_PATH_EXAMPLE[platform] ?? STEAM_PATH_EXAMPLE.linux}
        />
      </div>
      <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
        <p className="flex min-w-0 items-center gap-1.5 text-caption text-text-muted">
          <FolderSearch size={13} className={root?.resolved ? 'shrink-0 text-ok' : 'shrink-0'} aria-hidden="true" />
          <span className="min-w-0 truncate" title={root?.resolved ?? undefined}>
            {root?.resolved
              ? t('settings.gamerCompanion.steam.inUse').replace('{{path}}', root.resolved)
              : root?.detected
                ? t('settings.gamerCompanion.steam.detected').replace('{{path}}', root.detected)
                : t('settings.gamerCompanion.steam.notDetected')}
          </span>
        </p>
        <button
          type="button"
          onClick={() => void openExternalLink(STEAM_ID_HELP_URL, t('settings.providerCredentials.getKeyError'))}
          className="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-2 py-1 text-caption font-semibold text-info transition hover:bg-info-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <CircleHelp size={13} aria-hidden="true" />
          {t('settings.gamerCompanion.steam.idHelp')}
        </button>
      </div>
      <SectionDivider label={t('settings.gamerCompanion.panel.sectionWindows')} />
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <button
          type="button"
          onClick={() => void openView('gamer-companion.hud', t('settings.gamerCompanion.panel.openViewError'))}
          className={OPEN_VIEW_CLS}
        >
          <ExternalLink className="h-3 w-3" aria-hidden="true" />
          {t('settings.gamerCompanion.panel.openHud')}
        </button>
        <button
          type="button"
          onClick={() =>
            void openView('gamer-companion.steam-library', t('settings.gamerCompanion.panel.openViewError'))
          }
          className={OPEN_VIEW_CLS}
        >
          <ExternalLink className="h-3 w-3" aria-hidden="true" />
          {t('settings.gamerCompanion.panel.openSteamLibrary')}
        </button>
      </div>
    </GlassCard>
  )
}
