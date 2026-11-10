import { describe, expect, it } from 'vitest'
import { listModelParameters, resolveLipSyncParams } from '../live2d-animator-parameter-probe'

type FakeModel = Parameters<typeof resolveLipSyncParams>[0]

function modelWith(internal: Record<string, unknown>): FakeModel {
  return { internalModel: internal } as unknown as FakeModel
}

describe('resolveLipSyncParams', () => {
  it('usa os ids que o runtime da Cubism já resolveu do grupo LipSync', () => {
    expect(resolveLipSyncParams(modelWith({ lipSyncIds: ['ParamA'] }))).toEqual(['ParamA'])
  })

  it('cai para as settings do model3.json quando o runtime não resolveu', () => {
    const model = modelWith({
      lipSyncIds: [],
      settings: { getLipSyncParameters: () => ['ParamMouthOpenY'] },
    })

    expect(resolveLipSyncParams(model)).toEqual(['ParamMouthOpenY'])
  })

  it('devolve vazio quando o modelo não declara nada — quem decide o fallback é o animador', () => {
    expect(resolveLipSyncParams(modelWith({}))).toEqual([])
    expect(resolveLipSyncParams(modelWith({ lipSyncIds: [], settings: {} }))).toEqual([])
  })

  it('ignora entradas que não são string', () => {
    expect(resolveLipSyncParams(modelWith({ lipSyncIds: ['ParamA', 42, null] }))).toEqual(['ParamA'])
  })
})

describe('listModelParameters', () => {
  it('lê os ids do modelo interno (Cubism 4)', () => {
    const model = modelWith({ coreModel: { _model: { parameters: { ids: ['ParamA', 'ParamAngleX'] } } } })

    expect(listModelParameters(model)).toEqual(['ParamA', 'ParamAngleX'])
  })

  it('lê por índice quando só há a API de contagem (Cubism 2)', () => {
    const ids = ['PARAM_MOUTH_OPEN_Y', 'PARAM_ANGLE_X']
    const model = modelWith({
      coreModel: {
        getParameterCount: () => ids.length,
        getParameterId: (index: number) => ids[index],
      },
    })

    expect(listModelParameters(model)).toEqual(ids)
  })

  it('devolve vazio quando o modelo não expõe nenhuma das duas APIs', () => {
    expect(listModelParameters(modelWith({ coreModel: {} }))).toEqual([])
  })
})
