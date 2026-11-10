export function handleNavKeyDown(e: React.KeyboardEvent<HTMLElement>): void {
  const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('button'))
  if (buttons.length === 0) return
  const currentIdx = buttons.indexOf(document.activeElement as HTMLButtonElement)

  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    if (currentIdx === -1) {
      buttons[0]?.focus()
      buttons[0]?.click()
      return
    }
    const nextIdx =
      e.key === 'ArrowDown' ? (currentIdx + 1) % buttons.length : (currentIdx - 1 + buttons.length) % buttons.length
    buttons[nextIdx]?.focus()
    buttons[nextIdx]?.click()
  } else if (e.key === 'Home') {
    e.preventDefault()
    buttons[0]?.focus()
    buttons[0]?.click()
  } else if (e.key === 'End') {
    e.preventDefault()
    buttons[buttons.length - 1]?.focus()
    buttons[buttons.length - 1]?.click()
  }
}
