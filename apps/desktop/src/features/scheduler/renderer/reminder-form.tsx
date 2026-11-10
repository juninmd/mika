import { toDisplayError } from '@mika/sdk-ui'
import { AuroraField, choiceOption, GlassCard, GlassSelect, SectionHeader } from '@mika/sdk-ui/settings'
import { CalendarCheck, Pencil, Plus, Repeat, X, Zap } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { useUIStore } from '../../../renderer/src/store/useUIStore'
import { QUICK_WHEN, type QuickWhen, quickWhenArgs } from './reminder-time'
import type { Recurrence, ReminderDraft, ScheduleToolResult } from './reminder-types'

type Tool = (name: string, args: Record<string, unknown>) => Promise<unknown>
const runTool: Tool = (name, args) => getElectronApi().runtime.executeTool(name, args)

const CHIP_CLS =
  'inline-flex items-center gap-1 rounded-lg border px-2.5 py-1 text-caption font-bold transition active:scale-95 disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus'

async function schedule(args: Record<string, unknown>, fallback: string): Promise<void> {
  const result = (await runTool('reminder', { action: 'schedule', ...args })) as ScheduleToolResult
  if (result.success === false) throw new Error(result.error ?? fallback)
}

/**
 * Create or edit a reminder through the `reminder` tool. Editing is schedule-new-then-cancel-old:
 * the tool has no update action, and cancelling first would lose the reminder if scheduling failed.
 */
export function ReminderForm({
  onCreated,
  disabled,
  preset,
  onCancelEdit,
}: {
  onCreated: () => Promise<void>
  disabled: boolean
  preset?: ReminderDraft | null
  onCancelEdit: () => void
}) {
  const { t } = useI18n()
  const [subject, setSubject] = useState('')
  const [atTime, setAtTime] = useState('')
  const [quick, setQuick] = useState<QuickWhen['id'] | null>(null)
  const [recurrence, setRecurrence] = useState<Recurrence>('once')
  const [submitting, setSubmitting] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  const editId = preset?.editId

  useEffect(() => {
    if (!preset) return
    setSubject(preset.subject)
    setAtTime(preset.atTime)
    setQuick(null)
    setRecurrence(preset.recurrence)
  }, [preset])

  const reset = useCallback(() => {
    setSubject('')
    setAtTime('')
    setQuick(null)
    setRecurrence('once')
  }, [])

  const validate = useCallback((): string | null => {
    if (!subject.trim()) return t('settings.scheduler.panel.subjectRequiredError')
    if (!atTime && !quick) return t('settings.scheduler.panel.timeRequiredError')
    return null
  }, [subject, atTime, quick, t])

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault()
      if (disabled) return
      const invalid = validate()
      setFormError(invalid)
      if (invalid) return
      setSubmitting(true)
      try {
        const when = quick ? quickWhenArgs(quick, new Date()) : { atTime }
        await schedule({ subject: subject.trim(), recurrence, ...when }, t('settings.scheduler.panel.createError'))
        if (editId) await runTool('reminder', { action: 'cancel', id: editId })
        reset()
        const doneKey = editId ? 'settings.scheduler.edit.saved' : 'settings.scheduler.panel.createSuccess'
        useUIStore.getState().addNotification('success', t(doneKey))
        await onCreated()
      } catch (err) {
        const message = toDisplayError(err, t('settings.scheduler.panel.createError'))
        setFormError(message)
        useUIStore.getState().addNotification('error', message)
      } finally {
        setSubmitting(false)
      }
    },
    [subject, atTime, quick, recurrence, t, onCreated, disabled, validate, editId, reset],
  )

  const busy = submitting || disabled
  return (
    <GlassCard accent={Boolean(editId)}>
      <SectionHeader
        title={editId ? t('settings.scheduler.edit.title') : t('settings.scheduler.panel.createTitle')}
        className="mb-4"
        actions={
          editId && (
            <button
              type="button"
              onClick={() => {
                reset()
                onCancelEdit()
              }}
              className="inline-flex items-center gap-1 rounded-lg border border-border-subtle px-2 py-1 text-caption font-semibold text-text-muted transition hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <X size={12} aria-hidden="true" />
              {t('settings.scheduler.edit.cancel')}
            </button>
          )
        }
      />
      <form onSubmit={(e) => void handleSubmit(e)} className="flex flex-col gap-3">
        {formError && (
          <div role="alert" className="rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs text-warn">
            {formError}
          </div>
        )}
        <AuroraField
          label={t('settings.scheduler.panel.subjectLabel')}
          value={subject}
          onChange={setSubject}
          placeholder={t('settings.scheduler.panel.subjectPlaceholder')}
          disabled={busy}
        />
        <div>
          <p className="mb-1.5 text-micro font-bold uppercase tracking-[0.12em] text-text-muted">
            {t('settings.scheduler.quick.label')}
          </p>
          <div className="flex flex-wrap gap-1.5">
            {QUICK_WHEN.map((chip) => {
              const active = quick === chip.id
              return (
                <button
                  key={chip.id}
                  type="button"
                  aria-pressed={active}
                  disabled={busy}
                  onClick={() => {
                    setQuick(active ? null : chip.id)
                    setAtTime('')
                  }}
                  className={`${CHIP_CLS} ${
                    active
                      ? 'border-accent-rim bg-accent-soft text-accent'
                      : 'border-border-subtle bg-surface-raise/60 text-text-main hover:border-accent-rim hover:text-accent'
                  }`}
                >
                  <Zap size={12} aria-hidden="true" />
                  {t(chip.labelKey)}
                </button>
              )
            })}
          </div>
        </div>
        <div className="grid gap-3 sm:grid-cols-[minmax(0,10rem)_1fr]">
          <AuroraField
            label={t('settings.scheduler.panel.timeLabel')}
            type="time"
            value={atTime}
            onChange={(value) => {
              setAtTime(value)
              if (value) setQuick(null)
            }}
            disabled={busy}
          />
          <GlassSelect
            label={t('settings.scheduler.panel.recurrenceLabel')}
            value={recurrence}
            onChange={(v) => setRecurrence(v as Recurrence)}
            disabled={busy}
            options={[
              choiceOption(
                t('settings.scheduler.panel.recurrenceOnce'),
                'once',
                t('settings.scheduler.panel.recurrenceOnceHint'),
                CalendarCheck,
              ),
              choiceOption(
                t('settings.scheduler.panel.recurrenceDaily'),
                'daily',
                t('settings.scheduler.panel.recurrenceDailyHint'),
                Repeat,
              ),
            ]}
          />
        </div>
        <button
          type="submit"
          disabled={busy}
          className="mt-2 inline-flex items-center justify-center gap-2 self-start rounded-xl border border-accent-rim bg-accent-soft px-4 py-2.5 text-xs font-bold text-accent transition hover:bg-accent/20 active:scale-[0.98] disabled:opacity-50"
        >
          {editId ? <Pencil size={14} aria-hidden="true" /> : <Plus size={14} aria-hidden="true" />}
          {submitting
            ? t('settings.scheduler.panel.creatingButton')
            : editId
              ? t('settings.scheduler.edit.save')
              : t('settings.scheduler.panel.createButton')}
        </button>
      </form>
    </GlassCard>
  )
}
