import { describe, expect, it } from 'vitest'
import { fitModelToStage } from '../live2d-loader'

type FakeModel = {
  scale: { value: number; set: (v: number) => void }
  width: number
  height: number
  internalModel: { width: number; height: number }
}

function fakeModel(naturalWidth: number, naturalHeight: number): FakeModel {
  const model: FakeModel = {
    scale: {
      value: 1,
      set(v: number) {
        model.scale.value = v
      },
    },

    internalModel: { width: naturalWidth, height: naturalHeight },
    get width() {
      return naturalWidth * model.scale.value
    },
    get height() {
      return naturalHeight * model.scale.value
    },
  }
  return model
}

const fit = (m: FakeModel, w: number, h: number) => fitModelToStage(m as never, w, h)

describe('fitModelToStage', () => {
  const STAGE_W = 420
  const STAGE_H = 620

  it('enquadra um modelo pequeno dentro do palco', () => {
    const model = fakeModel(1024, 1024)
    fit(model, STAGE_W, STAGE_H)

    expect(model.height).toBeLessThanOrEqual(STAGE_H)
    expect(model.width).toBeLessThanOrEqual(STAGE_W)
  })

  it('enquadra um modelo grande igual a um pequeno, em vez de estourar', () => {
    const pequeno = fakeModel(1024, 1024)
    const grande = fakeModel(4096, 4096)
    fit(pequeno, STAGE_W, STAGE_H)
    fit(grande, STAGE_W, STAGE_H)

    expect(grande.height).toBeCloseTo(pequeno.height, 5)
    expect(grande.height).toBeLessThanOrEqual(STAGE_H)
  })

  it('respeita a largura do palco num modelo largo', () => {
    const largo = fakeModel(4000, 1000)
    fit(largo, STAGE_W, STAGE_H)

    expect(largo.width).toBeLessThanOrEqual(STAGE_W)
    expect(largo.height).toBeLessThanOrEqual(STAGE_H)
  })

  it('cai num fator seguro quando o modelo não reporta tamanho', () => {
    const semBounds = fakeModel(0, 0)
    fit(semBounds, STAGE_W, STAGE_H)

    expect(semBounds.scale.value).toBe(0.18)
  })
})
