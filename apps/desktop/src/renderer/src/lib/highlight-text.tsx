import type { ReactNode } from 'react'

// Split out of ChatHistoryMessageBubble so FencedText (../components/FencedText.tsx) can reuse it
// without a circular import between the two component files.
export function highlightText(text: string, query: string, regex: RegExp | null): ReactNode {
  if (!query || !regex) return text
  const parts = text.split(regex)
  // Índice do array, não offset acumulado: um casamento na posição 0 gera uma primeira parte vazia
  // e duas partes recebiam `key={0}`.
  return parts.map((part, index) =>
    part.toLowerCase() === query.toLowerCase() ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: parts are a fresh, order-stable split of `text` on every render — no reordering/filtering/state to desync, and fragments have no other stable id.
      <mark key={index} className="rounded bg-accent-soft text-accent px-0.5 font-medium not-italic">
        {part}
      </mark>
    ) : (
      part
    ),
  )
}
