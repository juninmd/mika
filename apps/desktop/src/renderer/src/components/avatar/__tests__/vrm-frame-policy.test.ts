import { describe, expect, it } from 'vitest'
import { gateFrame, vrmFramePolicy } from '../vrm-frame-policy'

// Unfocused is the companion's normal state (over a game or editor): eco used to switch the idle
// poses off, so she stood still almost all the time.
describe('política de quadros do VRM', () => {
  it('sem foco ela continua viva, só com menos quadros', () => {
    const eco = vrmFramePolicy('eco')
    expect(eco.paused).toBe(false)
    expect(eco.idle).toBe(true)
    expect(eco.minFrameMs).toBeGreaterThan(vrmFramePolicy('full').minFrameMs)
  })

  it('pausada não anima nem dispara pose', () => {
    expect(vrmFramePolicy('paused')).toMatchObject({ paused: true, idle: false })
  })

  it('com foco roda a 60 quadros', () => {
    expect(vrmFramePolicy('full')).toMatchObject({ paused: false, idle: true })
    expect(Math.round(1000 / vrmFramePolicy('full').minFrameMs)).toBe(60)
  })
})

// `now - last < minFrameMs` then `last = now` threw away the leftover: on a 144 Hz monitor the
// 60 fps cap landed on every 3rd vsync (48 fps), and a 16.66 ms vsync under the 16.67 ms cap skipped.
describe('portão de quadros do VRM', () => {
  const rendered = (vsyncMs: number, capFps: number, seconds = 5, jitter = 0) => {
    let last = 0
    let frames = 0
    for (let i = 1; i * vsyncMs <= seconds * 1000; i++) {
      const now = i * vsyncMs + (i % 2 ? jitter : -jitter)
      const gate = gateFrame(now, last, 1000 / capFps)
      if (!gate.render) continue
      frames++
      last = gate.last
    }
    return frames / seconds
  }

  it('num monitor de 144 Hz o teto de 60 entrega ~60 quadros, não 48', () => {
    expect(rendered(1000 / 144, 60)).toBeGreaterThan(57)
  })

  it('a 60 Hz com vsync oscilando meio milissegundo nenhum quadro é pulado', () => {
    expect(rendered(1000 / 60, 60, 5, 0.5)).toBeGreaterThan(59)
  })

  it('o teto eco continua segurando a taxa', () => {
    expect(rendered(1000 / 144, 15)).toBeLessThan(15.5)
  })

  it('depois de uma pausa longa não dispara uma rajada de quadros atrasados', () => {
    const gate = gateFrame(10_000, 0, 1000 / 60)
    expect(gate.render).toBe(true)
    expect(gateFrame(10_005, gate.last, 1000 / 60).render).toBe(false)
  })
})
