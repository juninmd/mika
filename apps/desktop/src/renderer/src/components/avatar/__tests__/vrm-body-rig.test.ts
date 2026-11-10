import { describe, expect, it } from 'vitest'
import {
  ARM_REST,
  type ArmPose,
  armPoseFor,
  BODY_ACTION_SECONDS,
  isVrmBodyAction,
  VRM_BODY_ACTIONS,
  VrmArmDriver,
} from '../vrm-body-rig'

const BONES = ['leftUpperArm', 'rightUpperArm', 'leftLowerArm', 'rightLowerArm'] as const

const movedFromRest = (action: (typeof VRM_BODY_ACTIONS)[number], progress: number) =>
  BONES.filter((bone) => Math.abs(armPoseFor(action, progress)[bone] - ARM_REST[bone]) > 0.05)

// `wave`, `shrug` and `cheer` must move the VRM arm, not just the expression.
describe('rig de braço do VRM', () => {
  it('cada ação de corpo tira braço do repouso no meio do movimento', () => {
    for (const action of VRM_BODY_ACTIONS) {
      expect(movedFromRest(action, 0.5).length, `${action} não moveu nada`).toBeGreaterThan(0)
    }
  })

  it('acenar levanta só o braço direito', () => {
    const moved = movedFromRest('wave', 0.5)

    expect(moved).toContain('rightUpperArm')
    expect(moved).not.toContain('leftUpperArm')
  })

  it('comemorar levanta os dois', () => {
    const moved = movedFromRest('cheer', 0.5)

    expect(moved).toContain('leftUpperArm')
    expect(moved).toContain('rightUpperArm')
  })

  it('gestos próximos do corpo avançam os braços para as mãos ficarem visíveis', () => {
    for (const action of ['thinking', 'shy', 'laugh'] as const) {
      const pose = armPoseFor(action, 0.5)
      expect(pose.rightUpperArmX).toBeLessThan(-0.5)
      expect(pose.rightLowerArmY).toBeLessThan(-0.5)
    }
  })

  it('começa e termina exatamente no repouso, sem passo de volta', () => {
    for (const action of VRM_BODY_ACTIONS) {
      expect(armPoseFor(action, 0)).toEqual(ARM_REST)
      expect(armPoseFor(action, 1)).toEqual(ARM_REST)
    }
  })

  it('progresso fora do intervalo devolve repouso — o chamador não precisa zerar nada', () => {
    expect(armPoseFor('cheer', -0.4)).toEqual(ARM_REST)
    expect(armPoseFor('cheer', 3)).toEqual(ARM_REST)
  })

  it('só as ações que o rig conhece entram nele', () => {
    expect(isVrmBodyAction('wave')).toBe(true)
    expect(isVrmBodyAction('nod')).toBe(false)
    expect(isVrmBodyAction('happy')).toBe(false)
  })

  it('toda ação declarada tem duração, e nenhuma é instantânea', () => {
    for (const action of VRM_BODY_ACTIONS) {
      expect(BODY_ACTION_SECONDS[action]).toBeGreaterThan(0.5)
    }
  })
})

