import { describe, expect, it } from 'vitest'
import { ORIGINAL_LOOK, parseVrmModelPath, vrmModelPath } from '../vrm-look'

describe('Haru redesenhada', () => {
  it('config antiga carrega a nova geometria sem tingir novamente o uniforme', () => {
    expect(parseVrmModelPath('vrm/mika/mika.vrm#look=haru')).toEqual({
      url: 'vrm/mika/haru.vrm',
      look: ORIGINAL_LOOK,
    })
    expect(vrmModelPath('vrm/mika/mika.vrm', 'haru')).toBe('vrm/mika/haru.vrm')
  })

  it('não redireciona um modelo importado ou outra skin', () => {
    expect(parseVrmModelPath('custom.vrm#look=haru').url).toBe('custom.vrm')
    expect(vrmModelPath('vrm/mika/mika.vrm', 'mao')).toBe('vrm/mika/mika.vrm#look=mao')
  })
})
