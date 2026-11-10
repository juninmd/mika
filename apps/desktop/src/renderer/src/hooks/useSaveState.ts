import { useMemo } from 'react'
import { create } from 'zustand'

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

type Entry = {
  state: SaveState
  lastSaved: number | null
  error: string | null
}

type SaveStateStore = {
  entries: Record<string, Entry>
  markSaving: (path: string) => void
  markSaved: (path: string) => void
  markError: (path: string, msg: string) => void
  reset: (path: string) => void
}

const EMPTY_ENTRY: Entry = { state: 'idle', lastSaved: null, error: null }

export const useSaveStateStore = create<SaveStateStore>((set) => ({
  entries: {},
  markSaving: (path) =>
    set((s) => ({
      entries: {
        ...s.entries,
        [path]: { state: 'saving', lastSaved: s.entries[path]?.lastSaved ?? null, error: null },
      },
    })),
  markSaved: (path) =>
    set((s) => ({
      entries: { ...s.entries, [path]: { state: 'saved', lastSaved: Date.now(), error: null } },
    })),
  markError: (path, msg) =>
    set((s) => ({
      entries: { ...s.entries, [path]: { state: 'error', lastSaved: s.entries[path]?.lastSaved ?? null, error: msg } },
    })),
  reset: (path) =>
    set((s) => {
      const next = { ...s.entries }
      delete next[path]
      return { entries: next }
    }),
}))

export function useSaveState(configPath: string) {
  const entry = useSaveStateStore((s) => s.entries[configPath] ?? EMPTY_ENTRY)
  const markSaving = useSaveStateStore((s) => s.markSaving)
  const markSaved = useSaveStateStore((s) => s.markSaved)
  const markError = useSaveStateStore((s) => s.markError)
  const reset = useSaveStateStore((s) => s.reset)

  return useMemo(
    () => ({
      isSaving: entry.state === 'saving',
      isSaved: entry.state === 'saved',
      hasError: entry.state === 'error',
      lastSaved: entry.lastSaved,
      error: entry.error,
      markSaving: () => markSaving(configPath),
      markSaved: () => markSaved(configPath),
      markError: (msg: string) => markError(configPath, msg),
      reset: () => reset(configPath),
    }),
    [entry, configPath, markSaving, markSaved, markError, reset],
  )
}
