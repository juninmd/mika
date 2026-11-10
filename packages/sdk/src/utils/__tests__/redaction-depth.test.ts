import { describe, expect, it } from 'vitest'
import { redactArgs, redactLogMeta } from '../redaction'

function nest(depth: number, leaf: Record<string, unknown>): Record<string, unknown> {
  let value: Record<string, unknown> = leaf
  for (let i = 0; i < depth; i++) value = { level: value }
  return value
}

const SECRET = 'sk-ant-live-abcdefghijklmnopqrstuvwxyz0123456789'

describe('redação além do limite de profundidade', () => {
  it('nunca devolve a subárvore crua: um apiKey no nível 8 não sai em texto puro', () => {
    const redacted = JSON.stringify(redactArgs(nest(8, { apiKey: SECRET })))
    expect(redacted).not.toContain(SECRET)
    expect(redacted).toContain('[TRUNCATED]')
  })

  it('vale também para meta de log', () => {
    const redacted = JSON.stringify(redactLogMeta(nest(8, { token: SECRET })))
    expect(redacted).not.toContain(SECRET)
  })

  it('dentro do limite continua redigindo campo a campo, sem truncar', () => {
    const redacted = redactArgs(nest(3, { apiKey: SECRET, note: 'ok' })) as Record<string, unknown>
    const text = JSON.stringify(redacted)
    expect(text).not.toContain(SECRET)
    expect(text).toContain('"note":"ok"')
    expect(text).not.toContain('[TRUNCATED]')
  })
})