// Action clock lives outside the VRM adapter (which needs WebGL) so it can be tested without the app.
describe('relógio da ação de corpo', () => {
  it('sem ação em curso não devolve pose nenhuma — o adaptador não escreve osso à toa', () => {
    const arms = new VrmArmDriver()

    expect(arms.advance(0.016)).toBeNull()
    expect(arms.playing).toBe(false)
  })

  it('ação conhecida começa a tocar; ação de expressão não', () => {
    const arms = new VrmArmDriver()

    expect(arms.play('nod')).toBe(false)
    expect(arms.playing).toBe(false)

    expect(arms.play('wave')).toBe(true)
    expect(arms.playing).toBe(true)
  })

  it('termina sozinha no repouso e para de devolver pose', () => {
    const arms = new VrmArmDriver()
    arms.play('shrug')

    let last: ArmPose | null = null
    for (let i = 0; i < 200 && arms.playing; i++) last = { ...(arms.advance(0.016) as ArmPose) }

    expect(last).toEqual(ARM_REST)
    expect(arms.advance(0.016)).toBeNull()
  })

  it('interromper uma ação preserva a pose e reinicia o relógio com transição suave', () => {
    const arms = new VrmArmDriver()
    arms.play('cheer')
    const mid = { ...arms.advance(BODY_ACTION_SECONDS.cheer * 0.5) }
    arms.play('wave')
    expect(arms.advance(0)).toEqual(mid)
    const restarted = arms.advance(0.016)
    expect(Math.abs((restarted?.rightUpperArm ?? 0) - (mid?.rightUpperArm ?? 0))).toBeLessThan(0.01)
    expect(arms.advance(BODY_ACTION_SECONDS.wave)).toEqual(ARM_REST)
    expect(arms.playing).toBe(false)
  })

  it('reusa o mesmo buffer de pose a cada quadro em vez de alocar um novo', () => {
    const arms = new VrmArmDriver()
    arms.play('clap')
    const first = arms.advance(0.016)
    expect(arms.advance(0.016)).toBe(first)
  })

  it('não contamina a pose com delta inválido nem volta o relógio', () => {
    const arms = new VrmArmDriver()
    arms.play('wave')
    const pose = { ...arms.advance(1) }
    for (const dt of [-1, Number.NaN, Number.POSITIVE_INFINITY]) expect(arms.advance(dt)).toEqual(pose)
  })

  // Collected, then asserted once: ~45 actions × 60 FPS × every channel as individual `expect`s timed out.
  it('cada coreografia é contínua em todos os canais a 60 FPS', () => {
    const broken: string[] = []
    for (const action of VRM_BODY_ACTIONS) {
      let previous = ARM_REST
      const frames = Math.ceil(BODY_ACTION_SECONDS[action] * 60)
      for (let frame = 0; frame <= frames; frame++) {
        const pose = armPoseFor(action, frame / frames)
        for (const key of Object.keys(pose) as (keyof typeof pose)[]) {
          const step = Math.abs(pose[key] - previous[key])
          if (!Number.isFinite(pose[key]) || !(step < 0.12)) broken.push(`${action}.${key} quadro ${frame}: ${step}`)
        }
        previous = pose
      }
    }
    expect(broken).toEqual([])
  })

  it('gesto de paz estende dois dedos e articula o punho; reverência move o tronco', () => {
    const peace = armPoseFor('peace', 0.5)
    expect(peace.rightPeace).toBe(1)
    expect(peace.rightGrip).toBeGreaterThan(0.8)
    expect(peace.rightLowerArm).toBeLessThan(-1)
    expect(armPoseFor('bow', 0.5).chestX).toBeGreaterThan(0.3)
  })

  it('pose 22 levanta as duas mãos em paz com os polegares fechados', () => {
    const pose = armPoseFor('22', 0.5)
    expect(pose.leftPeace).toBe(1)
    expect(pose.rightPeace).toBe(1)
    expect(pose.leftGrip).toBeGreaterThan(0.8)
    expect(pose.rightGrip).toBeGreaterThan(0.8)
  })

  it('six-seven alterna a altura dos braços com contrapeso de tronco', () => {
    const leftBeat = armPoseFor('six-seven', 0.43)
    const rightBeat = armPoseFor('six-seven', 0.57)

    expect(Math.abs(leftBeat.leftUpperArm - rightBeat.leftUpperArm)).toBeGreaterThan(0.8)
    expect(Math.abs(leftBeat.rightUpperArm - rightBeat.rightUpperArm)).toBeGreaterThan(0.8)
    expect(Math.sign(leftBeat.spineZ)).not.toBe(Math.sign(rightBeat.spineZ))
  })

  it('floss balança os dois braços lateralmente contra o tronco', () => {
    const left = armPoseFor('floss', 0.45)
    const right = armPoseFor('floss', 0.55)

    expect(left.leftUpperArm - right.leftUpperArm).toBeGreaterThan(1)
    expect(left.rightUpperArm - right.rightUpperArm).toBeGreaterThan(1)
    expect(Math.sign(left.spineZ)).not.toBe(Math.sign(right.spineZ))
  })

  it('orange-justice combina braços cruzados, elevação e agachamento ritmado', () => {
    const low = armPoseFor('orange-justice', 0.38)
    const high = armPoseFor('orange-justice', 0.62)
    const crouch = armPoseFor('orange-justice', 0.3125)

    expect(Math.abs(low.leftUpperArm - high.leftUpperArm)).toBeGreaterThan(0.35)
    expect(crouch.hipsY).toBeLessThan(-0.09)
    expect(crouch.leftLowerLegX).toBeGreaterThan(0.45)
  })

  it('surprised treme a cabeça durante a pausa, não fica estático como um susto congelado', () => {
    const samples = [0.4, 0.5, 0.6].map((progress) => armPoseFor('surprised', progress).headX)
    const spread = Math.max(...samples) - Math.min(...samples)

    expect(spread).toBeGreaterThan(0.01)
  })

  it('point mira o cursor real: braço muda com a posição, outras ações ignoram os eixos extras', () => {
    const center = armPoseFor('point', 0.5)
    const left = armPoseFor('point', 0.5, -1, 0)
    const right = armPoseFor('point', 0.5, 1, 0)
    const up = armPoseFor('point', 0.5, 0, -1)
    expect(left.rightUpperArm).toBeGreaterThan(center.rightUpperArm)
    expect(right.rightUpperArm).toBeLessThan(center.rightUpperArm)
    expect(up.rightUpperArmX).toBeLessThan(center.rightUpperArmX)
    expect(armPoseFor('wave', 0.5, 1, 1)).toEqual(armPoseFor('wave', 0.5))
  })
})

