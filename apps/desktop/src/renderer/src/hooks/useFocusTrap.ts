import { type RefObject, useCallback } from 'react'

const FOCUSABLE_SELECTOR = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'

type Focusable = { disabled?: boolean }

/**
 * Para onde o Tab/Shift+Tab deve levar o foco, ou `null` quando o navegador pode andar sozinho.
 * Controle desabilitado não recebe foco, então não conta como primeiro/último: com o "Continuar"
 * desabilitado por último, o trap nunca disparava e o Tab escapava do diálogo.
 */
export function trapTarget<T extends Focusable>(
  nodes: readonly T[],
  active: unknown,
  container: unknown,
  shiftKey: boolean,
): T | null {
  const enabled = nodes.filter((node) => !node.disabled)
  const first = enabled[0]
  const last = enabled.at(-1)
  if (!first || !last) return null
  if (shiftKey) return active === first || active === container ? last : null
  return active === last ? first : null
}

// Trap Tab/Shift+Tab navigation inside a modal so focus never escapes behind the backdrop.
export function useFocusTrap(containerRef: RefObject<HTMLElement | null>) {
  return useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key !== 'Tab' || !containerRef.current) return
      const nodes = Array.from(
        containerRef.current.querySelectorAll<HTMLElement & { disabled?: boolean }>(FOCUSABLE_SELECTOR),
      )
      const target = trapTarget(nodes, document.activeElement, containerRef.current, e.shiftKey)
      if (!target) return
      e.preventDefault()
      target.focus()
    },
    [containerRef],
  )
}
