import { toDisplayError } from '@mika/sdk-ui'
import { ConfirmButton, GlassCard, SectionHeader } from '@mika/sdk-ui/settings'
import { CalendarClock, Coffee, Droplets, type LucideIcon, Pencil, PersonStanding, Trash2 } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { getElectronApi } from '../../../renderer/src/lib/shell-api'
import { useUIStore } from '../../../renderer/src/store/useUIStore'
import { formatDue, toHhMm } from './reminder-time'
import type { Recurrence, Reminder, ReminderDraft, ReminderToolResult } from './reminder-types'

export function ReminderList({
  disabled,
  onSelectPreset,
}: {
  disabled: boolean
  onSelectPreset?: (preset: ReminderDraft) => void
}) {
  const { t, locale } = useI18n()
  const [reminders, setReminders] = useState<Reminder[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [removingId, setRemovingId] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = (await getElectronApi().runtime.executeTool('reminder', {
        action: 'list',
      })) as ReminderToolResult
      if (result.success === false) throw new Error(result.error ?? t('settings.scheduler.panel.remindersLoadError'))
      const list = result.data?.reminders ?? []
      setReminders([...list].sort((a, b) => a.dueAt - b.dueAt))
    } catch (err) {
      setError(toDisplayError(err, t('settings.scheduler.panel.remindersLoadError')))
    } finally {
      setLoading(false)
    }
  }, [t])

  useEffect(() => {
    void load()
  }, [load])

  const handleRemove = useCallback(
    async (id: string) => {
      if (disabled) return
      setRemovingId(id)
      try {
        const result = (await getElectronApi().runtime.executeTool('reminder', {
          action: 'cancel',
          id,
        })) as ReminderToolResult
        if (result.success === false) throw new Error(result.error ?? t('settings.scheduler.panel.removeError'))
        await load()
      } catch (err) {
        useUIStore.getState().addNotification('error', toDisplayError(err, t('settings.scheduler.panel.removeError')))
      } finally {
        setRemovingId(null)
      }
    },
    [load, t, disabled],
  )

  const presets: { icon: LucideIcon; label: string; subject: string; time: string; recurrence: Recurrence }[] = [
    {
      icon: Droplets,
      label: t('settings.scheduler.panel.preset.drinkWater.label'),
      subject: 'Hora de beber um copo d’água e hidratar!',
      time: '14:00',
      recurrence: 'daily',
    },
    {
      icon: PersonStanding,
      label: t('settings.scheduler.panel.preset.stretch.label'),
      subject: 'Pausa para esticar o corpo e relaxar os olhos.',
      time: '16:30',
      recurrence: 'daily',
    },
    {
      icon: Coffee,
      label: t('settings.scheduler.panel.preset.coffeeBreak.label'),
      subject: 'Pausa rápida para um café ou chá.',
      time: '15:15',
      recurrence: 'once',
    },
  ]

  return (
    <GlassCard>
      <SectionHeader
        title={t('settings.scheduler.panel.remindersTitle')}
        badge={loading ? undefined : String(reminders.length)}
        className="mb-4"
      />

      {error && (
        <div role="alert" className="mb-3 rounded-xl border border-warn-rim bg-warn-soft p-3 text-xs text-warn">
          {error}
        </div>
      )}

      {loading ? (
        <p className="text-xs text-text-muted animate-pulse py-4 text-center">
          {t('settings.scheduler.panel.loadingReminders')}
        </p>
      ) : reminders.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-6 text-center">
          <CalendarClock size={28} className="text-text-muted/60 mb-2" />
          <p className="text-xs font-semibold text-text-muted">{t('settings.scheduler.panel.remindersEmpty')}</p>
          <p className="mt-1 text-caption text-text-subtle max-w-xs">
            {t('settings.scheduler.panel.remindersEmptyHint')}
          </p>
          <div className="mt-3.5 flex flex-wrap items-center justify-center gap-2">
            {presets.map((p) => (
              <button
                key={p.label}
                type="button"
                disabled={disabled}
                onClick={() => onSelectPreset?.({ subject: p.subject, atTime: p.time, recurrence: p.recurrence })}
                className="inline-flex items-center gap-1 rounded-lg border border-border-subtle bg-surface-raise/60 px-2.5 py-1 text-caption font-bold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent disabled:opacity-50 active:scale-95"
              >
                <p.icon aria-hidden="true" size={14} />
                {p.label}
              </button>
            ))}
          </div>
        </div>
      ) : (
        <ul className="space-y-2.5 max-h-[280px] overflow-y-auto pr-1">
          {reminders.map((reminder) => (
            <li
              key={reminder.id}
              className="flex items-center justify-between gap-3 rounded-xl border border-border-subtle bg-surface-raise/40 p-3"
            >
              <div className="min-w-0">
                <p className="truncate text-xs font-bold text-text-main">{reminder.subject}</p>
                <p className="mt-0.5 text-caption text-text-muted">
                  {t('settings.scheduler.due.next')}{' '}
                  <span className="font-semibold text-text-main">
                    {formatDue(reminder.dueAt, new Date(), locale, t)}
                  </span>{' '}
                  ·{' '}
                  <span className="font-semibold text-accent">
                    {reminder.recurrence === 'daily'
                      ? t('settings.scheduler.panel.recurrenceDaily')
                      : t('settings.scheduler.panel.recurrenceOnce')}
                  </span>
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <button
                  type="button"
                  disabled={disabled}
                  onClick={() =>
                    onSelectPreset?.({
                      subject: reminder.subject,
                      atTime: toHhMm(reminder.dueAt),
                      recurrence: reminder.recurrence,
                      editId: reminder.id,
                    })
                  }
                  aria-label={`${t('settings.scheduler.edit.button')}: ${reminder.subject}`}
                  className="inline-flex min-h-8 items-center gap-1.5 rounded border border-border-subtle bg-surface-raise px-2.5 py-1.5 text-caption text-text-muted transition-colors hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-focus disabled:opacity-50"
                >
                  <Pencil size={11} aria-hidden="true" />
                  {t('settings.scheduler.edit.button')}
                </button>
                <ConfirmButton
                  icon={Trash2}
                  label={t('settings.scheduler.panel.removeButton')}
                  confirmLabel={t('settings.scheduler.panel.removeConfirmLabel')}
                  cancelLabel={t('settings.scheduler.panel.removeCancelLabel')}
                  ariaLabel={`${t('settings.scheduler.panel.removeButton')}: ${reminder.subject}`}
                  disabled={disabled || removingId === reminder.id}
                  onConfirm={() => void handleRemove(reminder.id)}
                />
              </div>
            </li>
          ))}
        </ul>
      )}
    </GlassCard>
  )
}