// "Uma animação completa com espada, baseada no Shire, Senbonzakura do Byakuya."
describe('senbonzakura', () => {
  it('ergue o punho com a espada à frente do peito e segura a pose', () => {
    for (const progress of [0.34, 0.45]) {
      const pose = armPoseFor('senbonzakura', progress)
      expect(pose.rightUpperArmX, `${progress}`).toBeLessThan(ARM_REST.rightUpperArmX - 0.5)
      expect(pose.rightGrip, `${progress}`).toBeGreaterThan(0.8)
    }
  })

  it('depois de soltar a lâmina, a mão esquerda comanda as pétalas para frente', () => {
    expect(armPoseFor('senbonzakura', 0.4).leftUpperArm).toBeLessThan(ARM_REST.leftUpperArm + 0.3)
    expect(armPoseFor('senbonzakura', 0.65).leftUpperArm).toBeGreaterThan(ARM_REST.leftUpperArm + 0.9)
  })

  // "~4–5 s": long enough to read draw, hold and release, short enough not to stall the chat.
  it('dura o bastante para ler cada fase, sem travar a conversa', () => {
    expect(BODY_ACTION_SECONDS.senbonzakura).toBeGreaterThanOrEqual(4)
    expect(BODY_ACTION_SECONDS.senbonzakura).toBeLessThanOrEqual(5)
  })
})

