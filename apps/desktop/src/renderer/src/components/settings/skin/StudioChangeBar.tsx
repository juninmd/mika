import { toDisplayError } from '@mika/sdk-ui'
import { AnimatePresence, motion } from 'framer-motion'
import { ChevronDown, History, RotateCcw } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../../hooks/useI18n'
import { useUIStore } from '../../../store/useUIStore'
import { ConfirmButton } from '../ds'
import type { ChangeJournal } from './useChangeJournal'

function prettyPath(path: string): string {
  return path.split('.').pop() ?? path
}

function describeValue(path: string, value: unknown, onLabel: string, offLabel: string): string {
  if (/key|secret|token|password/i.test(path)) return '••••••'
  if (typeof value === 'boolean') return value ? onLabel : offLabel
  if (value === '' || value === null || value === undefined) return '∅'
  const text = String(value)
  return text.length > 36 ? `${text.slice(0, 35)}…` : text
}

export function StudioChangeBar({ journal }: { journal: ChangeJournal }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const count = journal.entries.length
  const hasChanges = count > 0

  // Reseta o painel de diff quando todas as mudanças são revertidas: sem isso, a PRÓXIMA
  // mudança não relacionada reabre o painel sozinha, sem o usuário clicar em "Ver diff".
  useEffect(() => {
    if (!hasChanges) setOpen(false)
  }, [hasChanges])

  const handleRevertAll = () => {
    journal.revertAll().catch((err) => {
      useUIStore
        .getState()
        .addNotification('error', toDisplayError(err, t('settings.skin.studioChangeBar.revertAllError')))
    })
  }

  const handleRevert = (path: string) => {
    journal.revert(path).catch((err) => {
      useUIStore
        .getState()
        .addNotification('error', toDisplayError(err, t('settings.skin.studioChangeBar.revertOneError')))
    })
  }

  return (
    <div className="sticky bottom-3 z-20">
      <AnimatePresence initial={false}>
        {open && hasChanges && (
          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            transition={{ duration: 0.18 }}
            className="mb-2 max-h-56 overflow-y-auto rounded-2xl border border-border-subtle bg-bg-surface/95 p-2 backdrop-blur-md [scrollbar-width:thin]"
            role="region"
            aria-label={t('settings.skin.studioChangeBar.pendingChangesAriaLabel')}
          >
            {journal.entries.map((entry) => (
              <div
                key={entry.path}
                className="flex items-center gap-3 rounded-lg px-2.5 py-2 transition-colors hover:bg-surface-raise"
              >
                <div className="min-w-0 flex-1">
                  <p className="text-[12px] font-medium text-text-main">{prettyPath(entry.path)}</p>
                  <p className="flex items-center gap-1.5 text-caption text-text-muted">
                    <span className="line-through opacity-60">
                      {describeValue(
                        entry.path,
                        entry.before,
                        t('settings.skin.studioChangeBar.valueOn'),
                        t('settings.skin.studioChangeBar.valueOff'),
                      )}
                    </span>
                    <span className="text-text-subtle">→</span>
                    <span className="text-ok/90">
                      {describeValue(
                        entry.path,
                        entry.after,
                        t('settings.skin.studioChangeBar.valueOn'),
                        t('settings.skin.studioChangeBar.valueOff'),
                      )}
                    </span>
                  </p>
                </div>
                <ConfirmButton
                  icon={RotateCcw}
                  label={t('settings.skin.studioChangeBar.revertLabel')}
                  confirmLabel={t('settings.skin.studioChangeBar.confirmLabel')}
                  ariaLabel={t('settings.skin.studioChangeBar.revertOneAriaLabel').replace(
                    '{{path}}',
                    prettyPath(entry.path),
                  )}
                  onConfirm={() => handleRevert(entry.path)}
                />
              </div>
            ))}
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex items-center gap-3 rounded-2xl border border-border-subtle bg-bg-surface/95 px-4 py-2.5 backdrop-blur-md shadow-2xl shadow-black/50">
        <span
          aria-hidden="true"
          className={`h-2 w-2 shrink-0 rounded-full ${hasChanges ? 'animate-pulse bg-info' : 'bg-surface-raise-strong'}`}
        />
        <div className="min-w-0 flex-1">
          <p className="text-[12px] font-semibold text-text-main">
            {hasChanges
              ? (count === 1
                  ? t('settings.skin.studioChangeBar.changesCountSingular')
                  : t('settings.skin.studioChangeBar.changesCountPlural')
                ).replace('{{count}}', String(count))
              : t('settings.skin.studioChangeBar.noChanges')}
          </p>
          <p className="text-caption text-text-subtle">
            {hasChanges
              ? t('settings.skin.studioChangeBar.autoSavedWithHint')
              : t('settings.skin.studioChangeBar.autoSaved')}
          </p>
        </div>

        {hasChanges && (
          <>
            <button
              type="button"
              onClick={() => setOpen((value) => !value)}
              aria-expanded={open}
              className="inline-flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-3 py-1.5 text-caption font-medium text-text-main transition-colors hover:border-border-strong hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <History size={12} aria-hidden="true" /> {t('settings.skin.studioChangeBar.viewDiff')}
              <motion.span animate={{ rotate: open ? 180 : 0 }} transition={{ duration: 0.2 }}>
                <ChevronDown size={12} aria-hidden="true" />
              </motion.span>
            </button>
            <ConfirmButton
              icon={RotateCcw}
              label={t('settings.skin.studioChangeBar.revertAllLabel')}
              confirmLabel={t('settings.skin.studioChangeBar.confirmLabel')}
              ariaLabel={t('settings.skin.studioChangeBar.revertAllAriaLabel').replace('{{count}}', String(count))}
              onConfirm={handleRevertAll}
            />
          </>
        )}
      </div>
    </div>
  )
}
