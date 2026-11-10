import { describe, expect, it } from 'vitest'
import { ARM_REST } from '../vrm-body-rig'
import { bodyActionForExpression } from '../vrm-look'
import { applyIdleArmSway, applySpeechGesture } from '../vrm-pose'

describe('homologação da interatividade e pose do VRM', () => {
  it('pose de repouso (ARM_REST) elimina rigidez e flexiona cotovelos para a frente', () => {
    // Braços caídos próximos ao corpo
    expect(ARM_REST.leftUpperArm).toBeLessThan(-1.2)
    expect(ARM_REST.rightUpperArm).toBeGreaterThan(1.2)

    // Ombros relaxados à frente
    expect(ARM_REST.leftUpperArmX).toBeLessThan(0)
    expect(ARM_REST.rightUpperArmX).toBeLessThan(0)

    // Flexão sagital real dos cotovelos (antebraços projetados à frente)
    expect(ARM_REST.leftLowerArmY).toBeLessThan(-0.3)
    expect(ARM_REST.rightLowerArmY).toBeGreaterThan(0.3)

    // Mãos com dedos relaxados e curvados organicamente (não esticados duros a 0.12)
    expect(ARM_REST.leftGrip).toBeGreaterThanOrEqual(0.25)
    expect(ARM_REST.rightGrip).toBeGreaterThanOrEqual(0.25)
  })

  it('balanço contínuo (applyIdleArmSway) aplica respiração e inércia nos membros superiores', () => {
    const nodes: Record<string, { rotation: Record<string, number> }> = {
      leftUpperArm: { rotation: { z: 0, x: 0 } },
      rightUpperArm: { rotation: { z: 0, x: 0 } },
      leftLowerArm: { rotation: { y: 0 } },
      rightLowerArm: { rotation: { y: 0 } },
      leftHand: { rotation: { z: 0 } },
      rightHand: { rotation: { z: 0 } },
    }

    const fakeHumanoid = {
      getNormalizedBoneNode: (name: string) => nodes[name] ?? null,
    }

    applyIdleArmSway(fakeHumanoid as never, 1.0, 0.5)

    expect(nodes.leftUpperArm.rotation.z).not.toBe(0)
    expect(nodes.rightUpperArm.rotation.z).not.toBe(0)
    expect(nodes.leftLowerArm.rotation.y).not.toBe(0)
    expect(nodes.rightLowerArm.rotation.y).not.toBe(0)
    expect(nodes.leftHand.rotation.z).not.toBe(0)
  })

  it('expressões contextuais da conversa ativam gestos e linguagem corporal VRM', () => {
    expect(bodyActionForExpression('thinking')).toBe('thinking')
    expect(bodyActionForExpression('laugh')).toBe('laugh')
    expect(bodyActionForExpression('shy')).toBe('shy')
    expect(bodyActionForExpression('happy')).toBe('wave')
    expect(bodyActionForExpression('determined')).toBe('nod')
    expect(bodyActionForExpression('confused')).toBe('shrug')
    expect(bodyActionForExpression('smug')).toBe('peace')
    expect(['pout', 'rage']).toContain(bodyActionForExpression('angry'))
    expect(['facepalm', 'headshake']).toContain(bodyActionForExpression('disgusted'))
    expect(['sigh', 'slump']).toContain(bodyActionForExpression('sad'))
    expect(bodyActionForExpression('surprised')).toBe('surprised')
    expect(bodyActionForExpression('beckon')).toBe('beckon')
    expect(bodyActionForExpression('curious')).toBe('curious')
    expect(bodyActionForExpression('salute')).toBe('salute')
    expect(bodyActionForExpression('whisper')).toBe('whisper')
    expect(bodyActionForExpression('celebrate')).toBe('celebrate')
    expect(bodyActionForExpression('relaxed')).toBe('relaxed')
    expect(bodyActionForExpression('reading')).toBe('reading')
    expect(bodyActionForExpression('sitting')).toBe('sitting')
  })

  it('repertório expressivo do VRM cobre todas as ações anatômicas e durações', async () => {
    const { VRM_BODY_ACTIONS } = await import('@mika/sdk/contracts')
    const { POSES } = await import('../vrm-body-poses')
    const { BODY_ACTION_SECONDS } = await import('../vrm-body-rig')

    expect(VRM_BODY_ACTIONS.length).toBeGreaterThanOrEqual(25)
    for (const action of VRM_BODY_ACTIONS) {
      expect(POSES[action], `pose ausente para ${action}`).toBeDefined()
      expect(BODY_ACTION_SECONDS[action], `duração ausente para ${action}`).toBeGreaterThan(0)
    }
  })

  it('pose sitting articula pernas e quadris corretamente', async () => {
    const { POSES } = await import('../vrm-body-poses')
    const { addBodyPoseOffsets } = await import('../vrm-pose')
    const { ARM_REST } = await import('../vrm-body-rig')

    const sitPose = { ...ARM_REST, ...POSES.sitting }
    const legNodes: Record<string, { rotation: Record<string, number>; position?: { y: number } }> = {
      chest: { rotation: { x: 0, y: 0, z: 0 } },
      spine: { rotation: { x: 0, y: 0, z: 0 } },
      head: { rotation: { x: 0, y: 0, z: 0 } },
      leftUpperLeg: { rotation: { x: 0 } },
      rightUpperLeg: { rotation: { x: 0 } },
      leftLowerLeg: { rotation: { x: 0 } },
      rightLowerLeg: { rotation: { x: 0 } },
      hips: { rotation: { x: 0 }, position: { y: 0 } },
    }

    const fakeHumanoid = {
      getNormalizedBoneNode: (name: string) => legNodes[name] ?? null,
    }

    addBodyPoseOffsets(fakeHumanoid as never, sitPose)

    expect(legNodes.leftUpperLeg.rotation.x).toBeLessThan(-1)
    expect(legNodes.rightUpperLeg.rotation.x).toBeLessThan(-1)
    expect(legNodes.leftLowerLeg.rotation.x).toBeGreaterThan(1)
    expect(legNodes.rightLowerLeg.rotation.x).toBeGreaterThan(1)
    expect(sitPose.hipsY).toBeLessThan(0)

    expect(legNodes.leftUpperLeg.rotation.x).toBe(sitPose.leftUpperLegX)
    expect(legNodes.leftLowerLeg.rotation.x).toBe(sitPose.leftLowerLegX)
    const chestAfterFirstOffset = legNodes.chest.rotation.x
    legNodes.chest.rotation.x = 0
    addBodyPoseOffsets(fakeHumanoid as never, sitPose)
    expect(legNodes.chest.rotation.x).toBe(chestAfterFirstOffset)
  })

  it('gesticulação oratória (applySpeechGesture) move cabeça e membros ao falar', () => {
    const nodes: Record<string, { rotation: Record<string, number> }> = {
      head: { rotation: { x: 0, y: 0, z: 0 } },
      chest: { rotation: { x: 0, y: 0, z: 0 } },
      rightUpperArm: { rotation: { x: 0, z: 0 } },
      rightLowerArm: { rotation: { y: 0 } },
      rightHand: { rotation: { z: 0 } },
    }

    const fakeHumanoid = {
      getNormalizedBoneNode: (name: string) => nodes[name] ?? null,
    }

    applySpeechGesture(fakeHumanoid as never, true, 0.6, 1.5)

    expect(nodes.head.rotation.x).not.toBe(0)
    expect(nodes.chest.rotation.x).not.toBe(0)
    expect(nodes.rightUpperArm.rotation.x).not.toBe(0)
    expect(nodes.rightLowerArm.rotation.y).not.toBe(0)
  })
})
