import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { isCredentialConfigPath, isMaskedOnScreenConfigPath, SENSITIVE_CONFIG_PATHS } from '@mika/sdk/contracts'
import { describe, expect, it } from 'vitest'
import { isSensitivePath } from '../sensitive-paths'

const CREDENTIALS_PANEL = join(
  process.cwd(),
  'apps/desktop/src/renderer/src/components/settings/provider-credential-specs.ts',
)

type Declared = { configPath: string; type: string }

function declaredFields(): Declared[] {
  const source = readFileSync(CREDENTIALS_PANEL, 'utf8')
  const fields: Declared[] = []
  for (const match of source.matchAll(/configPath:\s*'([^']+)',[\s\S]{0,400}?\btype:\s*'([^']+)'/g)) {
    fields.push({ configPath: match[1] as string, type: match[2] as string })
  }
  return fields
}

// A field masked on screen but logged raw, or encrypted at rest but shown in clear, leaks a credential.
describe('credential fields vs sensitive paths', () => {
  it('parses every declared field of the credentials panel', () => {
    const source = readFileSync(CREDENTIALS_PANEL, 'utf8')
    const declaredCount = [...source.matchAll(/\bconfigPath:\s*'/g)].length

    expect(declaredCount).toBeGreaterThan(10)
    expect(declaredFields().map((field) => field.configPath)).toHaveLength(declaredCount)
  })

  it('redacts in logs every field the panel masks on screen', () => {
    const leaked = declaredFields()
      .filter((field) => field.type === 'password' && !isCredentialConfigPath(field.configPath))
      .map((field) => `${field.configPath} -> mascarado na tela, cru no log`)

    expect(leaked).toEqual([])
  })

  it('masks on screen exactly the paths the screen predicate names', () => {
    const diverged = declaredFields()
      .filter((field) => (field.type === 'password') !== isMaskedOnScreenConfigPath(field.configPath))
      .map((field) => `${field.configPath} -> type: '${field.type}'`)

    expect(diverged).toEqual([])
  })

  it('masks on screen every path encrypted at rest', () => {
    const leaked = SENSITIVE_CONFIG_PATHS.filter((path) => !isMaskedOnScreenConfigPath(path))
    expect(leaked).toEqual([])
  })

  it('redacts in logs every path encrypted at rest', () => {
    const leaked = SENSITIVE_CONFIG_PATHS.filter((path) => !isCredentialConfigPath(path))
    expect(leaked).toEqual([])
  })

  it('keeps the renderer predicate identical to the contract one', () => {
    for (const field of declaredFields()) {
      expect(isSensitivePath(field.configPath)).toBe(isCredentialConfigPath(field.configPath))
    }
  })

  it('redacts the paths whose divergence this test was written for', () => {
    for (const path of [
      'voice.azureTtsSubscriptionKey',
      'memory.redisUrl',
      'apiKeys.ollamaBaseUrl',
      'apiKeys.openaiCompatibleBaseUrl',
      'voice.openaiTtsBaseUrl',
    ]) {
      expect(isCredentialConfigPath(path), path).toBe(true)
    }
  })

  it('shows local service endpoints in the clear while still redacting them in logs', () => {
    for (const path of ['apiKeys.ollamaBaseUrl', 'apiKeys.openaiCompatibleBaseUrl', 'voice.openaiTtsBaseUrl']) {
      expect(isMaskedOnScreenConfigPath(path), path).toBe(false)
      expect(isCredentialConfigPath(path), path).toBe(true)
    }
  })

  it('keeps masking the Redis URL, which carries its password inline', () => {
    expect(isMaskedOnScreenConfigPath('memory.redisUrl')).toBe(true)
  })

  it('does not redact ordinary settings', () => {
    for (const path of ['voice.azureTtsRegion', 'llm.temperature', 'character.name', 'vision.enabled']) {
      expect(isCredentialConfigPath(path), path).toBe(false)
    }
  })
})
