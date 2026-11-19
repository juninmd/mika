import { describe, expect, it } from 'vitest'
import { resolveLive2DAssetUrl } from '../live2d-loader'

describe('resolveLive2DAssetUrl', () => {
  it('no Windows, usa o esquema custom na forma http://<esquema>.localhost', () => {
    const url = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', 'win32')
    expect(url).toBe('http://mika-live2d.localhost/haru/haru_greeter_t05.model3.json')
  })

  it('fora do Windows (macOS/Linux), usa <esquema>://localhost', () => {
    const linux = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', 'linux')
    const mac = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', 'darwin')
    expect(linux).toBe('mika-live2d://localhost/haru/haru_greeter_t05.model3.json')
    expect(mac).toBe('mika-live2d://localhost/haru/haru_greeter_t05.model3.json')
  })

  it('normaliza barra e ./ iniciais antes de montar a URL', () => {
    const url = resolveLive2DAssetUrl('/live2d/Mao/Mao.model3.json', 'win32')
    expect(url).toBe('http://mika-live2d.localhost/Mao/Mao.model3.json')

    const relative = resolveLive2DAssetUrl('./live2d/Mao/Mao.model3.json', 'win32')
    expect(relative).toBe('http://mika-live2d.localhost/Mao/Mao.model3.json')
  })

  // O import grava no perfil, nunca em `public/`: um VRM importado tem de sair pelo mesmo esquema.
  it('serve um VRM importado pelo esquema, não pelo bundle', () => {
    expect(resolveLive2DAssetUrl('live2d/minha-skin/minha-skin.vrm', 'linux')).toBe(
      'mika-live2d://localhost/minha-skin/minha-skin.vrm',
    )
  })
})
