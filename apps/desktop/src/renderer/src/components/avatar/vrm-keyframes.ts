/** Curvas puras da coreografia VRM: sem three.js, sem alocação por quadro. */

/** Quintic smoothstep: velocidade e aceleração nulas nas pontas. */
export function easePose(value: number): number {
  const t = Math.max(0, Math.min(1, value))
  return t * t * t * (t * (t * 6 - 15) + 10)
}

/** 0 → 1 → 0 com derivada nula nas pontas: o sino de um micro-gesto; zero fora de [0, 1]. */
export function bell(progress: number): number {
  if (!(progress > 0 && progress < 1)) return 0
  return Math.sin(progress * Math.PI) ** 2
}

/** `[progresso, valor]` em ordem crescente de progresso; declarado uma vez, fora do quadro. */
export type Keyframes = readonly (readonly [number, number])[]

/**
 * Valor no `progress`, suavizado entre chaves vizinhas (cada trecho sai e chega parado — o "ease in/out"
 * de um animador). Antes da primeira chave e depois da última, segura o valor da ponta.
 */
export function sampleKeys(keys: Keyframes, progress: number): number {
  const first = keys[0]
  if (!(progress > first[0])) return first[1]
  for (let i = 1; i < keys.length; i++) {
    const to = keys[i]
    if (progress <= to[0]) {
      const from = keys[i - 1]
      return from[1] + (to[1] - from[1]) * easePose((progress - from[0]) / (to[0] - from[0]))
    }
  }
  return keys[keys.length - 1][1]
}

/**
 * Entrada de um balanço periódico: 0 → 1 com derivada nula nas pontas, a partir de `start` (progresso).
 * Sem ela o seno nasce com a velocidade máxima no instante em que a pose chega, e o braço "arranca".
 */
export function swell(progress: number, start = 0.3, span = 0.14): number {
  return easePose((progress - start) / span)
}

/**
 * Seno retificado e suave (sin²): mesmo período e faixa 0..1 de |sin|, sem a quina nos zeros. A quina
 * era uma aceleração de ~450 rad/s² no joelho/antebraço — o "quique de boneco" dos gestos rítmicos.
 */
export function hop(x: number): number {
  const s = Math.sin(x)
  return s * s
}
