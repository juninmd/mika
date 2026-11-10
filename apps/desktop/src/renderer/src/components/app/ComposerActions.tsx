import { SPRING, staggerDelay } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { useI18n } from '../../hooks/useI18n'
import { CloseIcon, KeyboardIcon, SendIcon, StopIcon } from '../Icons'
import { Tooltip, useTooltipTrigger } from '../ui/Tooltip'
import { QUICK_ACTIONS } from './chat-composer-helpers'

export type ActionButtonsProps = {
  canSend: boolean
  isSending: boolean
  onClose: () => void
  onCancel?: () => void
}

export function ActionButtons({ canSend, isSending, onClose, onCancel }: ActionButtonsProps) {
  const { t } = useI18n()
  return (
    <div className="flex flex-shrink-0 items-center gap-1.5">
      {isSending && onCancel && (
        <motion.button
          type="button"
          onClick={onCancel}
          whileHover={{ scale: 1.08 }}
          whileTap={{ scale: 0.92 }}
          transition={SPRING.snappy}
          aria-label={t('shell.composer.cancel')}
          title={t('shell.composer.cancel')}
          className="flex h-8 w-8 items-center justify-center rounded-full border border-danger-rim bg-danger-soft text-danger transition-colors duration-150 hover:bg-danger hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <StopIcon className="h-3 w-3" aria-hidden="true" />
        </motion.button>
      )}

      <motion.button
        type="submit"
        disabled={!canSend}
        whileHover={canSend ? { scale: 1.08 } : {}}
        whileTap={canSend ? { scale: 0.92 } : {}}
        aria-label={t('shell.composer.send')}
        title={`${t('shell.composer.send')} (Enter)`}
        className={`flex h-8 w-8 items-center justify-center rounded-full transition-[background-color,border-color,color,box-shadow] duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
          canSend
            ? 'bg-info-soft text-info border border-info-rim shadow-[0_0_16px_var(--neural-cyan)] hover:bg-info/25 hover:shadow-[0_0_24px_var(--neural-cyan)]'
            : 'cursor-not-allowed bg-surface-raise border border-border-subtle text-text-muted/40'
        }`}
      >
        {isSending ? (
          <motion.div animate={{ rotate: 360 }} transition={{ duration: 1, repeat: Infinity, ease: 'linear' }}>
            <SendIcon className="h-3.5 w-3.5" aria-hidden="true" />
          </motion.div>
        ) : (
          <SendIcon className="h-3.5 w-3.5" aria-hidden="true" />
        )}
      </motion.button>

      <motion.button
        type="button"
        onClick={onClose}
        whileHover={{ scale: 1.08 }}
        whileTap={{ scale: 0.92 }}
        aria-label={t('shell.composer.close')}
        title={`${t('shell.composer.close')} (Esc)`}
        className="flex h-8 w-8 items-center justify-center rounded-full text-text-muted transition-colors duration-150 hover:bg-danger-soft hover:text-danger hover:border hover:border-danger-rim focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <CloseIcon className="h-3.5 w-3.5" aria-hidden="true" />
      </motion.button>
    </div>
  )
}

export function QuickActionsBar({
  onChange,
  textareaRef,
}: {
  onChange: (next: string) => void
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-wrap items-center justify-between gap-1 px-4 py-2">
      <div className="flex flex-wrap items-center gap-1">
        {QUICK_ACTIONS.map((action, i) => (
          <motion.button
            key={action.label}
            type="button"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ ...SPRING.soft, delay: staggerDelay(i) }}
            whileHover={{ scale: 1.04 }}
            whileTap={{ scale: 0.95 }}
            onClick={() => {
              onChange(t(action.msg))
              textareaRef.current?.focus()
            }}
            className="min-h-7 rounded-full border border-border-subtle bg-surface-raise px-2.5 py-1 text-caption font-medium leading-none text-text-muted backdrop-blur-md transition-[background-color,border-color,color,box-shadow] duration-150 hover:border-info-rim hover:bg-info-soft hover:text-text-main hover:shadow-md hover:shadow-info/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {t(action.label)}
          </motion.button>
        ))}
      </div>
      <ShortcutsHint />
    </div>
  )
}

export function ShortcutsHint() {
  const { t } = useI18n()
  const { visible, triggerProps } = useTooltipTrigger()
  const detail = [
    t('shell.composer.shortcuts.send'),
    t('shell.composer.shortcuts.newline'),
    t('shell.composer.shortcuts.recall'),
    t('shell.composer.shortcuts.closeOrStop'),
    t('shell.composer.shortcuts.openGlobal'),
  ].join(' · ')

  return (
    <div className="relative flex items-center" {...triggerProps}>
      <button
        type="button"
        aria-label={t('shell.composer.shortcuts.label')}
        className="flex h-8 w-8 items-center justify-center rounded-full text-text-muted/70 transition-colors hover:bg-surface-raise hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <KeyboardIcon className="h-3.5 w-3.5" aria-hidden="true" />
      </button>
      <Tooltip label={t('shell.composer.shortcuts.label')} detail={detail} visible={visible} />
    </div>
  )
}
