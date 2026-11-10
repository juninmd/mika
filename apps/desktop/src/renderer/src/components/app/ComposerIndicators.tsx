import { motion } from 'framer-motion'
import type { SlashCommandDef } from '../../hooks/shell-commands'
import { useI18n } from '../../hooks/useI18n'
import { SLASH_COMMAND_LISTBOX_ID, slashCommandOptionId } from '../../hooks/useSlashCommandSuggestions'

export type SlashCommandMenuProps = {
  matches: SlashCommandDef[]
  selectedIndex: number
  onPick: (index: number) => void
  onHover: (index: number) => void
}

export function SlashCommandMenu({ matches, selectedIndex, onPick, onHover }: SlashCommandMenuProps) {
  const { t } = useI18n()
  if (matches.length === 0) return null

  return (
    <div
      role="listbox"
      id={SLASH_COMMAND_LISTBOX_ID}
      aria-label={t('shell.composer.commands.title')}
      className="absolute inset-x-5 bottom-full z-10 mb-2 max-h-56 overflow-y-auto rounded-2xl border border-border-subtle bg-bg-surface/95 p-1.5 shadow-xl backdrop-blur-2xl"
    >
      {matches.map((cmd, i) => (
        <div
          key={cmd.command}
          id={slashCommandOptionId(cmd.command)}
          role="option"
          tabIndex={-1}
          aria-selected={i === selectedIndex}
          onMouseEnter={() => onHover(i)}
          onMouseDown={(e) => {
            e.preventDefault()
            onPick(i)
          }}
          className={`flex cursor-pointer items-center justify-between gap-3 rounded-xl px-3 py-2 text-[12px] transition-colors ${
            i === selectedIndex ? 'bg-info-soft text-text-main' : 'text-text-muted hover:bg-surface-raise'
          }`}
        >
          <span className="font-mono font-semibold text-text-main">/{cmd.command}</span>
          <span className="truncate text-caption text-text-muted">{t(cmd.descriptionKey)}</span>
        </div>
      ))}
    </div>
  )
}

export function StatusDot({
  listening,
  transcribing,
  isFocused,
}: {
  listening: boolean
  transcribing?: boolean
  isFocused: boolean
}) {
  return (
    <motion.div
      aria-hidden="true"
      className="mb-2 h-2 w-2 flex-shrink-0 rounded-full"
      animate={
        listening
          ? { backgroundColor: 'var(--danger)', boxShadow: '0 0 10px var(--danger-soft)', scale: [1, 1.4, 1] }
          : transcribing
            ? { backgroundColor: 'var(--warn)', boxShadow: '0 0 10px var(--warn-soft)', scale: [1, 1.25, 1] }
            : isFocused
              ? { backgroundColor: 'var(--neural-cyan)', boxShadow: '0 0 10px var(--neural-cyan)', scale: 1.2 }
              : { backgroundColor: 'var(--border-strong)', boxShadow: 'none', scale: 1 }
      }
      transition={{
        duration: listening ? 0.8 : transcribing ? 0.5 : 0.25,
        repeat: listening || transcribing ? Infinity : 0,
      }}
    />
  )
}
