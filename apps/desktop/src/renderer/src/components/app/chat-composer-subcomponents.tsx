import { AnimatePresence, motion } from 'framer-motion'
import type { SlashCommandDef } from '../../hooks/shell-commands'
import { useI18n } from '../../hooks/useI18n'
import { SLASH_COMMAND_LISTBOX_ID } from '../../hooks/useSlashCommandSuggestions'
import { ActionButtons, QuickActionsBar } from './ComposerActions'
import { StatusDot } from './ComposerIndicators'
import { getComposerFormVisuals, MAX_MSG_LENGTH, resolveActiveSlashOptionId } from './chat-composer-helpers'

export type { ActionButtonsProps } from './ComposerActions'
export { ActionButtons, QuickActionsBar, ShortcutsHint } from './ComposerActions'
export type { SlashCommandMenuProps } from './ComposerIndicators'
export { SlashCommandMenu, StatusDot } from './ComposerIndicators'
export { MAX_MSG_LENGTH, PLACEHOLDERS, QUICK_ACTIONS } from './chat-composer-helpers'

export type ComposerTextareaProps = {
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  listening: boolean
  placeholder: string
  value: string
  onChange: (next: string) => void
  setIsFocused: (v: boolean) => void
  isComposingRef: React.RefObject<boolean>
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
  slashMenuOpen?: boolean
  slashActiveOptionId?: string
}

export function ComposerTextarea({
  textareaRef,
  listening,
  placeholder,
  value,
  onChange,
  setIsFocused,
  isComposingRef,
  handleKeyDown,
  slashMenuOpen,
  slashActiveOptionId,
}: ComposerTextareaProps) {
  const { t } = useI18n()
  return (
    <textarea
      ref={textareaRef}
      name="prompt"
      rows={2}
      maxLength={MAX_MSG_LENGTH}
      aria-label={listening ? t('shell.composer.listening') : placeholder}
      role="combobox"
      aria-autocomplete="list"
      aria-expanded={Boolean(slashMenuOpen)}
      aria-controls={SLASH_COMMAND_LISTBOX_ID}
      aria-activedescendant={slashMenuOpen ? slashActiveOptionId : undefined}
      className="flex-1 resize-none bg-transparent text-[13px] leading-relaxed text-text-main/95 placeholder:text-text-muted outline-none"
      style={{ minHeight: '44px', maxHeight: '220px' }}
      placeholder={listening ? t('shell.composer.listening') : placeholder}
      value={value}
      // Synchronous on purpose: a controlled field cannot be driven from a transition — React restores the old
      // value after the event, so a keystroke landing before the transition commits overwrote the previous one.
      onChange={(e) => onChange(e.target.value)}
      onFocus={() => setIsFocused(true)}
      onBlur={() => setIsFocused(false)}
      onCompositionStart={() => {
        isComposingRef.current = true
      }}
      onCompositionEnd={() => {
        isComposingRef.current = false
      }}
      onKeyDown={handleKeyDown}
    />
  )
}

export type ComposerFormProps = {
  submitAction: (payload: FormData) => void
  listening: boolean
  transcribing?: boolean
  isFocused: boolean
  value: string
  placeholder: string
  textareaRef: React.RefObject<HTMLTextAreaElement | null>
  isComposingRef: React.RefObject<boolean>
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void
  setIsFocused: (v: boolean) => void
  nearLimit: boolean
  atLimit: boolean
  canSend: boolean
  isSending: boolean
  onClose: () => void
  onChange: (next: string) => void
  onCancel?: () => void
  slashMatches?: SlashCommandDef[]
  slashSelectedIndex?: number
}

export function ComposerForm({
  submitAction,
  listening,
  transcribing,
  isFocused,
  value,
  placeholder,
  textareaRef,
  isComposingRef,
  handleKeyDown,
  setIsFocused,
  nearLimit,
  atLimit,
  canSend,
  isSending,
  onClose,
  onChange,
  onCancel,
  slashMatches = [],
  slashSelectedIndex = 0,
}: ComposerFormProps) {
  const slashMenuOpen = slashMatches.length > 0
  const slashActiveOptionId = slashMenuOpen ? resolveActiveSlashOptionId(slashMatches, slashSelectedIndex) : undefined
  const { borderClass, gradientTop, gradientBottom } = getComposerFormVisuals(listening, isFocused)

  return (
    <form
      action={submitAction}
      className={`relative flex flex-col gap-0 overflow-hidden rounded-[24px] border bg-bg-surface/90 backdrop-blur-2xl transition duration-300 ${borderClass}`}
    >
      <motion.div
        className="absolute inset-x-0 top-0 h-px"
        animate={{ opacity: listening ? 1 : isFocused ? 1 : 0.35 }}
        style={{ background: gradientTop }}
      />

      {listening && (
        <motion.div
          className="absolute inset-x-0 top-0 h-0.5 bg-danger"
          animate={{ scaleX: [0, 1, 0.7, 1, 0.4, 1] }}
          transition={{ duration: 1.5, repeat: Infinity, ease: 'easeInOut' }}
        />
      )}

      <div className="flex items-end gap-3 px-5 pt-4 pb-3.5">
        <StatusDot listening={listening} transcribing={transcribing} isFocused={isFocused} />

        <ComposerTextarea
          textareaRef={textareaRef}
          listening={listening}
          placeholder={placeholder}
          value={value}
          onChange={onChange}
          setIsFocused={setIsFocused}
          isComposingRef={isComposingRef}
          handleKeyDown={handleKeyDown}
          slashMenuOpen={slashMenuOpen}
          slashActiveOptionId={slashActiveOptionId}
        />

        <div className="flex flex-shrink-0 items-center gap-1">
          <AnimatePresence>
            {nearLimit && (
              <motion.span
                initial={{ opacity: 0, x: 6 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 6 }}
                className={`mr-1 font-mono text-caption tabular-nums ${atLimit ? 'text-danger' : 'text-warn'}`}
              >
                {value.length}/{MAX_MSG_LENGTH}
              </motion.span>
            )}
          </AnimatePresence>

          <ActionButtons canSend={canSend} isSending={isSending} onClose={onClose} onCancel={onCancel} />
        </div>
      </div>

      <div className="mx-5 h-px bg-surface-raise" />

      <QuickActionsBar onChange={onChange} textareaRef={textareaRef} />

      <motion.div
        className="h-px w-full"
        animate={{ opacity: listening ? 0.9 : isFocused ? 0.7 : 0.2 }}
        style={{ background: gradientBottom }}
      />
    </form>
  )
}
