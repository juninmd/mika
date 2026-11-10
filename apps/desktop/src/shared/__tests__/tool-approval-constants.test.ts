import { describe, expect, it } from 'vitest'
import { TOOL_APPROVAL_BACKSTOP_MS, TOOL_APPROVAL_COUNTDOWN_SECONDS } from '../tool-approval-constants'

describe('constantes de aprovação de tool — o backstop nunca vence a corrida contra o diálogo', () => {
  it('o backstop do IPC é estritamente maior que o contador do diálogo', () => {
    expect(TOOL_APPROVAL_BACKSTOP_MS).toBeGreaterThan(TOOL_APPROVAL_COUNTDOWN_SECONDS * 1000)
  })

  it('a margem do backstop é generosa o bastante para o round-trip do IPC (>= 3s)', () => {
    expect(TOOL_APPROVAL_BACKSTOP_MS - TOOL_APPROVAL_COUNTDOWN_SECONDS * 1000).toBeGreaterThanOrEqual(3000)
  })
})
