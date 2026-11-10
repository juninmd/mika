import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { Cpu, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { getByPath } from '../../../renderer/src/components/settings/settings-utils'
import type { Translate } from '../../../renderer/src/hooks/useI18n'
import { useUIStore } from '../../../renderer/src/store/useUIStore'

const PATH = 'gamer-companion.customGames'

const ICON_BTN =
  'inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-surface-raise text-text-muted transition hover:border-danger-rim hover:bg-danger-soft hover:text-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'

function readGames(config: FeatureComponentProps['config']): Record<string, string> {
  const raw = getByPath(config, PATH)
  if (!raw || typeof raw !== 'object') return {}
  return Object.fromEntries(
    Object.entries(raw).filter((entry): entry is [string, string] => typeof entry[1] === 'string'),
  )
}

/**
 * "Jogos personalizados": process name → title, saved whole to `gamer-companion.customGames` and
 * matched BEFORE Steam/known games by `game_detection::find_custom_game` (Rust) on every watcher tick.
 */
export function GamerCustomGames({
  t,
  config,
  onSave,
}: {
  t: Translate
  config: FeatureComponentProps['config']
  onSave: FeatureComponentProps['onSave']
}) {
  const games = readGames(config)
  const [process, setProcess] = useState('')
  const [title, setTitle] = useState('')
  const entries = Object.entries(games)

  const save = async (next: Record<string, string>) => {
    try {
      await onSave(PATH, next)
    } catch (err) {
      useUIStore.getState().addNotification('error', toDisplayError(err, t('sdkUi.saveError')))
    }
  }

  const add = async () => {
    const key = process.trim()
    const name = title.trim()
    if (!key || !name) return
    await save({ ...games, [key]: name })
    setProcess('')
    setTitle('')
  }

  const remove = (key: string) => {
    const { [key]: _removed, ...rest } = games
    void save(rest)
  }

  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.gamerCompanion.customGames.title')}
        description={t('settings.gamerCompanion.customGames.description')}
        badge={String(entries.length)}
        className="mb-3"
      />
      {entries.length > 0 && (
        <ul className="mb-3 grid gap-2 md:grid-cols-2">
          {entries.map(([key, name]) => (
            <li
              key={key}
              className="flex items-center gap-3 rounded-xl border border-border-subtle bg-surface-raise/40 px-3 py-2"
            >
              <Cpu size={14} className="shrink-0 text-accent" aria-hidden="true" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-xs font-bold text-text-main">{name}</p>
                <p className="truncate font-mono text-caption text-text-muted">{key}</p>
              </div>
              <button
                type="button"
                onClick={() => remove(key)}
                aria-label={t('settings.gamerCompanion.customGames.remove').replace('{{title}}', name)}
                className={ICON_BTN}
              >
                <Trash2 size={13} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="grid items-end gap-3 sm:grid-cols-[1fr_1fr_auto]"
        onSubmit={(e) => {
          e.preventDefault()
          void add()
        }}
      >
        <AuroraField
          label={t('settings.gamerCompanion.customGames.processLabel')}
          value={process}
          onChange={setProcess}
          placeholder={t('settings.gamerCompanion.customGames.processPlaceholder')}
        />
        <AuroraField
          label={t('settings.gamerCompanion.customGames.titleLabel')}
          value={title}
          onChange={setTitle}
          placeholder={t('settings.gamerCompanion.customGames.titlePlaceholder')}
        />
        <button
          type="submit"
          disabled={!process.trim() || !title.trim()}
          className="inline-flex h-10 items-center justify-center gap-1.5 rounded-xl border border-accent-rim bg-accent-soft px-4 text-xs font-bold text-accent transition hover:bg-accent/20 active:scale-[0.98] disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <Plus size={14} aria-hidden="true" />
          {t('settings.gamerCompanion.customGames.add')}
        </button>
      </form>
    </GlassCard>
  )
}
