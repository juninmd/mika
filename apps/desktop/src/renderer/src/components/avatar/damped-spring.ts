/** Mola criticamente amortecida: parte do repouso sem tranco, chega ao alvo sem passar dele. */
export interface SpringState {
  value: number
  velocity: number
}

/** Solução fechada do passo (estável para qualquer `dt`); `omega` é a rigidez — ~4.7/omega s até 95%. */
export function springTo(state: SpringState, target: number, omega: number, dt: number): number {
  const offset = state.value - target
  const decay = Math.exp(-omega * dt)
  const pull = (state.velocity + omega * offset) * dt
  state.value = target + (offset + pull) * decay
  state.velocity = (state.velocity - omega * pull) * decay
  return state.value
}
