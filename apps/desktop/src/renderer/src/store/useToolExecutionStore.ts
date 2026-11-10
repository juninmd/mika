import { create } from 'zustand'
import { maskSensitiveArgs } from '../lib/sensitive-tool-arg'

export type ExecutionStatus = 'success' | 'error'

const MAX_RESULT_CHARS = 2000

function formatResult(result: unknown): string {
  if (result === null || result === undefined) return ''
  let text: string
  if (typeof result === 'string') {
    text = result
  } else {
    try {
      text = JSON.stringify(result, null, 2)
    } catch {
      text = String(result)
    }
  }
  return text.length > MAX_RESULT_CHARS ? `${text.slice(0, MAX_RESULT_CHARS)}…` : text
}

type ToolExecutionEntry = {
  id: string
  toolName: string
  pluginId: string
  args: Record<string, unknown>
  result: unknown
  formattedResult: string
  status: ExecutionStatus
  error?: string
  durationMs: number
  timestamp: number
}

type ToolExecutionState = {
  entries: ToolExecutionEntry[]
  addEntry: (entry: Omit<ToolExecutionEntry, 'formattedResult'>) => void
  clear: () => void
  clearPlugin: (pluginId: string) => void
}

const MAX_ENTRIES = 100

export const useToolExecutionStore = create<ToolExecutionState>((set) => ({
  entries: [],
  addEntry: (entry) =>
    set((state) => {
      const maskedArgs = maskSensitiveArgs(entry.args) as Record<string, unknown>
      const maskedResult = maskSensitiveArgs(entry.result)
      return {
        entries: [
          { ...entry, args: maskedArgs, result: maskedResult, formattedResult: formatResult(maskedResult) },
          ...state.entries,
        ].slice(0, MAX_ENTRIES),
      }
    }),
  clear: () => set({ entries: [] }),
  clearPlugin: (pluginId) =>
    set((state) => ({
      entries: state.entries.filter((e) => e.pluginId !== pluginId),
    })),
}))
