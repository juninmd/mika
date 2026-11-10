import { DURATION, EASE, MOTION } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { useActionState, useCallback, useEffect, useRef, useState } from 'react'
import { useAutoResizeTextarea } from '../../hooks/useAutoResizeTextarea'
import { useChatComposerKeyboard } from '../../hooks/useChatComposerKeyboard'
import { useComposerSpeech } from '../../hooks/useComposerSpeech'
import { useI18n } from '../../hooks/useI18n'
import { useSlashCommandSuggestions } from '../../hooks/useSlashCommandSuggestions'
import { getElectronApi } from '../../lib/shell-api'
import { useChatStore } from '../../store/useChatStore'
import { ComposerForm, MAX_MSG_LENGTH, PLACEHOLDERS, SlashCommandMenu } from './chat-composer-subcomponents'

type ChatComposerProps = {
  value: string
  onChange: (next: string) => void
  onClose: () => void
  onSend: () => void | Promise<void>
  speechLang?: string
}

export function ChatComposer({ value, onChange, onClose, onSend, speechLang }: ChatComposerProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isComposingRef = useRef(false)
  const [isFocused, setIsFocused] = useState(false)
  const { t, locale } = useI18n()
  const [placeholderKey] = useState(
    () => PLACEHOLDERS[Math.floor(Math.random() * PLACEHOLDERS.length)] as (typeof PLACEHOLDERS)[number],
  )
  const placeholder = t(placeholderKey)
  const valueRef = useRef(value)
  valueRef.current = value
  const onSendRef = useRef(onSend)
  onSendRef.current = onSend

  const {
    listening,
    processing: transcribing,
    stop: stopListening,
  } = useComposerSpeech({
    speechLang: speechLang || locale,
    onChange,
    textareaRef,
  })

  useAutoResizeTextarea(textareaRef, 240, value)

  const [, submitAction, isSending] = useActionState<null, FormData>(async (prev) => {
    if (!valueRef.current.trim()) return prev
    await onSendRef.current()
    return prev
  }, null)

  const canSend = Boolean(value.trim()) && !isSending

  const handleCancel = useCallback(() => {
    void getElectronApi().runtime.cancelChatMessage()
  }, [])
  const nearLimit = value.length >= MAX_MSG_LENGTH * 0.9
  const atLimit = value.length >= MAX_MSG_LENGTH

  const onRecallLastMessage = useCallback(() => {
    const messages = useChatStore.getState().messages
    const lastUser = [...messages].reverse().find((m) => m.role === 'user')
    if (lastUser?.content) {
      onChange(lastUser.content)
      requestAnimationFrame(() => {
        if (textareaRef.current) {
          textareaRef.current.selectionStart = textareaRef.current.value.length
          textareaRef.current.selectionEnd = textareaRef.current.value.length
        }
      })
    }
  }, [onChange])

  const slashCommands = useSlashCommandSuggestions(value, onChange)

  const { handleKeyDown } = useChatComposerKeyboard({
    isComposingRef,
    canSend,
    listening,
    stopListening,
    onClose,
    onRecallLastMessage,
    slashMenu: {
      isOpen: slashCommands.isOpen,
      onNavigate: slashCommands.navigate,
      onSelect: slashCommands.select,
      onClose: slashCommands.close,
    },
  })

  useEffect(() => {
    textareaRef.current?.focus()
  }, [])

  return (
    <div
      className="absolute inset-x-0 bottom-20 flex justify-center px-4"
      style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
    >
      <motion.div {...MOTION.sheet} className="relative mx-auto w-full max-w-2xl">
        <motion.div
          className="pointer-events-none absolute inset-0 rounded-[2rem]"
          animate={{
            opacity: listening ? 1 : isFocused ? 1 : 0.25,
            scale: listening ? 1.12 : isFocused ? 1.06 : 1,
          }}
          transition={{ duration: DURATION.slow, ease: EASE.out }}
          style={{
            background: listening
              ? 'radial-gradient(ellipse at 50% 100%, var(--danger-soft), var(--accent-soft), transparent 70%)'
              : 'radial-gradient(ellipse at 50% 100%, var(--neural-cyan), var(--neural-purple), transparent 70%)',
            filter: 'blur(24px)',
          }}
        />

        {slashCommands.isOpen && (
          <SlashCommandMenu
            matches={slashCommands.matches}
            selectedIndex={slashCommands.selectedIndex}
            onPick={slashCommands.select}
            onHover={slashCommands.setSelectedIndex}
          />
        )}

        <ComposerForm
          submitAction={submitAction}
          listening={listening}
          transcribing={transcribing}
          isFocused={isFocused}
          value={value}
          placeholder={placeholder}
          textareaRef={textareaRef}
          isComposingRef={isComposingRef}
          handleKeyDown={handleKeyDown}
          setIsFocused={setIsFocused}
          nearLimit={nearLimit}
          atLimit={atLimit}
          canSend={canSend}
          isSending={isSending}
          onClose={onClose}
          onChange={onChange}
          onCancel={handleCancel}
          slashMatches={slashCommands.isOpen ? slashCommands.matches : undefined}
          slashSelectedIndex={slashCommands.selectedIndex}
        />
      </motion.div>
    </div>
  )
}
