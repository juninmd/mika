import { useCallback, useMemo, useRef, useState } from 'react'
import { SLASH_COMMAND_DEFS, type SlashCommandDef } from './shell-commands'

const SLASH_TOKEN_RE = /^\/(\S*)$/

// Closes once a space is typed: arguments, or a normal message that starts with `/`.
export function useSlashCommandSuggestions(value: string, onChange: (next: string) => void) {
  const query = useMemo(() => SLASH_TOKEN_RE.exec(value)?.[1] ?? null, [value])

  const matches = useMemo<SlashCommandDef[]>(() => {
    if (query === null) return []
    const needle = query.toLowerCase()
    return SLASH_COMMAND_DEFS.filter((def) => def.command.startsWith(needle))
  }, [query])

  const [selectedIndex, setSelectedIndex] = useState(0)
  const [dismissed, setDismissed] = useState(false)

  // Reset selection/dismissal as soon as the command token changes (new keystroke) — done during
  // render (React's documented pattern for resetting state on a prop/derived-value change) rather
  // than an effect, since nothing inside the reset reads `query` itself.
  const prevQueryRef = useRef(query)
  if (prevQueryRef.current !== query) {
    prevQueryRef.current = query
    if (selectedIndex !== 0) setSelectedIndex(0)
    if (dismissed) setDismissed(false)
  }

  const isOpen = query !== null && matches.length > 0 && !dismissed

  const navigate = useCallback(
    (direction: 'up' | 'down') => {
      setSelectedIndex((current) => {
        const len = matches.length
        if (!len) return 0
        return direction === 'down' ? (current + 1) % len : (current - 1 + len) % len
      })
    },
    [matches.length],
  )

  const select = useCallback(
    (index: number = selectedIndex) => {
      const picked = matches[index]
      if (!picked) return
      onChange(`/${picked.command} `)
    },
    [matches, selectedIndex, onChange],
  )

  const close = useCallback(() => setDismissed(true), [])

  return { isOpen, matches, selectedIndex, setSelectedIndex, navigate, select, close }
}

export function slashCommandOptionId(command: string): string {
  return `composer-slash-option-${command}`
}

export const SLASH_COMMAND_LISTBOX_ID = 'composer-slash-listbox'
