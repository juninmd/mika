import { describe, expect, it } from 'vitest'
import { emptyWindow, recordEntry } from '../long-task-probe'

const entry = (entryType: string, duration: number) => ({ entryType, duration })

describe('long-task probe window', () => {
  it('counts only long tasks at or above the threshold — the API reports everything over 50 ms', () => {
    const w = emptyWindow()
    recordEntry(w, entry('longtask', 60))
    recordEntry(w, entry('longtask', 140))
    recordEntry(w, entry('longtask', 100))
    expect(w).toMatchObject({ tarefas: 2, maiorMs: 140, somaMs: 240 })
  })

  it('tracks the slowest interaction separately and ignores fast ones', () => {
    const w = emptyWindow()
    recordEntry(w, entry('event', 40))
    recordEntry(w, entry('event', 180.6))
    expect(w).toMatchObject({ tarefas: 0, interacaoMaiorMs: 181 })
  })
})
