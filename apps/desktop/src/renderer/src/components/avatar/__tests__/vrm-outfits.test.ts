import { describe, expect, it } from 'vitest'
import { VRM_MODELS } from '../../live2d/model-path'
import { HARU_OUTFITS, meshVisibleInLook, ORIGINAL_LOOK, parseVrmModelPath, tintFor, vrmModelPath } from '../vrm-look'

// The outfits ship as hidden meshes inside haru.vrm: a skin that forgot to hide the blazer would
// render two garments through each other, and the plain Haru skin must never show any of them.
describe('trajes da Haru', () => {
  const byId = (id: string) => HARU_OUTFITS.find((look) => look.id === id)
  const outfitIds = ['seifuku', 'maid', 'idol']

  it('seifuku, maid e idol existem e carregam o haru.vrm pelo fragmento', () => {
    expect(HARU_OUTFITS.map((look) => look.id)).toEqual(outfitIds)
    for (const id of outfitIds) {
      const path = vrmModelPath('vrm/mika/mika.vrm', id)
      expect(path).toBe(`vrm/mika/haru.vrm#look=${id}`)
      expect(parseVrmModelPath(path)).toEqual({ url: 'vrm/mika/haru.vrm', look: byId(id) })
    }
  })

  it('a Haru original não mostra peça de traje nenhum e mantém o blazer', () => {
    const { look } = parseVrmModelPath('vrm/mika/haru.vrm')
    expect(look).toBe(ORIGINAL_LOOK)
    expect(meshVisibleInLook('Outfit_Seifuku_Collar', look)).toBe(false)
    expect(meshVisibleInLook('Outfit_Maid_HeadFrill', look)).toBe(false)
    expect(meshVisibleInLook('Haru_Skirt', look)).toBe(true)
    expect(meshVisibleInLook('Haru_Lapel', look)).toBe(true)
  })

  it('cada traje mostra só as próprias peças e troca a saia lápis pela dele', () => {
    for (const id of outfitIds) {
      const look = byId(id)
      if (!look) throw new Error(id)
      const own = `Outfit_${id[0]?.toUpperCase()}${id.slice(1)}_Skirt`
      expect(meshVisibleInLook(own, look), own).toBe(true)
      for (const other of outfitIds.filter((o) => o !== id)) {
        expect(meshVisibleInLook(`Outfit_${other[0]?.toUpperCase()}${other.slice(1)}_Skirt`, look)).toBe(false)
      }
      expect(meshVisibleInLook('Haru_Skirt', look), `${id}: saia lápis`).toBe(false)
      expect(meshVisibleInLook('Haru_SkirtHem', look), `${id}: barra da saia lápis`).toBe(false)
      expect(meshVisibleInLook('Haru_ScarfTail_1', look), `${id}: lenço`).toBe(false)
      // Sleeves and torso carry the arm and chest the body mesh no longer has: never hidden.
      expect(meshVisibleInLook('Haru_Blazer', look), `${id}: tronco`).toBe(true)
      expect(meshVisibleInLook('Haru_leftTailoredSleeve', look), `${id}: manga`).toBe(true)
      expect(tintFor('Haru_Blazer', look), `${id}: cor do tronco`).not.toBeNull()
    }
  })

  it('o painel lista os trajes com miniatura própria', () => {
    for (const id of outfitIds) {
      const model = VRM_MODELS.find((m) => m.id === `vrm-${id}`)
      expect(model?.path, id).toBe(`vrm/mika/haru.vrm#look=${id}`)
      expect(model?.preview, id).toBe(`vrm/mika/preview-${id}.png`)
    }
  })
})
