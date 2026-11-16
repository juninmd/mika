import { describe, expect, it } from 'vitest'
import { dockVisibility } from '../dock-visibility'

const base = { autoHide: true, discreet: false, chatOpen: false, active: false, idle: true }

describe('dockVisibility', () => {
  it('collapses to the edge strip only when idle, inactive and auto-hide is on', () => {
    expect(dockVisibility(base)).toBe('strip')
    expect(dockVisibility({ ...base, idle: false })).toBe('open')
    expect(dockVisibility({ ...base, active: true })).toBe('open')
    expect(dockVisibility({ ...base, autoHide: false })).toBe('open')
  })

  it('hides completely in discreet mode, even while she is busy, unless the chat is open', () => {
    expect(dockVisibility({ ...base, discreet: true, active: true, idle: false })).toBe('hidden')
    expect(dockVisibility({ ...base, discreet: true, autoHide: false })).toBe('hidden')
    expect(dockVisibility({ ...base, discreet: true, chatOpen: true })).toBe('open')
  })

  it('never collapses an open chat', () => {
    expect(dockVisibility({ ...base, chatOpen: true })).toBe('open')
  })
})
