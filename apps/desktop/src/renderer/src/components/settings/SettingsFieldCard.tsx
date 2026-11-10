import type { SettingsField } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { memo, useCallback, useEffect, useId, useRef, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { Collapsible, GlassBox, GlassToggle, StatusPill } from './ds'
import { pendingCommits } from './pending-settings-commits'
import { SettingsFieldInput } from './SettingsFieldInput'
import { FieldExamples, FieldRangeAndLink, FieldSaveButton } from './SettingsFieldParts'
import { formatValue, parseValue, validateField } from './settings-utils'

export { flushPendingSettingsCommits } from './pending-settings-commits'

type SettingsFieldCardProps = {
  field: SettingsField
  value: unknown
  onSave: (path: string, value: unknown) => Promise<void>
  compact?: boolean
}

export const SettingsFieldCard = memo(function SettingsFieldCard({
  field,
  value,
  onSave,
  compact,
}: SettingsFieldCardProps) {
  const { t } = useI18n()
  const ERR_MSG = t('settings.fields.saveErrorFallback')
  const [draft, setDraft] = useState(formatValue(field, value))
  const [savedDraft, setSavedDraft] = useState(formatValue(field, value))
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)
  const commitRequestRef = useRef(0)
  const fieldId = useId()
  const descriptionId = `${fieldId}-description`
  const errorId = `${fieldId}-error`

  useEffect(() => {
    const f = formatValue(field, value)
    setDraft(f)
    setSavedDraft(f)
  }, [field, value])

  useEffect(() => {
    if (!error) return
    const t = setTimeout(() => setError(null), 5000)
    return () => clearTimeout(t)
  }, [error])

  const isDirty = draft !== savedDraft

  const commit = useCallback(
    async (next = draft) => {
      // Slider comita no blur e no soltar: sem esta guarda, passar o Tab por ele regravava o mesmo valor.
      if (saving || next === savedDraft) return
      const parsed = parseValue(field, next, value)
      const v = validateField(field, parsed)
      if (v) {
        setError(v)
        return
      }
      const requestId = ++commitRequestRef.current
      setSaving(true)
      setError(null)
      let saveError: string | null = null
      try {
        await onSave(field.configPath, parsed)
      } catch (e) {
        saveError = toDisplayError(e, ERR_MSG)
      }
      if (commitRequestRef.current !== requestId) return
      setSaving(false)
      if (saveError) {
        setError(saveError)
        return
      }
      setSavedDraft(next)
      setSaved(true)
      setTimeout(() => setSaved(false), 1500)
    },
    [draft, field, value, onSave, saving, savedDraft, ERR_MSG],
  )

  const handleBlur = useCallback(() => {
    if (isDirty) void commit()
  }, [isDirty, commit])

  useEffect(() => {
    if (!isDirty) {
      pendingCommits.delete(field.configPath)
      return
    }
    pendingCommits.set(field.configPath, () => commit())
    return () => {
      pendingCommits.delete(field.configPath)
    }
  }, [isDirty, commit, field.configPath])

  if (field.type === 'boolean') {
    return (
      <GlassToggle
        label={field.label}
        // `compact` é o interruptor do cabeçalho do módulo: o título e o "Ligado/Desligado" ao lado já
        // dizem o nome, então só o `aria-label` o carrega (antes ia vazio: switch sem nome no leitor de tela).
        hideLabel={compact}
        description={
          field.description ?? t('settings.fields.toggleDescription').replace('{{label}}', field.label.toLowerCase())
        }
        badge={field.badge}
        configPath={field.configPath}
        value={Boolean(value)}
        onSave={async (path, v) => {
          await onSave(path, v)
        }}
      />
    )
  }

  const examples = field.examples?.length ? field.examples : field.example ? [field.example] : []
  const purpose =
    field.description ?? t('settings.fields.defaultDescription').replace('{{label}}', field.label.toLowerCase())

  return (
    <GlassBox variant="compact" hover as="article">
      <div className="flex flex-wrap items-start justify-between gap-3 mb-1">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span title={field.description} className="text-sm font-medium text-text-main">
              {field.label}
            </span>
            {field.badge && <StatusPill variant="accent" label={field.badge} />}
            {field.unit && <StatusPill variant="neutral" label={field.unit} />}
          </div>
          {isDirty && (
            <span role="status" className="mt-0.5 inline-flex items-center gap-1 text-caption text-warn">
              <span className="h-1 w-1 rounded-full bg-warn animate-pulse" aria-hidden="true" />
              {t('settings.fields.unsaved')}
            </span>
          )}
        </div>
      </div>
      <p id={descriptionId} className="text-caption text-text-muted leading-relaxed mb-2 mt-0.5">
        {purpose}
      </p>

      <div className="space-y-2">
        <div className="flex items-end gap-2">
          <div className="min-w-0 flex-1">
            <SettingsFieldInput
              field={field}
              draft={draft}
              describedBy={`${descriptionId}${error ? ` ${errorId}` : ''}`}
              onDraftChange={setDraft}
              onSubmit={(next) => void commit(next)}
              onCommit={handleBlur}
              disabled={saving}
            />
          </div>
          <FieldSaveButton
            label={field.label}
            saving={saving}
            saved={saved}
            isDirty={isDirty}
            onCommit={() => void commit()}
          />
        </div>

        {(field.helperText || field.pattern) && (
          <p className="text-caption font-medium text-text-subtle pl-1 italic">
            {field.helperText ?? t('settings.fields.patternDefault').replace('{{pattern}}', String(field.pattern))}
          </p>
        )}

        <FieldExamples
          examples={examples}
          configPath={field.configPath}
          saving={saving}
          onPick={(ex) => {
            setDraft(ex)
            void commit(ex)
          }}
        />

        <FieldRangeAndLink field={field} />
        <Collapsible open={!!error}>
          {error && (
            <div
              id={errorId}
              role="alert"
              className="rounded-lg border border-danger-rim bg-danger-soft px-3.5 py-3 text-xs font-bold text-danger mt-2"
            >
              {error}
            </div>
          )}
        </Collapsible>
      </div>
    </GlassBox>
  )
})
