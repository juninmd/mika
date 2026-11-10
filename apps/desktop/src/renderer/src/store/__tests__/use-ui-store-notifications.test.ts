import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useUIStore } from '../useUIStore'

describe('useUIStore notifications overflow', () => {
  beforeEach(() => {
    useUIStore.setState({ notifications: [] })
  })

  it('descarta as mais antigas da fila, nunca as mais recentes (visíveis)', () => {
    for (let i = 1; i <= 7; i += 1) {
      useUIStore.getState().addNotification('info', `mensagem ${i}`)
    }

    const { notifications } = useUIStore.getState()

    // Respeita o teto declarado (MAX_STACK = 5).
    expect(notifications).toHaveLength(5)
    // As duas mais novas (as visíveis na tela) nunca são removidas por overflow.
    expect(notifications.at(-1)?.message).toBe('mensagem 7')
    expect(notifications.at(-2)?.message).toBe('mensagem 6')
    // As mais antigas (nunca chegaram a ser mostradas) é que saem primeiro.
    expect(notifications.some((n) => n.message === 'mensagem 1')).toBe(false)
    expect(notifications.some((n) => n.message === 'mensagem 2')).toBe(false)
  })

  it('as toasts atualmente visíveis (as 2 mais novas) nunca somem no meio de uma rajada', () => {
    const MAX_VISIBLE_TOASTS = 2

    for (let i = 1; i <= 8; i += 1) {
      const before = useUIStore.getState().notifications
      const visibleBefore = before.slice(-MAX_VISIBLE_TOASTS).map((n) => n.id)

      useUIStore.getState().addNotification('info', `rajada ${i}`)

      const after = useUIStore.getState().notifications
      const idsAfter = new Set(after.map((n) => n.id))
      for (const id of visibleBefore) {
        expect(idsAfter.has(id)).toBe(true)
      }
    }
  })

  it('sem overflow mantém todas as notificações', () => {
    useUIStore.getState().addNotification('info', 'a')
    useUIStore.getState().addNotification('info', 'b')

    expect(useUIStore.getState().notifications).toHaveLength(2)
  })

  it('não duplica notificação idêntica consecutiva', () => {
    useUIStore.getState().addNotification('error', 'falha', 'Título')
    useUIStore.getState().addNotification('error', 'falha', 'Título')

    expect(useUIStore.getState().notifications).toHaveLength(1)
  })
})

describe('useUIStore notifications timeout', () => {
  beforeEach(() => {
    useUIStore.setState({ notifications: [] })
    vi.useFakeTimers()
  })

  it('descarta automaticamente após 60s se ainda presente', () => {
    useUIStore.getState().addNotification('info', 'expira sozinha')
    expect(useUIStore.getState().notifications).toHaveLength(1)

    vi.advanceTimersByTime(60000)

    expect(useUIStore.getState().notifications).toHaveLength(0)
    vi.useRealTimers()
  })
})
