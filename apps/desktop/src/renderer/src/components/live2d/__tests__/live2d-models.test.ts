import { describe, expect, it } from 'vitest'
import { LIVE2D_ACTION_CATALOG } from '../live2d-action-catalog.generated'
import { AVATAR_MODELS, DEFAULT_MODEL_PATH, LIVE2D_MODELS } from '../model-path'

describe('catálogo de modelos Live2D', () => {
  it('oferece exatamente os modelos que o app embarca', () => {
    const catalogKeys = Object.keys(LIVE2D_ACTION_CATALOG).sort()
    const offered = LIVE2D_MODELS.map((model) => model.catalogKey).sort()
    expect(offered).toEqual(catalogKeys)
  })

  it('não repete id, rótulo nem caminho', () => {
    for (const field of ['id', 'label', 'path'] as const) {
      const values = LIVE2D_MODELS.map((model) => model[field])
      expect(new Set(values).size, field).toBe(values.length)
    }
  })

  it('aponta cada caminho para o .model3.json do próprio modelo', () => {
    for (const model of LIVE2D_MODELS) {
      expect(model.path, model.id).toMatch(/^live2d\/[^/]+\/.+\.model3\.json$/)
      expect(model.path.endsWith(`/${model.catalogKey}.model3.json`), model.id).toBe(true)
    }
  })

  it('mantém o modelo padrão dentro da lista', () => {
    expect(AVATAR_MODELS.some((model) => model.path === DEFAULT_MODEL_PATH)).toBe(true)
  })

  it('mantém a miniatura na pasta do próprio modelo', () => {
    for (const model of LIVE2D_MODELS) {
      if (!model.preview) continue
      const dir = model.path.slice(0, model.path.lastIndexOf('/'))
      expect(model.preview, model.id).toBe(`${dir}/preview.png`)
    }
  })
})
