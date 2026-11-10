import { BODY_ACTION_SECONDS, isVrmBodyAction } from './vrm-body-rig'
import { ACTION_EXPRESSION } from './vrm-look'

const ACTION_PULSE_SEC = 1.6

export class VrmActionClock {
  preset: string | null = null
  name = ''
  remaining = 0
  duration = ACTION_PULSE_SEC

  start(name: string): boolean {
    const preset = ACTION_EXPRESSION[name]
    if (!preset) return false
    this.preset = preset
    this.name = name
    this.duration = isVrmBodyAction(name) ? BODY_ACTION_SECONDS[name] : ACTION_PULSE_SEC
    this.remaining = this.duration
    return true
  }

  tick(dt: number): void {
    this.remaining = Math.max(0, this.remaining - dt)
  }

  /** 0→1 through the running action; 1 once it has ended. */
  get progress(): number {
    return this.remaining > 0 ? 1 - this.remaining / this.duration : 1
  }

  get pulse(): number {
    return this.remaining > 0 ? Math.sin((this.remaining / this.duration) * Math.PI) : 0
  }

  isPlaying(name: string): boolean {
    return this.name === name && this.remaining > 0
  }
}
