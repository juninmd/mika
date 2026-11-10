import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { FEATURE_COMPONENT_HANDLED_FIELDS } from '../feature-component-fields'

const root = process.cwd()
const SETTINGS_DIR = join(root, 'apps/desktop/src/renderer/src/components/settings')
const CREDENTIALS_PANEL = join(SETTINGS_DIR, 'provider-credential-specs.ts')

const RENDER_EXEMPT: Record<string, readonly string[]> = {
  BehaviorPanel: ['character.slangDictionary'],
  GamerCompanionPanel: ['gamer-companion.momentCooldowns'],
}

const CENTRALIZED_IN_CREDENTIALS: Record<string, readonly string[]> = {
  EmbeddingProviderPanel: [
    'apiKeys.googleAI',
    'apiKeys.openrouter',
    'apiKeys.ollamaBaseUrl',
    'apiKeys.ollamaApiKey',
    'apiKeys.openaiCompatibleBaseUrl',
    'apiKeys.openaiCompatibleApiKey',
    'apiKeys.unslothBaseUrl',
    'apiKeys.unslothApiKey',
  ],
  GamerCompanionPanel: ['gamer-companion.steamApiKey'],

  LLMConnectionPanel: [
    'apiKeys.googleAI',
    'apiKeys.openrouter',
    'apiKeys.ollamaBaseUrl',
    'apiKeys.ollamaApiKey',
    'apiKeys.openaiCompatibleBaseUrl',
    'apiKeys.openaiCompatibleApiKey',
    'apiKeys.unslothBaseUrl',
    'apiKeys.unslothApiKey',
  ],
  STTProviderPanel: ['voice.groqApiKey', 'voice.openaiApiKey'],
  TTSProviderPanel: [
    'apiKeys.elevenLabs',
    'voice.azureTtsSubscriptionKey',
    'voice.googleTtsApiKey',
    'voice.azureTtsRegion',

    'voice.openaiTtsApiKey',
    'voice.openaiTtsBaseUrl',
  ],
}

const RELATIVE_IMPORT = /from\s+'(\.[^']+)'/g

function resolveLocalModule(fromFile: string, spec: string): string | null {
  const base = resolve(dirname(fromFile), spec)
  for (const candidate of [`${base}.tsx`, `${base}.ts`, join(base, 'index.tsx'), join(base, 'index.ts')]) {
    if (existsSync(candidate)) return candidate
  }
  return null
}

const fileCache = new Map<string, string>()

function readFile(file: string): string {
  const cached = fileCache.get(file)
  if (cached !== undefined) return cached
  const content = readFileSync(file, 'utf8')
  fileCache.set(file, content)
  return content
}

const CORPUS_CUT = new Set(['feature-components.tsx', 'SettingsPanelContent.tsx', 'feature-component-fields.ts'])

// [raiz, subpasta de UI dentro de cada contexto]: features têm `<ctx>/renderer/`; os painéis do
// núcleo moraram em `main/services/<ctx>/renderer/` e hoje moram direto em `renderer/settings/<ctx>/`.
const RENDERER_ROOTS: ReadonlyArray<[string, string]> = [
  [join(root, 'apps/desktop/src/features'), 'renderer'],
  [join(root, 'apps/desktop/src/renderer/settings'), ''],
]

function resolveComponentEntry(component: string): string | null {
  const shellEntry = join(SETTINGS_DIR, `${component}.tsx`)
  if (existsSync(shellEntry)) return shellEntry

  const exported = new RegExp(`export\\s+(?:function|const)\\s+${component}\\b`)
  const reexported = new RegExp(`export\\s*\\{[^}]*\\b${component}\\b`)
  for (const [featureRoot, uiDir] of RENDERER_ROOTS) {
    if (!existsSync(featureRoot)) continue
    for (const ctx of readdirSync(featureRoot, { withFileTypes: true })) {
      if (!ctx.isDirectory()) continue
      const rendererDir = join(featureRoot, ctx.name, uiDir)
      if (!existsSync(rendererDir)) continue
      const found = resolveInDir(rendererDir, component, exported, reexported)
      if (found) return found
    }
  }
  return null
}

function isSourceFile(name: string): boolean {
  return (name.endsWith('.tsx') || name.endsWith('.ts')) && !name.includes('.test.')
}

