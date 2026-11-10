import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  DEV_CSP,
  DEV_CSP_DIRECTIVES,
  DEV_CSP_META,
  PROD_CSP,
  PROD_CSP_DIRECTIVES,
  PROD_CSP_META,
  TAURI_IPC_SOURCES,
  withTauriIpc,
} from '../csp-policy'

const META_IGNORED = ['frame-ancestors', 'report-uri', 'sandbox']

describe('CSP: cabeçalho e <meta> carregam o que cada canal sabe aplicar', () => {
  it('o cabeçalho mantém frame-ancestors (é ele que protege de clickjacking)', () => {
    expect(PROD_CSP).toContain("frame-ancestors 'none'")
    expect(DEV_CSP).toContain("frame-ancestors 'none'")
  })

  it('o <meta> não carrega nenhuma diretiva que o navegador ignora nesse canal', () => {
    for (const directive of META_IGNORED) {
      expect(PROD_CSP_META).not.toContain(directive)
      expect(DEV_CSP_META).not.toContain(directive)
    }
  })

  it('fora as ignoradas, <meta> e cabeçalho são a mesma política', () => {
    const expectedProd = PROD_CSP_DIRECTIVES.filter((d) => !META_IGNORED.some((i) => d.startsWith(i)))
    const expectedDev = DEV_CSP_DIRECTIVES.filter((d) => !META_IGNORED.some((i) => d.startsWith(i)))

    expect(PROD_CSP_META).toBe(expectedProd.join('; '))
    expect(DEV_CSP_META).toBe(expectedDev.join('; '))
  })

  it('o <meta> de produção continua trancando script-src em self', () => {
    expect(PROD_CSP_META).toContain("script-src 'self'")
    expect(PROD_CSP_META).toContain("default-src 'self'")
    expect(PROD_CSP_META).toContain("form-action 'none'")
  })

  /*
   * Regressão de 2026-09-21, achada só executando o app empacotado: a `<meta>` gerada pelo build
   * do renderer valia `connect-src 'self'` e a `tauri.conf.json` liberava `ipc:`. Com as duas
   * políticas presentes o navegador aplica a INTERSEÇÃO, então a meta vencia, todo `invoke` era
   * recusado, e o app abria com tema, i18n, config, histórico e health falhando em silêncio —
   * verde em lint, gates, typecheck, unit e build.
   */
  describe('o canal de IPC do Tauri sobrevive à interseção meta × cabeçalho', () => {
    it('withTauriIpc acrescenta as fontes de IPC ao connect-src, sem perder as que já havia', () => {
      const policy = withTauriIpc(PROD_CSP_META)

      // `connect-src` também carrega as fontes do esquema `mika-live2d` (ver `LIVE2D_SCHEME_SOURCES`)
      // — o regex prova que o IPC sobrevive à interseção sem exigir que ele seja o token seguinte.
      expect(policy).toMatch(/connect-src 'self'[^;]*ipc: http:\/\/ipc\.localhost/)
      expect(policy).toContain("connect-src 'self' http://mika-live2d.localhost mika-live2d:")
      expect(policy).toContain("script-src 'self'")
      expect(policy).toContain("form-action 'none'")
    })

    it('vale também para a política de desenvolvimento, que é a usada sob MIKA_E2E', () => {
      const policy = withTauriIpc(DEV_CSP_META)

      expect(policy).toMatch(/connect-src [^;]*ipc: http:\/\/ipc\.localhost/)
      expect(policy).toContain('ws://localhost:*')
    })

    it('a CSP da tauri.conf.json libera as MESMAS fontes — duas tabelas divergindo em silêncio é o furo', () => {
      const conf = JSON.parse(readFileSync(resolve(__dirname, '../../../src-tauri/tauri.conf.json'), 'utf8')) as {
        app: { security: { csp: string } }
      }
      const connectSrc = /connect-src ([^;]*)/.exec(conf.app.security.csp)?.[1]

      expect(connectSrc).toBeDefined()
      for (const source of TAURI_IPC_SOURCES.split(' ')) expect(connectSrc).toContain(source)
    })
  })
})
