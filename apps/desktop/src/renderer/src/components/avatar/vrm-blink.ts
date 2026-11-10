/** Piscada do VRM: pausa quando a expressão já fecha o olho e às vezes pisca duas vezes. */

export const BLINK_SEC = 0.15
/** A pálpebra desce em ~1/3 da piscada e sobe no resto, como a de verdade. */
const CLOSE_SHARE = 0.35
/** Acima disso, happy/relaxed já fecham o olho; somar a piscada empurra a pálpebra através da bochecha. */
export const EYES_BUSY_THRESHOLD = 0.5
const DOUBLE_BLINK_CHANCE = 0.18
const DOUBLE_BLINK_GAP = 0.09
/** Giro de cabeça (rad/s) a partir do qual a pessoa pisca junto: ~15° de mudança de direção. */
export const GAZE_BLINK_SPEED = 0.8
/** Depois de uma piscada, ela só antecipa a próxima passado este tempo: sem tremer a pálpebra. */
const GAZE_BLINK_COOLDOWN = 1
/** Intervalo em repouso: 2,8 s a 6,2 s, ~14 piscadas/min — a faixa de quem olha a tela calado. */
const REST_INTERVAL_MIN = 2.8
const REST_INTERVAL_SPREAD = 3.4
/** Conversando a pessoa pisca ~1,5x mais (≈ 22/min): o intervalo encolhe na mesma proporção. */
const SPEAKING_INTERVAL_SCALE = 0.65

export class BlinkDriver {
  private timer = 0
  private nextIn: number
  private doublePending = false
  private speaking = false

  constructor(private readonly random: () => number = Math.random) {
    this.nextIn = this.interval()
  }

  /**
   * Peso de `blink` (0..1). `eyesBusy` é o quanto a expressão atual já fecha os olhos; `gazeSpeed` é a
   * velocidade angular da cabeça — quem vira o rosto de uma vez pisca no meio do giro. `speaking`
   * acelera o ritmo; ao começar a falar, uma espera longa já sorteada é encurtada para o ritmo novo.
   */
  advance(dt: number, eyesBusy: number, gazeSpeed = 0, speaking = false): number {
    if (speaking && !this.speaking) this.nextIn = Math.min(this.nextIn, this.timer + this.interval(true))
    this.speaking = speaking
    if (eyesBusy >= EYES_BUSY_THRESHOLD) {
      this.timer = 0
      return 0
    }
    this.timer += dt
    if (gazeSpeed > GAZE_BLINK_SPEED && this.timer >= GAZE_BLINK_COOLDOWN && this.timer < this.nextIn) {
      this.nextIn = this.timer
    }
    const progress = (this.timer - this.nextIn) / BLINK_SEC
    if (progress < 0) return 0
    if (progress >= 1) {
      this.scheduleNext()
      return 0
    }
    return blinkCurve(progress)
  }

  private scheduleNext(): void {
    this.timer = 0
    this.doublePending = !this.doublePending && this.random() < DOUBLE_BLINK_CHANCE
    this.nextIn = this.doublePending ? DOUBLE_BLINK_GAP : this.interval()
  }

  private interval(speaking = this.speaking): number {
    const rest = REST_INTERVAL_MIN + this.random() * REST_INTERVAL_SPREAD
    return speaking ? rest * SPEAKING_INTERVAL_SCALE : rest
  }
}

const smoothstep = (t: number) => t * t * (3 - 2 * t)

/** Fecha rápido (ease-in), abre devagar (ease-out). */
export function blinkCurve(progress: number): number {
  if (progress < CLOSE_SHARE) return smoothstep(progress / CLOSE_SHARE)
  return 1 - smoothstep((progress - CLOSE_SHARE) / (1 - CLOSE_SHARE))
}
