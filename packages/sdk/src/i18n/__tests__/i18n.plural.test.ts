import { afterEach, describe, expect, it } from 'vitest'
import { SUPPORTED_LOCALES } from '../../contracts'
import { loadLocale, resolvePlural, setLocale, t, translations, whenLocaleReady } from '../index'

// Chave de plural real do catálogo: o teste prova o contrato de `t(key, {count})` sobre ela.
const KEY = 'settings.mcp.card.toolCount' as const

describe('t(key, { count }) — plural por Intl.PluralRules', () => {
  afterEach(async () => {
    setLocale('pt-BR')
    await whenLocaleReady()
  })

  it('en-US: 1 é singular, o resto é plural', () => {
    expect(t(KEY, { count: 1 }, 'en-US')).toBe('1 tool')
    expect(t(KEY, { count: 0 }, 'en-US')).toBe('0 tools')
    expect(t(KEY, { count: 7 }, 'en-US')).toBe('7 tools')
  })

  it('pt-BR: 0 e 1 são singular (regra do português), 2 em diante plural', () => {
    expect(t(KEY, { count: 0 }, 'pt-BR')).toBe('0 ferramenta')
    expect(t(KEY, { count: 1 }, 'pt-BR')).toBe('1 ferramenta')
    expect(t(KEY, { count: 2 }, 'pt-BR')).toBe('2 ferramentas')
  })

  it('ja-JP não tem plural: toda contagem usa a forma única', async () => {
    await loadLocale('ja-JP')
    expect(t(KEY, { count: 1 }, 'ja-JP')).toBe(t(KEY, { count: 5 }, 'ja-JP').replace('5', '1'))
  })

  it('sem locale explícito usa o corrente', async () => {
    setLocale('en-US')
    expect(t(KEY, { count: 1 })).toBe('1 tool')
  })

  it('chave sem variantes de plural continua valendo, interpolando as outras variáveis', () => {
    expect(t('shell.palette.toolError', { label: 'X' }, 'en-US')).toBe('Could not run "X"')
  })

  it('assinatura antiga t(key, locale) segue igual', () => {
    expect(t('button.save', 'pt-BR')).toBe('Salvar')
  })
})

describe('resolvePlural — escolha da forma', () => {
  it('ru: one/few/many seguem a regra eslava', () => {
    const ru = { k: 'outro', 'k.one': 'um', 'k.few': 'poucos', 'k.many': 'muitos' }
    const forms = [1, 2, 5, 11, 21, 22, 25].map((count) => resolvePlural(ru, 'k', count, 'ru-RU'))
    expect(forms).toEqual(['um', 'poucos', 'muitos', 'muitos', 'um', 'poucos', 'muitos'])
  })

  it('categoria sem variante cai na forma base', () => {
    expect(resolvePlural({ k: 'base' }, 'k', 1, 'en-US')).toBe('base')
    expect(resolvePlural({}, 'k', 1, 'en-US')).toBeUndefined()
  })
})

/** Categorias que um inteiro de 0 a 200 de fato usa no idioma (`other` é a chave base). */
function usedCategories(locale: string): Set<string> {
  const rules = new Intl.PluralRules(locale)
  const used = new Set<string>()
  for (let n = 0; n <= 200; n++) used.add(rules.select(n))
  used.delete('other')
  return used
}

const VARIANTS = ['one', 'few', 'many'] as const

/** Variantes `<base>.<categoria>` que o catálogo define. */
function variantsOf(dict: Record<string, string>, bases: string[]): string[] {
  return bases.flatMap((base) => VARIANTS.filter((c) => `${base}.${c}` in dict).map((c) => `${base}.${c}`))
}

describe('catálogos — variantes de plural', () => {
  async function plurals() {
    await Promise.all(SUPPORTED_LOCALES.map((locale) => loadLocale(locale)))
    const en = translations['en-US'] as Record<string, string>
    const bases = Object.keys(en)
      .filter((key) => key.endsWith('.one'))
      .map((key) => key.slice(0, -'.one'.length))
    return {
      bases,
      dict: (locale: string) => translations[locale as keyof typeof translations] as Record<string, string>,
    }
  }

  // Variante que a regra do idioma nunca seleciona é chave morta que ninguém mantém ("ja .one").
  it('nenhum locale define variante que o seu idioma nunca usa, e toda variante tem a forma base', async () => {
    const { bases, dict } = await plurals()
    expect(bases.length).toBeGreaterThan(0)
    const problems: string[] = []
    for (const locale of SUPPORTED_LOCALES) {
      const used = usedCategories(locale)
      for (const variant of variantsOf(dict(locale), bases)) {
        const [base, category] = [
          variant.slice(0, variant.lastIndexOf('.')),
          variant.slice(variant.lastIndexOf('.') + 1),
        ]
        if (!used.has(category)) problems.push(`${locale} ${variant} nunca é selecionada`)
        if (!(base in dict(locale))) problems.push(`${locale} ${variant} sem a forma base`)
      }
    }
    expect(problems).toEqual([])
  })

  // Os catálogos migrados à mão trazem a variante singular de toda chave de plural: sem ela a UI
  // mostraria "1 tools". (ko/ru/de escolhem entre variantes e frase invariável, então ficam de fora.)
  it.each(['en-US', 'pt-BR', 'es-ES', 'fr-FR', 'it-IT'])('%s traz .one para toda chave de plural', async (locale) => {
    const { bases, dict } = await plurals()
    const missing = bases.filter((base) => !(`${base}.one` in dict(locale)))
    expect(missing).toEqual([])
  })
})
