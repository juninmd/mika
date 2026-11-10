import { describe, expect, it } from 'vitest'
import { DESKTOP_RUNTIME_INVOKE_ROUTES } from '../desktop-ipc'

/** Zod-validated IPC route: an undeclared field silently vanishes, so `origin` must be schema-declared. */
describe('desktopToolExecuteRequestSchema — origin', () => {
  const schema = DESKTOP_RUNTIME_INVOKE_ROUTES.toolExecute.requestSchema

  it('aceita origin "turn"', () => {
    const result = schema.parse({ name: 'mood', args: {}, origin: 'turn' })
    expect(result).toMatchObject({ origin: 'turn' })
  })

  it('aceita origin "background"', () => {
    const result = schema.parse({ name: 'mood', args: {}, origin: 'background' })
    expect(result).toMatchObject({ origin: 'background' })
  })

  it('origin é opcional — chamador antigo sem o campo continua validando', () => {
    const result = schema.parse({ name: 'mood', args: {} })
    expect(result.origin).toBeUndefined()
  })

  it('rejeita valor fora do enum', () => {
    expect(() => schema.parse({ name: 'mood', args: {}, origin: 'proactive' })).toThrow()
  })
})
