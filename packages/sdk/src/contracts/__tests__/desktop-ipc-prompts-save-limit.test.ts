import { describe, expect, it } from 'vitest'
import { DESKTOP_RUNTIME_INVOKE_ROUTES, PROMPT_OVERRIDE_MAX_CHARS } from '../desktop-ipc'

// O override vira system prompt no próximo turno: sem teto na fronteira, um colar gigante
// ocupava a janela de contexto inteira e ninguém via o motivo.
describe('promptsSave — teto do texto', () => {
  const schema = DESKTOP_RUNTIME_INVOKE_ROUTES.promptsSave.requestSchema
  const base = { namespace: 'brain', key: 'system.md' }

  it('aceita texto exatamente no teto', () => {
    expect(schema.safeParse({ ...base, text: 'a'.repeat(PROMPT_OVERRIDE_MAX_CHARS) }).success).toBe(true)
  })

  it('rejeita texto acima do teto em vez de gravá-lo', () => {
    expect(schema.safeParse({ ...base, text: 'a'.repeat(PROMPT_OVERRIDE_MAX_CHARS + 1) }).success).toBe(false)
  })

  it('teto cobre com folga o maior prompt real (~11 KB)', () => {
    expect(PROMPT_OVERRIDE_MAX_CHARS).toBeGreaterThan(11_000 * 4)
  })
})
