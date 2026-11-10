import { create } from 'zustand'

export type NotificationKind = 'info' | 'success' | 'warning' | 'error'

export interface AppNotification {
  id: string
  kind: NotificationKind
  title?: string
  message: string
  shown: boolean
}

const MAX_STACK = 5

interface UIState {
  showInput: boolean
  showHistory: boolean
  pinned: boolean
  notifications: AppNotification[]

  setPinned: (pinned: boolean) => void
  setShowInput: (show: boolean) => void
  setShowHistory: (show: boolean) => void

  toggleOverlay: (overlay: 'input' | 'history') => void
  closeOverlays: () => void

  addNotification: (kind: NotificationKind, message: string, title?: string) => void
  markNotificationShown: (id: string) => void
  dismissNotification: (id: string) => void
  dismissAllNotifications: () => void
}

export const useUIStore = create<UIState>((set, get) => ({
  showInput: false,
  showHistory: false,
  pinned: false,
  notifications: [],

  setPinned: (pinned) => set({ pinned }),
  setShowInput: (show) => set({ showInput: show }),
  setShowHistory: (show) => set({ showHistory: show }),

  toggleOverlay: (overlay) => {
    const state = get()
    const isOpen = overlay === 'input' ? state.showInput : state.showHistory

    set({
      showInput: overlay === 'input' ? !isOpen : false,
      showHistory: overlay === 'history' ? !isOpen : false,
    })
  },

  closeOverlays: () =>
    set({
      showInput: false,
      showHistory: false,
    }),

  addNotification: (kind, message, title) => {
    const existing = get().notifications
    const last = existing[existing.length - 1]
    if (last && last.kind === kind && last.message === message && last.title === title) {
      return
    }
    const id = crypto.randomUUID()
    set((state) => {
      const next = [...state.notifications, { id, kind, message, title, shown: false }]
      // Drop oldest queued notifications first — visible toasts are always the newest entries.
      const trimmed = next.length > MAX_STACK ? next.slice(next.length - MAX_STACK) : next
      return { notifications: trimmed }
    })
    setTimeout(() => {
      const current = get()
      if (current.notifications.some((n) => n.id === id)) {
        current.dismissNotification(id)
      }
    }, 60000)
  },

  markNotificationShown: (id) =>
    set((state) => ({
      notifications: state.notifications.map((n) => (n.id === id && !n.shown ? { ...n, shown: true } : n)),
    })),

  dismissNotification: (id) =>
    set((state) => ({
      notifications: state.notifications.filter((n) => n.id !== id),
    })),

  dismissAllNotifications: () =>
    set({
      notifications: [],
    }),
}))
