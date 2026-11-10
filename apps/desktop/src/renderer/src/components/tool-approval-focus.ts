export const FOCUSABLE_SELECTOR = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'

export function trapTab(container: HTMLElement | null, e: React.KeyboardEvent) {
  const nodes = container?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)
  if (!nodes || nodes.length === 0) return
  const first = nodes[0]
  const last = nodes[nodes.length - 1]
  const active = document.activeElement
  if (e.shiftKey && (active === first || active === container)) {
    e.preventDefault()
    last.focus()
  } else if (!e.shiftKey && active === last) {
    e.preventDefault()
    first.focus()
  }
}
