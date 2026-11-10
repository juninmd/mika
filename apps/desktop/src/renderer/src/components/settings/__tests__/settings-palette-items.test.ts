import { setLocale, whenLocaleReady } from '@mika/sdk/i18n'
import { afterEach, describe, expect, it } from 'vitest'
import { buildItems } from '../settings-palette-items'

// Smoke 2026-09-27: Ctrl+K "backup", "credenciais", "dispositivos" came back empty — the built-in tabs
// are not feature declarations, so the palette never indexed them.
describe('paleta de configurações', () => {
  const declared = [{ tabId: 'modules', tabLabel: 'Módulos', groups: [] }]

  it('encontra as abas embutidas e abre a aba certa', () => {
    const opened: string[] = []
    const items = buildItems(declared, (id) => opened.push(id), 'Módulos')
    const backup = items.find((item) => item.label === 'Backup')
    expect(backup).toBeDefined()
    backup?.action()
    expect(opened).toEqual(['backup'])
    expect(items.some((item) => item.label === 'Dispositivos de Áudio')).toBe(true)
  })

  it('não duplica uma aba embutida que já veio declarada', () => {
    const items = buildItems(declared, () => {}, 'Módulos')
    expect(items.filter((item) => item.id === 'modules')).toHaveLength(1)
  })

  // Regression: the built-in tab names were a Portuguese constant resolved once, whatever the locale.
  describe('rótulos das abas embutidas', () => {
    afterEach(async () => {
      setLocale('pt-BR')
      await whenLocaleReady()
    })

    it('seguem o idioma ativo a cada chamada', async () => {
      setLocale('en-US')
      await whenLocaleReady()
      const labels = buildItems([], () => {}, 'Modules').map((item) => item.label)
      expect(labels).toContain('Credentials')
      expect(labels).not.toContain('Credenciais')
    })
  })
})
