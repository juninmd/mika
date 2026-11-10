import type { CrisisRegion } from '@mika/sdk/contracts'
import { create } from 'zustand'
import type {
  ActiveToolState,
  AppMessage,
  AppStatus,
  SessionUsage,
  ThinkingStep,
} from '../components/app/app-shell.types'
import { DEFAULT_SESSION_USAGE } from '../components/app/app-shell.types'

const MAX_STORE_MESSAGES = 500
/** Um turno com muitos passos vira ruido na bolha; os ultimos sao os que importam. */
const MAX_TURN_STEPS = 6

interface ChatState {
  messages: AppMessage[]
  bubbleText: string
  bubbleVisible: boolean
  appStatus: AppStatus
  activeTool: ActiveToolState
  turnSteps: ThinkingStep[]
  sessionUsage: SessionUsage
  /** Region of the crisis-resources card (`brain/crisis.rs`); null = card hidden. */
  crisisRegion: CrisisRegion | null

  setMessages: (messages: AppMessage[] | ((prev: AppMessage[]) => AppMessage[])) => void
  addMessage: (message: AppMessage) => void
  setBubble: (text: string, visible?: boolean) => void
  setBubbleVisible: (visible: boolean) => void
  setAppStatus: (status: AppStatus | ((prev: AppStatus) => AppStatus)) => void
  setActiveTool: (tool: ActiveToolState) => void
  resetTurnSteps: () => void
  beginStep: (toolName: string, label: string) => void
  finishStep: (toolName: string, ok: boolean) => void
  setSessionUsage: (usage: SessionUsage | ((prev: SessionUsage) => SessionUsage)) => void
  setCrisisRegion: (region: CrisisRegion | null) => void
}

export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  bubbleText: '',
  bubbleVisible: false,
  appStatus: 'idle',
  activeTool: null,
  turnSteps: [],
  sessionUsage: DEFAULT_SESSION_USAGE,
  crisisRegion: null,

  setCrisisRegion: (crisisRegion) => set({ crisisRegion }),

  setMessages: (messages) =>
    set((state) => {
      const next = typeof messages === 'function' ? messages(state.messages) : messages
      return { messages: next.length > MAX_STORE_MESSAGES ? next.slice(-MAX_STORE_MESSAGES) : next }
    }),

  addMessage: (message) =>
    set((state) => {
      const next = [...state.messages, message]
      if (next.length > MAX_STORE_MESSAGES) {
        return { messages: next.slice(-MAX_STORE_MESSAGES) }
      }
      return { messages: next }
    }),

  setBubble: (text, visible) =>
    set((state) => ({
      bubbleText: text,
      bubbleVisible: visible !== undefined ? visible : state.bubbleVisible,
    })),

  setBubbleVisible: (visible) => set({ bubbleVisible: visible }),

  setAppStatus: (status) =>
    set((state) => ({
      appStatus: typeof status === 'function' ? status(state.appStatus) : status,
    })),

  setActiveTool: (tool) => set({ activeTool: tool }),

  // A checklist é do TURNO: começa vazia a cada `thinking-start` e não carrega o turno anterior.
  resetTurnSteps: () => set({ turnSteps: [] }),

  beginStep: (toolName, label) =>
    set((state) => {
      if (state.turnSteps.some((step) => step.id === toolName && step.status === 'active')) return state
      const steps = [...state.turnSteps, { id: toolName, label, status: 'active' as const }]
      return { turnSteps: steps.length > MAX_TURN_STEPS ? steps.slice(-MAX_TURN_STEPS) : steps }
    }),

  // Fecha o passo ATIVO mais recente daquela ferramenta: a mesma tool pode rodar duas vezes no turno.
  finishStep: (toolName, ok) =>
    set((state) => {
      const index = state.turnSteps.findLastIndex((step) => step.id === toolName && step.status === 'active')
      if (index === -1) return state
      const steps = [...state.turnSteps]
      steps[index] = { ...steps[index], status: ok ? 'done' : 'error' }
      return { turnSteps: steps }
    }),

  setSessionUsage: (usage) =>
    set((state) => ({
      sessionUsage: typeof usage === 'function' ? usage(state.sessionUsage) : usage,
    })),
}))
