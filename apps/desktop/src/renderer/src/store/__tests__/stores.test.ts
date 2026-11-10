import { beforeEach, describe, expect, it } from 'vitest'
import { useChatStore } from '../useChatStore'
import { useTrackingStore } from '../useTrackingStore'
import { useUIStore } from '../useUIStore'

describe('useChatStore', () => {
  beforeEach(() => {
    useChatStore.setState({
      messages: [],
      bubbleText: '',
      bubbleVisible: false,
      appStatus: 'idle',
      activeTool: null,
      turnSteps: [],
      sessionUsage: {
        promptTokens: 0,
        completionTokens: 0,
        totalTokens: 0,
        costUsd: 0,
        turns: 0,
        lastPromptTokens: 0,
        contextWindow: 0,
        unknownCostTurns: 0,
      },
    })
  })

  it('gerencia mensagens respeitando teto de 500', () => {
    const store = useChatStore.getState()
    store.addMessage({ id: '1', role: 'user', content: 'Olá', timestamp: 100 })
    expect(useChatStore.getState().messages).toHaveLength(1)

    const many = Array.from({ length: 505 }, (_, i) => ({
      id: String(i),
      role: 'user' as const,
      content: `msg ${i}`,
      timestamp: i,
    }))
    store.setMessages(many)
    expect(useChatStore.getState().messages).toHaveLength(500)
    expect(useChatStore.getState().messages[499].id).toBe('504')

    store.addMessage({ id: 'overflow', role: 'assistant', content: 'fim', timestamp: 9999 })
    expect(useChatStore.getState().messages).toHaveLength(500)
    expect(useChatStore.getState().messages[499].id).toBe('overflow')
  })

  it('controla bolha e status da aplicação', () => {
    const store = useChatStore.getState()
    store.setBubble('Pensando...', true)
    expect(useChatStore.getState().bubbleText).toBe('Pensando...')
    expect(useChatStore.getState().bubbleVisible).toBe(true)

    store.setBubbleVisible(false)
    expect(useChatStore.getState().bubbleVisible).toBe(false)
    expect(useChatStore.getState().bubbleText).toBe('Pensando...')

    store.setAppStatus('thinking')
    expect(useChatStore.getState().appStatus).toBe('thinking')
    store.setAppStatus((prev) => (prev === 'thinking' ? 'speaking' : 'idle'))
    expect(useChatStore.getState().appStatus).toBe('speaking')
  })

  it('registra e finaliza passos de ferramentas com limite', () => {
    const store = useChatStore.getState()
    store.beginStep('tool_a', 'Lendo arquivo')
    store.beginStep('tool_b', 'Pesquisando')
    store.beginStep('tool_a', 'Lendo de novo')
    expect(useChatStore.getState().turnSteps).toHaveLength(2)

    store.finishStep('tool_a', true)
    expect(useChatStore.getState().turnSteps[0].status).toBe('done')

    store.finishStep('tool_b', false)
    expect(useChatStore.getState().turnSteps[1].status).toBe('error')

    store.resetTurnSteps()
    expect(useChatStore.getState().turnSteps).toHaveLength(0)
  })

  it('atualiza métricas de sessão de uso', () => {
    const store = useChatStore.getState()
    store.setSessionUsage((prev) => ({ ...prev, turns: 3, totalTokens: 1500 }))
    expect(useChatStore.getState().sessionUsage.turns).toBe(3)
    expect(useChatStore.getState().sessionUsage.totalTokens).toBe(1500)
  })
})

describe('useUIStore', () => {
  beforeEach(() => {
    useUIStore.setState({
      showInput: false,
      showHistory: false,
      pinned: false,
      notifications: [],
    })
  })

  it('alterna overlays exclusivamente', () => {
    const store = useUIStore.getState()
    store.toggleOverlay('input')
    expect(useUIStore.getState().showInput).toBe(true)
    expect(useUIStore.getState().showHistory).toBe(false)

    store.toggleOverlay('history')
    expect(useUIStore.getState().showInput).toBe(false)
    expect(useUIStore.getState().showHistory).toBe(true)

    store.closeOverlays()
    expect(useUIStore.getState().showInput).toBe(false)
    expect(useUIStore.getState().showHistory).toBe(false)
  })

  it('enfileira e descarta notificações', () => {
    const store = useUIStore.getState()
    store.addNotification('info', 'Mensagem 1', 'Título')
    expect(useUIStore.getState().notifications).toHaveLength(1)

    store.addNotification('info', 'Mensagem 1', 'Título')
    expect(useUIStore.getState().notifications).toHaveLength(1)

    const id = useUIStore.getState().notifications[0].id
    store.markNotificationShown(id)
    expect(useUIStore.getState().notifications[0].shown).toBe(true)

    store.dismissNotification(id)
    expect(useUIStore.getState().notifications).toHaveLength(0)

    for (let i = 0; i < 8; i++) {
      store.addNotification('warning', `Alerta ${i}`)
    }
    expect(useUIStore.getState().notifications).toHaveLength(5)

    store.dismissAllNotifications()
    expect(useUIStore.getState().notifications).toHaveLength(0)
  })

  it('controla fixação na tela (pin)', () => {
    const store = useUIStore.getState()
    expect(store.pinned).toBe(false)
    store.setPinned(true)
    expect(useUIStore.getState().pinned).toBe(true)
  })
})

describe('useTrackingStore', () => {
  it('alterna mouse tracking', () => {
    const store = useTrackingStore.getState()
    expect(store.mouseTrackingEnabled).toBe(true)
    store.setMouseTracking(false)
    expect(useTrackingStore.getState().mouseTrackingEnabled).toBe(false)
    store.setMouseTracking(true)
    expect(useTrackingStore.getState().mouseTrackingEnabled).toBe(true)
  })
})
