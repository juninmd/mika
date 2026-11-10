import { describe, expect, it } from 'vitest'
import { EXPRESSION_ACTION_MAP } from '../../live2d/live2d-animator-config'
import { AVATAR_MODELS, LIVE2D_MODELS, VRM_MODELS } from '../../live2d/model-path'
import {
  ACTION_EXPRESSION,
  actionFaceWeight,
  EXPRESSION_PRESET,
  expressionPreset,
  HARU_OUTFITS,
  isVrmModelPath,
  ORIGINAL_LOOK,
  parseVrmModelPath,
  tintFor,
  VRM_LOOKS,
  VRM_PRESETS,
  vrmModelPath,
} from '../vrm-look'

describe('avatar VRM — parte pura', () => {
  it('reconhece modelo VRM só pela extensão', () => {
    expect(isVrmModelPath('vrm/mika/mika.vrm')).toBe(true)
    expect(isVrmModelPath('vrm/mika/mika.vrm#look=haru')).toBe(true)
    expect(isVrmModelPath('VRM/x.VRM ')).toBe(true)
    expect(isVrmModelPath('live2d/haru/haru_greeter_t05.model3.json')).toBe(false)
    expect(isVrmModelPath(null)).toBe(false)
  })

  it('toda expressão que o Live2D conhece tem preset VRM, e o preset existe no padrão VRM 1.0', () => {
    const allowed = new Set<string>([...VRM_PRESETS, 'neutral'])
    for (const expression of Object.keys(EXPRESSION_ACTION_MAP)) {
      // Poses de corpo trazem o preset por ACTION_EXPRESSION; cair em 'neutral' por omissão seria o defeito.
      expect(EXPRESSION_PRESET[expression] ?? ACTION_EXPRESSION[expression], expression).toBeDefined()
      expect(allowed.has(expressionPreset(expression)), expression).toBe(true)
    }
    expect(expressionPreset('inexistente')).toBe('neutral')
  })

  it('toda ação gerada do catálogo Live2D tem equivalente VRM', () => {
    const actions = new Set(Object.values(EXPRESSION_ACTION_MAP).flat())
    for (const action of actions) expect(ACTION_EXPRESSION[action], action).toBeDefined()
  })

  it('toda skin tinta cabelo, olhos e roupa do modelo de amostra e deixa pele e rosto intactos', () => {
    for (const look of VRM_LOOKS.filter((l) => l !== ORIGINAL_LOOK)) {
      for (const material of [
        'Hair_00_HAIR',
        'HairBack_00_HAIR',
        'EyeIris_00_EYE',
        'Tops_01_CLOTH',
        'Bottoms_01_CLOTH',
      ]) {
        expect(tintFor(material, look), `${look.id}:${material}`).not.toBeNull()
      }
      for (const material of [
        'Body_00_SKIN',
        'Face_00_SKIN',
        'FaceMouth_00_FACE',
        'EyeWhite_00_EYE',
        'EyeHighlight_00_EYE',
      ]) {
        expect(tintFor(material, look), `${look.id}:${material}`).toBeNull()
      }
    }
  })

  it('o modelo VRM entra na lista do painel sem colidir com os Live2D', () => {
    expect(AVATAR_MODELS).toHaveLength(LIVE2D_MODELS.length + VRM_MODELS.length)
    const ids = AVATAR_MODELS.map((m) => m.id)
    expect(new Set(ids).size).toBe(ids.length)
    for (const model of VRM_MODELS) {
      expect(isVrmModelPath(model.path), model.id).toBe(true)
      const { url, look } = parseVrmModelPath(model.path)
      // A skin haru virou um modelo próprio (47408f55): arquivo redesenhado, sem tinta por cima.
      // Os trajes (seifuku, maid, idol) são peças ocultas dentro do haru.vrm.
      const outfit = HARU_OUTFITS.some((l) => `vrm-${l.id}` === model.id)
      const expected =
        model.id === 'vrm-haru'
          ? { url: 'vrm/mika/haru.vrm', look: 'original' }
          : { url: outfit ? 'vrm/mika/haru.vrm' : 'vrm/mika/mika.vrm', look: model.id.slice(4) }
      expect({ url, look: look.id }, model.id).toEqual(expected)
      expect(model.preview, model.id).toBe(`vrm/mika/preview-${model.id.slice(4)}.png`)
    }
  })

  it('o fragmento #look escolhe a skin e o original não tinta nada', () => {
    expect(parseVrmModelPath('vrm/mika/mika.vrm').look).toBe(ORIGINAL_LOOK)
    expect(parseVrmModelPath('vrm/mika/mika.vrm#look=mao').look).toBe(VRM_LOOKS.find((l) => l.id === 'mao'))
    expect(parseVrmModelPath('vrm/mika/mika.vrm#look=haru')).toEqual({ url: 'vrm/mika/haru.vrm', look: ORIGINAL_LOOK })
    expect(parseVrmModelPath('vrm/mika/mika.vrm#look=nao-existe').look).toBe(ORIGINAL_LOOK)
    expect(vrmModelPath('vrm/mika/mika.vrm', 'original')).toBe('vrm/mika/mika.vrm')
    expect(vrmModelPath('vrm/mika/mika.vrm', 'rice')).toBe('vrm/mika/mika.vrm#look=rice')
    expect(ORIGINAL_LOOK.tints).toHaveLength(0)
  })

  // e91eed70 tirou os Live2D Hiyori/Mark/Rice e manteve as skins VRM deles: cobertura, não bijeção.
  it('há uma skin VRM para cada modelo Live2D embarcado, mais a original', () => {
    const lookIds = VRM_LOOKS.map((l) => l.id)
    expect(lookIds).toContain('original')
    for (const model of LIVE2D_MODELS) expect(lookIds, model.id).toContain(model.id)
    for (const look of VRM_LOOKS) {
      if (look.id === 'original') continue
      for (const material of ['Hair_00_HAIR', 'EyeIris_00_EYE', 'Tops_01_CLOTH']) {
        expect(tintFor(material, look), `${look.id}:${material}`).not.toBeNull()
      }
    }
  })

  it('expressões conversacionais disparam gestos corporais equivalentes no VRM', async () => {
    const { bodyActionForExpression, VRM_EXPRESSION_ACTIONS } = await import('../vrm-look')
    const { VRM_BODY_ACTIONS } = await import('../vrm-body-rig')

    expect(bodyActionForExpression('thinking')).toBe('thinking')
    expect(bodyActionForExpression('laugh')).toBe('laugh')
    expect(bodyActionForExpression('shy')).toBe('shy')
    expect(['cheer', 'victory', 'bounce']).toContain(bodyActionForExpression('excited'))
    expect(bodyActionForExpression('happy')).toBe('wave')
    expect(bodyActionForExpression('determined')).toBe('nod')
    expect(bodyActionForExpression('confused')).toBe('shrug')
    expect(bodyActionForExpression('smug')).toBe('peace')
    expect(bodyActionForExpression('neutral')).toBeNull()
    expect(bodyActionForExpression('inexistente')).toBeNull()

    const validActions = new Set<string>([...VRM_BODY_ACTIONS, 'nod'])
    for (const [expr, actions] of Object.entries(VRM_EXPRESSION_ACTIONS)) {
      for (const act of actions) {
        expect(validActions.has(act), `${expr} mapeou para ação desconhecida ${act}`).toBe(true)
      }
    }
  })

  it('pose pedida pelo nome da ação move o corpo em vez de cair em null', async () => {
    const { bodyActionForExpression, expressionPreset } = await import('../vrm-look')
    const { VRM_BODY_ACTIONS } = await import('../vrm-body-rig')

    for (const action of VRM_BODY_ACTIONS) {
      expect(bodyActionForExpression(action), `${action} não move o corpo`).not.toBeNull()
    }
    expect(bodyActionForExpression('nod')).toBe('nod')
    expect(expressionPreset('wave')).toBe('happy')
    expect(expressionPreset('bow')).toBe('relaxed')
  })
})

// At 0.32 every gesture played on a near-neutral face: a shy, a pout and a cheer looked alike.
describe('rosto das ações', () => {
  it('toda ação com expressão aparece no rosto num relance', () => {
    for (const [action, preset] of Object.entries(ACTION_EXPRESSION)) {
      if (preset === 'neutral') continue
      expect(actionFaceWeight(action), action).toBeGreaterThanOrEqual(0.5)
    }
  })

  it('gargalhada e susto pesam mais que um aceno', () => {
    expect(actionFaceWeight('laugh')).toBeGreaterThan(actionFaceWeight('wave'))
    expect(actionFaceWeight('surprised')).toBeGreaterThan(actionFaceWeight('wave'))
    expect(actionFaceWeight('laugh')).toBeLessThanOrEqual(1)
  })
})

describe('cansaço vira bocejo no corpo', () => {
  it('tired, sleep e sleepy disparam o bocejo, não só um suspiro', async () => {
    const { VRM_EXPRESSION_ACTIONS } = await import('../vrm-look')
    for (const emotion of ['tired', 'sleep', 'sleepy']) {
      expect(VRM_EXPRESSION_ACTIONS[emotion]).toContain('yawn')
    }
  })
})
