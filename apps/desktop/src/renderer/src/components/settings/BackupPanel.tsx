import {
  BentoGrid,
  ConfirmButton,
  GlassCard,
  GlassSection,
  GlassSelect,
  GlassToggle,
  PageHero,
  StatTile,
  StatusPill,
} from '@mika/sdk-ui/settings'
import { AlertTriangle, ArchiveRestore, Download, FileJson, type LucideIcon, Upload } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { rendererLog } from '../../lib/renderer-logger'
import { isSensitivePath } from '../../lib/sensitive-paths'
import { useUIStore } from '../../store/useUIStore'
import {
  type BackupFormat,
  buildBackupPayload,
  downloadBackup,
  flattenConfigFields,
  parseBackupText,
} from './backup-config'

const FORMATS: BackupFormat[] = ['toml', 'json']

function CardTitle({ icon: Icon, title, description }: { icon: LucideIcon; title: string; description: string }) {
  return (
    <div className="flex items-start gap-3">
      <span
        aria-hidden="true"
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent"
      >
        <Icon size={16} />
      </span>
      <div className="min-w-0">
        <h3 className="text-sm font-semibold text-text-main">{title}</h3>
        <p className="mt-0.5 text-xs leading-relaxed text-text-muted">{description}</p>
      </div>
    </div>
  )
}

interface BackupPanelProps {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
}

export function BackupPanel({ config, onSave }: BackupPanelProps) {
  const { t } = useI18n()
  const [includeSecrets, setIncludeSecrets] = useState(false)
  const [format, setFormat] = useState<BackupFormat>('toml')
  const [pendingFields, setPendingFields] = useState<Record<string, unknown> | null>(null)
  const [busy, setBusy] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleExport = () => {
    try {
      downloadBackup(buildBackupPayload(config, includeSecrets), format)
      useUIStore.getState().addNotification('success', t('settings.backup.export.success'))
    } catch (error) {
      rendererLog.error('export settings backup failed', { error })
      useUIStore.getState().addNotification('error', t('settings.backup.export.error'))
    }
  }

  const handleFilePicked = async (file: File) => {
    try {
      setPendingFields(parseBackupText(await file.text(), file.name))
    } catch (error) {
      rendererLog.error('parse settings backup failed', { error })
      const message = error instanceof Error ? error.message : t('settings.backup.import.readError')
      useUIStore.getState().addNotification('error', message)
    }
  }

  const applyImport = async () => {
    if (!pendingFields) return
    setBusy(true)
    // One refused field (a key an older backup carries and the schema dropped) must not strand the rest of the file.
    let applied = 0
    let failed = 0
    for (const [path, value] of Object.entries(pendingFields)) {
      try {
        await onSave(path, value)
        applied += 1
      } catch (error) {
        failed += 1
        rendererLog.warn('import settings backup: field refused', { path, error })
      }
    }
    const { addNotification } = useUIStore.getState()
    if (applied > 0) addNotification('success', t('settings.backup.import.success', { count: applied }))
    if (failed > 0) addNotification('error', t('settings.backup.import.error'))
    setBusy(false)
    setPendingFields(null)
  }

  const fieldStats = useMemo(() => {
    const all = flattenConfigFields(config)
    return { total: all.length, secrets: all.filter((f) => isSensitivePath(f.path)).length }
  }, [config])
  const exportCount = includeSecrets ? fieldStats.total : fieldStats.total - fieldStats.secrets

  return (
    <div className="space-y-6">
      <PageHero
        icon={ArchiveRestore}
        title={t('settings.backup.panel.title')}
        description={t('settings.backup.panel.description')}
        pills={
          <>
            <StatusPill variant="neutral" label={t('settings.backup.panel.badge')} />
            <StatusPill
              variant={includeSecrets ? 'warn' : 'ok'}
              label={includeSecrets ? t('settings.backup.hero.withSecrets') : t('settings.backup.hero.noSecrets')}
              dot
            />
          </>
        }
        aside={
          <>
            <StatTile label={t('settings.backup.hero.fields')} value={String(exportCount)} />
            <StatTile label={t('settings.backup.hero.secrets')} value={String(fieldStats.secrets)} />
          </>
        }
      />
      <GlassSection
        title={t('settings.backup.section.title')}
        icon={FileJson}
        description={t('settings.backup.section.description')}
      >
        <BentoGrid cols={2}>
          <GlassCard hover={false}>
            <div className="flex h-full flex-col gap-4">
              <CardTitle
                icon={Download}
                title={t('settings.backup.export.title')}
                description={t('settings.backup.export.description')}
              />
              <GlassSelect
                label={t('settings.backup.export.formatLabel')}
                description={t('settings.backup.export.formatDescription')}
                value={format}
                onChange={(value) => setFormat(value as BackupFormat)}
                variant="dropdown"
                options={FORMATS.map((value) => ({ value, label: t(`settings.backup.format.${value}`) }))}
              />
              <GlassToggle
                label={t('settings.backup.export.includeSecretsLabel')}
                description={t('settings.backup.export.includeSecretsDescription')}
                value={includeSecrets}
                onChange={setIncludeSecrets}
              />
              {includeSecrets && (
                <div
                  role="alert"
                  className="flex items-start gap-2 rounded-xl border border-warn-rim bg-warn-soft px-3 py-2 text-xs leading-relaxed text-warn"
                >
                  <AlertTriangle size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
                  <span>{t('settings.backup.export.secretsWarning')}</span>
                </div>
              )}
              <button
                type="button"
                onClick={handleExport}
                className="mt-auto inline-flex items-center justify-center gap-1.5 rounded-xl border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-bold text-accent-text transition hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-[0.98]"
              >
                <Download size={14} aria-hidden="true" />
                {t('settings.backup.export.button')}
              </button>
            </div>
          </GlassCard>

          <GlassCard hover={false}>
            <div className="flex h-full flex-col gap-4">
              <CardTitle
                icon={Upload}
                title={t('settings.backup.import.title')}
                description={t('settings.backup.import.description')}
              />
              {pendingFields ? (
                <div className="mt-auto flex items-center gap-2">
                  <ConfirmButton
                    label={t('settings.backup.import.apply', { count: Object.keys(pendingFields).length })}
                    ariaLabel={t('settings.backup.import.applyAriaLabel')}
                    icon={Upload}
                    onConfirm={() => void applyImport()}
                    disabled={busy}
                  />
                  <button
                    type="button"
                    onClick={() => setPendingFields(null)}
                    disabled={busy}
                    className="text-caption font-bold text-text-muted underline-offset-2 hover:underline disabled:opacity-50"
                  >
                    {t('button.cancel')}
                  </button>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  className="mt-auto inline-flex items-center justify-center gap-1.5 rounded-xl border border-border-subtle bg-surface-raise px-3 py-2 text-xs font-bold text-text-main transition hover:border-border-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-[0.98]"
                >
                  <Upload size={14} aria-hidden="true" />
                  {t('settings.backup.import.chooseFile')}
                </button>
              )}
              <input
                ref={fileInputRef}
                type="file"
                accept=".toml,.json,application/toml,application/json"
                hidden
                onChange={(e) => {
                  const file = e.target.files?.[0]
                  e.target.value = ''
                  if (file) void handleFilePicked(file)
                }}
              />
            </div>
          </GlassCard>
        </BentoGrid>
      </GlassSection>
    </div>
  )
}