function resolveInDir(dir: string, component: string, exported: RegExp, reexported: RegExp): string | null {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name)
    if (entry.isDirectory()) {
      const found = entry.name === 'node_modules' ? null : resolveInDir(full, component, exported, reexported)
      if (found) return found
    } else if (isSourceFile(entry.name)) {
      const content = readFile(full)
      if (exported.test(content) || reexported.test(content)) return full
    }
  }
  return null
}

function componentCorpus(component: string): string | null {
  const entry = resolveComponentEntry(component)
  if (!entry) return null

  const seen = new Set<string>()
  const queue = [entry]
  const parts: string[] = []

  while (queue.length > 0) {
    const file = queue.shift() as string
    if (seen.has(file)) continue
    seen.add(file)
    const content = readFile(file)
    parts.push(content)
    for (const match of content.matchAll(RELATIVE_IMPORT)) {
      const target = resolveLocalModule(file, match[1] as string)
      if (!target) continue
      if (
        CORPUS_CUT.has(
          target
            .slice(target.lastIndexOf('\\') + 1)
            .split('/')
            .pop() as string,
        )
      )
        continue
      queue.push(target)
    }
  }

  return parts.join('\n')
}

function mentions(corpus: string, path: string): boolean {
  return corpus.includes(`'${path}'`) || corpus.includes(`"${path}"`) || corpus.includes(`\`${path}\``)
}

function unrenderedPaths(component: string, paths: readonly string[], corpus: string): string[] {
  const exempt = new Set([...(RENDER_EXEMPT[component] ?? []), ...(CENTRALIZED_IN_CREDENTIALS[component] ?? [])])
  return paths.filter((path) => !exempt.has(path) && !mentions(corpus, path)).map((path) => `${component} -> ${path}`)
}

describe('settings render coverage (list → JSX)', () => {
  it('tem alcance mínimo — não é indistinguível de um gate que não rodou', () => {
    const entries = Object.entries(FEATURE_COMPONENT_HANDLED_FIELDS)
    const totalFields = entries.reduce((sum, [, paths]) => sum + paths.length, 0)
    expect(entries.length).toBeGreaterThanOrEqual(15)
    expect(totalFields).toBeGreaterThanOrEqual(100)
  })

  it('renders every handled field inside the component that declares it', () => {
    const unrendered: string[] = []
    const missingFile: string[] = []

    for (const [component, paths] of Object.entries(FEATURE_COMPONENT_HANDLED_FIELDS)) {
      const corpus = componentCorpus(component)
      if (corpus === null) {
        if (paths.length > 0) missingFile.push(component)
        continue
      }
      unrendered.push(...unrenderedPaths(component, paths, corpus))
    }

    expect(missingFile).toEqual([])
    expect(unrendered).toEqual([])
  })

  it('keeps centralized credentials verifiable — every exempt credential is in ProviderCredentialsPanel', () => {
    const credentials = readFile(CREDENTIALS_PANEL)
    const orphans: string[] = []

    for (const [component, paths] of Object.entries(CENTRALIZED_IN_CREDENTIALS)) {
      for (const path of paths) {
        if (!mentions(credentials, path)) orphans.push(`${component} -> ${path}`)
      }
    }

    expect(orphans).toEqual([])
  })

  it('keeps the exemption lists tight — every exempt field is still declared', () => {
    const stale: string[] = []
    for (const table of [RENDER_EXEMPT, CENTRALIZED_IN_CREDENTIALS]) {
      for (const [component, paths] of Object.entries(table)) {
        const handled = new Set<string>(
          (FEATURE_COMPONENT_HANDLED_FIELDS as Record<string, readonly string[]>)[component] ?? [],
        )
        for (const path of paths) {
          if (!handled.has(path)) stale.push(`${component} -> ${path}`)
        }
      }
    }
    expect(stale).toEqual([])
  })

  it('keeps the credentials exemption honest — an exempt field must not already be in its own panel', () => {
    const stillLocal: string[] = []
    for (const [component, paths] of Object.entries(CENTRALIZED_IN_CREDENTIALS)) {
      const corpus = componentCorpus(component)
      if (!corpus) continue
      for (const path of paths) {
        if (mentions(corpus, path)) stillLocal.push(`${component} -> ${path}`)
      }
    }
    expect(stillLocal).toEqual([])
  })
})
