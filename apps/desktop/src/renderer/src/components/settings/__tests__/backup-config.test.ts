import { describe, expect, it } from 'vitest'
import {
  backupFormatFor,
  buildBackupPayload,
  flattenConfigFields,
  parseBackupPayload,
  parseBackupText,
  serializeBackup,
} from '../backup-config'

describe('flattenConfigFields', () => {
  it('achata em pares namespace.campo, um nível', () => {
    const fields = flattenConfigFields({
      character: { name: 'Mika', energyLevel: 5 },
      llm: { provider: 'ollama', smartRouter: { lightModel: 'a' } },
    })

    expect(fields).toEqual([
      { path: 'character.name', value: 'Mika' },
      { path: 'character.energyLevel', value: 5 },
      { path: 'llm.provider', value: 'ollama' },
      { path: 'llm.smartRouter', value: { lightModel: 'a' } },
    ])
  })

  it('ignora __configMigrations — estado interno, não preferência do usuário', () => {
    const fields = flattenConfigFields({ __configMigrations: { foo: true }, app: { locale: 'pt-BR' } })
    expect(fields).toEqual([{ path: 'app.locale', value: 'pt-BR' }])
  })

  it('ignora namespaces que não são objeto', () => {
    const fields = flattenConfigFields({ orphanFlag: true, character: { name: 'Mika' } })
    expect(fields).toEqual([{ path: 'character.name', value: 'Mika' }])
  })
})

describe('buildBackupPayload', () => {
  const config = {
    character: { name: 'Mika' },
    apiKeys: { googleAI: 'secret-key' },
    voice: { groqApiKey: 'another-secret', volume: 80 },
  }

  it('por padrão exclui credenciais (apiKeys.* e campos que terminam em Key)', () => {
    const payload = buildBackupPayload(config, false)
    expect(payload.includesSecrets).toBe(false)
    expect(payload.fields).toEqual({ 'character.name': 'Mika', 'voice.volume': 80 })
  })

  it('com includeSecrets=true, inclui as credenciais', () => {
    const payload = buildBackupPayload(config, true)
    expect(payload.includesSecrets).toBe(true)
    expect(payload.fields).toEqual({
      'character.name': 'Mika',
      'apiKeys.googleAI': 'secret-key',
      'voice.groqApiKey': 'another-secret',
      'voice.volume': 80,
    })
  })
})

describe('parseBackupPayload', () => {
  it('devolve os campos de um payload válido', () => {
    expect(parseBackupPayload({ fields: { 'character.name': 'Mika' } })).toEqual({ 'character.name': 'Mika' })
  })

  it('rejeita algo que não é objeto', () => {
    expect(() => parseBackupPayload('not-json-object')).toThrow(/não é um objeto JSON/)
  })

  it('rejeita objeto sem "fields"', () => {
    expect(() => parseBackupPayload({ version: 1 })).toThrow(/campo "fields"/)
  })

  it('rejeita "fields" que não é objeto', () => {
    expect(() => parseBackupPayload({ fields: 'nope' })).toThrow(/campo "fields"/)
  })

  it('rejeita "fields" como array', () => {
    expect(() => parseBackupPayload({ fields: [] })).toThrow(/campo "fields"/)
  })
})

describe('backup em TOML', () => {
  const config = {
    character: { name: 'Mika', energyLevel: 5 },
    'gamer-companion': { steamApiKey: 'steam-secret', enabled: true },
    llm: { provider: 'ollama', smartRouter: { lightModel: 'a' }, fallback: null },
  }

  it('agrupa por namespace em tabelas [fields.<ns>], legível à mão', () => {
    const text = serializeBackup(buildBackupPayload(config, false), 'toml')
    expect(text).toMatch(/^\[fields\.character\]$/m)
    expect(text).toMatch(/^name = "Mika"$/m)
    expect(text).not.toContain('steam-secret')
  })

  it('ida e volta preserva cada campo; null (que TOML não tem) é omitido e não sobrescreve nada', () => {
    const payload = buildBackupPayload(config, true)
    const fields = parseBackupText(serializeBackup(payload, 'toml'), 'mika.toml')
    const { 'llm.fallback': _dropped, ...expected } = payload.fields
    expect(fields).toEqual(expected)
  })

  it('continua lendo o .json dos backups antigos', () => {
    const payload = buildBackupPayload(config, false)
    expect(parseBackupText(serializeBackup(payload, 'json'), 'old.json')).toEqual(payload.fields)
  })

  it('arquivo sem extensão conhecida: decide pelo conteúdo', () => {
    expect(backupFormatFor('backup', '{"fields":{}}')).toBe('json')
    expect(backupFormatFor('backup', 'version = 1')).toBe('toml')
  })

  it('TOML quebrado vira erro de leitura, não exceção crua do parser', () => {
    expect(() => parseBackupText('[fields\nname = ', 'x.toml')).toThrow(/inválido/i)
  })
})
