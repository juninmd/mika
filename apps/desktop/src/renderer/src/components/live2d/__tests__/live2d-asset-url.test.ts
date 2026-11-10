import { describe, expect, it } from 'vitest'
import { resolveLive2DAssetUrl } from '../live2d-loader'

describe('resolveLive2DAssetUrl', () => {
  it('em produção no Windows, usa o esquema custom na forma http://<esquema>.localhost', () => {
    const url = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', { platform: 'win32', isProd: true })
    expect(url).toBe('http://mika-live2d.localhost/haru/haru_greeter_t05.model3.json')
  })

  it('em produção fora do Windows (macOS/Linux), usa <esquema>://localhost', () => {
    const linux = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', { platform: 'linux', isProd: true })
    const mac = resolveLive2DAssetUrl('live2d/haru/haru_greeter_t05.model3.json', { platform: 'darwin', isProd: true })
    expect(linux).toBe('mika-live2d://localhost/haru/haru_greeter_t05.model3.json')
    expect(mac).toBe('mika-live2d://localhost/haru/haru_greeter_t05.model3.json')
  })

  it('normaliza barra e ./ iniciais antes de montar a URL', () => {
    const url = resolveLive2DAssetUrl('/live2d/Mao/Mao.model3.json', { platform: 'win32', isProd: true })
    expect(url).toBe('http://mika-live2d.localhost/Mao/Mao.model3.json')

    const relative = resolveLive2DAssetUrl('./live2d/Mao/Mao.model3.json', { platform: 'win32', isProd: true })
    expect(relative).toBe('http://mika-live2d.localhost/Mao/Mao.model3.json')
  })

  // O ramo `!isProd` chama `resolveRendererAssetUrl` (usa `document.baseURI`) — DOM é coberto
  // pelo E2E (`skin-boot`/`live2d-actions`), não por este ambiente `node` sem jsdom.
})
