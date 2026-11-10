import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, Collapsible, ConfirmButton, GlassCard, StatusPill } from '@mika/sdk-ui/settings'
import { RotateCcw, Save } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useUIStore } from '../../store/useUIStore'

export type PromptSummary = {
  namespace: string
  key: string
  defaultText: string
  overrideText: string | null
  placeholders: string[]
}

function placeholderChips(placeholders: string[]) {
  if (placeholders.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1">
      {placeholders.map((p) => (
        <span
          key={p}
          className="rounded border border-border-subtle bg-surface-raise px-1.5 py-0.5 font-mono text-caption text-text-muted"
        >
          {`{{${p}}}`}
        </span>
      ))}
    </div>
  )
}

function droppedPlaceholders(defaultPlaceholders: string[], text: string): string[] {
  return defaultPlaceholders.filter((p) => !text.includes(`{{${p}}}`))
}

interface PromptCardProps {
  prompt: PromptSummary
  onSave: (namespace: string, key: string, text: string) => Promise<void>
  onReset: (namespace: string, key: string) => Promise<void>
}

export function PromptCard({ prompt, onSave, onReset }: PromptCardProps) {
  const { t } = useI18n()
  const effectiveText = prompt.overrideText ?? prompt.defaultText
  const [open, setOpen] = useState(false)
  const [showDefault, setShowDefault] = useState(false)
  const [draft, setDraft] = useState(effectiveText)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setDraft(effectiveText)
  }, [effectiveText])

  const dirty = draft.trim() !== effectiveText.trim()
  const warning = useMemo(() => {
    const dropped = droppedPlaceholders(prompt.placeholders, draft)
    if (dropped.length === 0) return undefined
    return t('settings.prompts.droppedPlaceholderWarning').replace(
      '{{placeholders}}',
      dropped.map((p) => `{{${p}}}`).join(', '),
    )
  }, [draft, prompt.placeholders, t])

  const isCustomized = prompt.overrideText !== null

  const handleSave = useCallback(async () => {
    setSaving(true)
    try {
      await onSave(prompt.namespace, prompt.key, draft)
    } catch (error) {
      useUIStore.getState().addNotification('error', toDisplayError(error, t('sdkUi.saveError')))
    } finally {
      setSaving(false)
    }
  }, [draft, onSave, prompt.key, prompt.namespace, t])

  const handleReset = useCallback(() => {
    onReset(prompt.namespace, prompt.key).catch((error: unknown) =>
      useUIStore.getState().addNotification('error', toDisplayError(error, t('sdkUi.saveError'))),
    )
  }, [onReset, prompt.key, prompt.namespace, t])

  return (
    <GlassCard span={open ? 'full' : 1} className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center justify-between gap-2 text-left"
      >
        <span className="font-mono text-[12px] text-text-main">{prompt.key}</span>
        {isCustomized && <StatusPill variant="ok" label={t('settings.prompts.customizedPill')} dot />}
      </button>

      {placeholderChips(prompt.placeholders)}

      <Collapsible open={open}>
        <div className="mt-2 flex flex-col gap-3 md:flex-row">
          <div className="flex-1 space-y-2">
            <AuroraField
              label={prompt.key}
              hideLabel
              type="textarea"
              rows={16}
              value={draft}
              onChange={setDraft}
              warning={warning}
            />
            <div className="flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowDefault((v) => !v)}
                className="inline-flex min-h-8 shrink-0 items-center gap-1.5 rounded border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption text-text-muted transition-colors hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus"
              >
                {showDefault ? t('settings.prompts.viewDefaultClose') : t('settings.prompts.viewDefault')}
              </button>
              <ConfirmButton
                ariaLabel={t('settings.prompts.restoreDefaultAriaLabel').replace('{{key}}', prompt.key)}
                label={t('settings.prompts.restoreDefault')}
                icon={RotateCcw}
                onConfirm={handleReset}
                disabled={!isCustomized}
              />
              <button
                type="button"
                onClick={handleSave}
                disabled={!dirty || saving}
                className="inline-flex min-h-8 shrink-0 items-center gap-1.5 rounded border border-accent-rim bg-accent-soft px-2.5 py-1.5 text-caption font-medium text-accent transition-colors hover:bg-accent-soft/80 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus disabled:opacity-50"
              >
                <Save size={11} aria-hidden="true" />
                {t('settings.prompts.save')}
              </button>
            </div>
          </div>

          {showDefault && (
            <div className="flex-1 rounded-lg border border-border-subtle bg-surface-raise p-2">
              <pre className="max-h-[26rem] overflow-auto whitespace-pre-wrap break-words text-caption leading-snug text-text-muted">
                {prompt.defaultText}
              </pre>
            </div>
          )}
        </div>
      </Collapsible>
    </GlassCard>
  )
}
