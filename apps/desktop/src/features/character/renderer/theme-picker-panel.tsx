import { toDisplayError } from '@mika/sdk-ui'
import { GlassCard, GlassSection } from '@mika/sdk-ui/settings'
import { Palette } from 'lucide-react'
import { useCallback, useState } from 'react'
import { configStr } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { applyThemeById } from '../../../renderer/src/lib/apply-theme'
import { useUIStore } from '../../../renderer/src/store/useUIStore'
import { THEMES } from '../../../renderer/src/themes'

type ThemePickerPanelProps = {
  config?: Record<string, unknown>
  theme?: string
  onSave: (path: string, value: unknown) => Promise<void>
}

export function ThemePickerPanel(props: ThemePickerPanelProps) {
  const { t } = useI18n()
  const { onSave } = props
  const theme = props.theme ?? configStr(props.config ?? {}, 'character.theme', 'dark')
  const [saving, setSaving] = useState<string | null>(null)
  const [preview, setPreview] = useState<string | null>(null)
  const displayTheme = preview ?? theme

  const doSave = useCallback(
    async (path: string, value: unknown, previousValue?: unknown) => {
      setSaving(path)
      if (path === 'character.theme') applyThemeById(value as string)
      try {
        await onSave(path, value)
      } catch (err) {
        if (path === 'character.theme' && typeof previousValue === 'string') applyThemeById(previousValue)
        useUIStore
          .getState()
          .addNotification('error', toDisplayError(err, t('settings.character.themePanel.saveError')))
      } finally {
        setSaving(null)
        if (path === 'character.theme') setPreview(null)
      }
    },
    [onSave, t],
  )

  return (
    <GlassSection
      title={t('settings.character.field.theme.label')}
      icon={Palette}
      badge={t('settings.character.themePanel.badge')}
    >
      <div className="grid gap-3 sm:grid-cols-5">
        {THEMES.map((t) => {
          const active = displayTheme === t.id
          const isSaving = saving === 'character.theme' && preview === t.id
          return (
            <GlassCard key={t.id} accent={active} accentHue={t.id === 'light' ? 45 : 270}>
              <button
                type="button"
                aria-pressed={active}
                disabled={saving !== null}
                onClick={() => {
                  const previousTheme = displayTheme
                  setPreview(t.id)
                  applyThemeById(t.id)
                  doSave('character.theme', t.id, previousTheme)
                }}
                className={(() => {
                  const baseClass =
                    'w-full text-center transition duration-200 focus-visible:ring-2 focus-visible:ring-accent focus-visible:outline-none rounded-lg p-1'
                  return isSaving ? `${baseClass} animate-pulse` : baseClass
                })()}
              >
                <div className="flex gap-1 justify-center mb-2">
                  {(
                    [
                      ['background', t.colors.background],
                      ['primary', t.colors.primary],
                      ['accent', t.colors.accent],
                    ] as const
                  ).map(([role, color]) => (
                    <div key={role} className="h-8 w-6 rounded-md" style={{ backgroundColor: color }} />
                  ))}
                </div>
                <div className={`text-caption font-bold ${active ? 'text-accent' : 'text-text-muted'}`}>{t.name}</div>
                {active && (
                  <div
                    aria-hidden="true"
                    className="absolute -right-1 -top-1 h-3 w-3 rounded-full bg-accent shadow-[0_0_8px_color-mix(in_srgb,var(--accent)_60%,transparent)]"
                  />
                )}
              </button>
            </GlassCard>
          )
        })}
      </div>
    </GlassSection>
  )
}
