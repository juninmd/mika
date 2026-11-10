import { describe, expect, it } from 'vitest'
import { DesktopHealthStatus } from '../desktop-health-status'

// Renderer and IPC both switch on these string values; a typo here silently breaks the health
// indicator across the whole app because TypeScript can't catch a wrong string literal downstream.
describe('DesktopHealthStatus', () => {
  it('exposes the four bootstrap phases as stable string values', () => {
    expect(DesktopHealthStatus.Initializing).toBe('initializing')
    expect(DesktopHealthStatus.Starting).toBe('starting')
    expect(DesktopHealthStatus.Degraded).toBe('degraded')
    expect(DesktopHealthStatus.Ok).toBe('ok')
  })
})
