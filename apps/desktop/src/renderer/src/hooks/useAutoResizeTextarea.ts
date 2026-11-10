import { type RefObject, useEffect } from 'react'

/** Pure height calc extracted for unit testing without a DOM textarea. */
export function computeResizedHeight(scrollHeight: number, maxHeight: number): number {
  return Math.min(scrollHeight, maxHeight)
}

export function useAutoResizeTextarea(
  textareaRef: RefObject<HTMLTextAreaElement | null>,
  maxHeight = 120,
  value?: unknown,
) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: `value` is a trigger-only dep — it forces a resize when the textarea's value changes programmatically (not via user input), even though the effect body never reads it directly.
  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    const resize = () => {
      el.style.height = 'auto'
      el.style.height = `${computeResizedHeight(el.scrollHeight, maxHeight)}px`
    }
    resize()
    el.addEventListener('input', resize)
    return () => el.removeEventListener('input', resize)
  }, [textareaRef, maxHeight, value])
}
