import { describe, expect, it } from 'vitest'
import { gpuOffload, nextPullState, type PullState, pullFraction } from '../use-llm-status'

const running = (completed: number, total: number): PullState => ({
  phase: 'running',
  model: 'qwen3:8b',
  status: 'pulling abc',
  completed,
  total,
})

describe('nextPullState', () => {
  // Ollama sends phase lines ("verifying sha256 digest") without byte counts; resetting to 0 made
  // the bar jump back to empty right before the download finished.
  it('keeps the byte counts when a phase line carries none', () => {
    const next = nextPullState(running(900, 1000), {
      model: 'qwen3:8b',
      status: 'verifying sha256 digest',
      done: false,
    })
    expect(next).toEqual({ ...running(900, 1000), status: 'verifying sha256 digest' })
    expect(pullFraction(next)).toBe(0.9)
  })

  it('starts fresh counts for a different model', () => {
    const next = nextPullState(running(900, 1000), { model: 'gemma3:4b', status: 'pulling manifest', done: false })
    expect(next).toEqual({ phase: 'running', model: 'gemma3:4b', status: 'pulling manifest', completed: 0, total: 0 })
    expect(pullFraction(next)).toBeNull()
  })

  it('turns the final event into done or a readable error', () => {
    expect(nextPullState(running(1, 2), { model: 'qwen3:8b', done: true, ok: true })).toEqual({
      phase: 'done',
      model: 'qwen3:8b',
    })
    expect(nextPullState(running(1, 2), { model: 'x', done: true, ok: false, error: 'file does not exist' })).toEqual({
      phase: 'error',
      model: 'x',
      message: 'file does not exist',
    })
  })
})

describe('gpuOffload', () => {
  // /api/ps: `size_vram` < `size` means Ollama split the model and part of it runs on the CPU —
  // the usual reason a turn suddenly takes several seconds while a game holds the VRAM.
  it('tells a model fully in VRAM from a split one and a CPU-only one', () => {
    expect(gpuOffload({ sizeBytes: 6_000, sizeVramBytes: 6_000 })).toEqual({ kind: 'gpu', percent: 100 })
    expect(gpuOffload({ sizeBytes: 6_000, sizeVramBytes: 3_990 })).toEqual({ kind: 'partial', percent: 67 })
    expect(gpuOffload({ sizeBytes: 6_000, sizeVramBytes: 0 })).toEqual({ kind: 'cpu', percent: 0 })
  })

  it('stays silent when the server reports no sizes (older Ollama)', () => {
    expect(gpuOffload({ sizeBytes: null, sizeVramBytes: null })).toBeNull()
    expect(gpuOffload({ sizeBytes: 0, sizeVramBytes: 0 })).toBeNull()
  })
})