// "Adicione mais poses e animações nas skins VRM": each new gesture has to read as itself mid-motion.
describe('gestos novos', () => {
  const at = (action: (typeof VRM_BODY_ACTIONS)[number], progress: number) => armPoseFor(action, progress)
  const spread = (values: number[]) => Math.max(...values) - Math.min(...values)
  const hold = (from = 0.36, to = 0.64, steps = 40) =>
    Array.from({ length: steps + 1 }, (_, i) => from + ((to - from) * i) / steps)

  it('palmas: mãos à frente do peito, abrindo e fechando espelhadas', () => {
    const mid = at('clap', 0.5)
    expect(mid.leftUpperArmX).toBeLessThan(-0.6)
    expect(mid.rightUpperArmX).toBeLessThan(-0.6)
    const right = hold().map((p) => at('clap', p).rightLowerArmY)
    const left = hold().map((p) => at('clap', p).leftLowerArmY)
    expect(spread(right)).toBeGreaterThan(0.2)
    expect(spread(left)).toBeGreaterThan(0.2)
    // Mirrored: when the right forearm swings in, the left swings in too (opposite sign).
    expect(Math.sign(right[3] - right[0])).toBe(-Math.sign(left[3] - left[0]))
  })

  it('joinha: punho fechado com o polegar estendido só na mão direita', () => {
    const pose = at('thumbs-up', 0.5)
    expect(pose.rightGrip).toBeGreaterThan(0.8)
    expect(pose.rightThumb).toBeGreaterThan(0.9)
    expect(at('wave', 0.5).rightThumb).toBe(0)
    expect(ARM_REST.rightThumb).toBe(0)
  })

  it('balançar a cabeça em "não" vira o rosto para os dois lados', () => {
    const yaw = hold().map((p) => at('headshake', p).headY)
    expect(Math.max(...yaw)).toBeGreaterThan(0.1)
    expect(Math.min(...yaw)).toBeLessThan(-0.1)
  })

  it('pulinho animado dobra os joelhos no ritmo, sem tirar os pés do lugar', () => {
    const hips = hold().map((p) => at('bounce', p).hipsY)
    expect(Math.min(...hips)).toBeLessThan(-0.03)
    expect(Math.max(...hips)).toBeGreaterThan(-0.01)
    expect(Math.max(...hold().map((p) => at('bounce', p).leftLowerLegX))).toBeGreaterThan(0.15)
  })

  it('desânimo curva o tronco e baixa a cabeça', () => {
    const pose = at('slump', 0.5)
    expect(pose.chestX).toBeGreaterThan(0.12)
    expect(pose.headX).toBeGreaterThan(0.2)
  })

  it('foco no jogo inclina para a tela com as duas mãos à frente segurando o controle', () => {
    const pose = at('focus', 0.5)
    expect(pose.chestX).toBeGreaterThan(0.08)
    expect(pose.leftUpperArmX).toBeLessThan(-0.5)
    expect(pose.rightUpperArmX).toBeLessThan(-0.5)
    expect(Math.min(pose.leftGrip, pose.rightGrip)).toBeGreaterThan(0.5)
  })

  it('dança curta balança o tronco para os dois lados', () => {
    const sway = hold(0.3, 0.7).map((p) => at('groove', p).spineZ)
    expect(Math.max(...sway)).toBeGreaterThan(0.05)
    expect(Math.min(...sway)).toBeLessThan(-0.05)
  })

  it('cochilo: a cabeça pesa, dá um tranco de volta e pesa de novo', () => {
    const head = hold(0.15, 0.85, 70).map((p) => at('doze', p).headX)
    const firstDroop = head.findIndex((v) => v > 0.25)
    expect(firstDroop).toBeGreaterThan(-1)
    const startle = head.findIndex((v, i) => i > firstDroop && v < 0.1)
    expect(startle).toBeGreaterThan(firstDroop)
    expect(head.findIndex((v, i) => i > startle && v > 0.2)).toBeGreaterThan(startle)
    expect(BODY_ACTION_SECONDS.doze).toBeGreaterThanOrEqual(5)
  })

  it('soquinho de vitória: punho fechado que desce com força e volta', () => {
    const lower = hold(0.25, 0.75, 60).map((p) => at('fist-pump', p))
    expect(Math.min(...lower.map((p) => p.rightGrip))).toBeGreaterThan(0.5)
    expect(spread(lower.map((p) => p.rightUpperArm))).toBeGreaterThan(0.4)
  })

  it('susto (gasp) leva as duas mãos à boca', () => {
    const pose = at('gasp', 0.45)
    expect(pose.leftUpperArmX).toBeLessThan(-0.8)
    expect(pose.rightUpperArmX).toBeLessThan(-0.8)
    expect(Math.abs(pose.leftLowerArm)).toBeGreaterThan(1.8)
    expect(Math.abs(pose.rightLowerArm)).toBeGreaterThan(1.8)
  })

  it('raiva: punhos cerrados tremendo', () => {
    const pose = at('rage', 0.5)
    expect(Math.min(pose.leftGrip, pose.rightGrip)).toBeGreaterThan(0.9)
    expect(spread(hold().map((p) => at('rage', p).rightLowerArm))).toBeGreaterThan(0.05)
  })

  it('tcharam: os dois braços abertos para cima com as mãos abertas', () => {
    const pose = at('tada', 0.5)
    expect(pose.leftUpperArm).toBeGreaterThan(ARM_REST.leftUpperArm + 0.8)
    expect(pose.rightUpperArm).toBeLessThan(ARM_REST.rightUpperArm - 0.8)
    expect(Math.max(pose.leftGrip, pose.rightGrip)).toBeLessThan(0.15)
  })
})
