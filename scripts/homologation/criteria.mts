/**
 * As provas de homologação, na ordem em que falham mais cedo. Cada função responde UMA pergunta e
 * devolve status + evidência; quem decide se a resposta reprova é o perfil (`profile.mts`).
 */
import { spawnSync } from 'node:child_process'
import { resolve as resolvePath } from 'node:path'
import { pathToFileURL } from 'node:url'
import { exists, isSource, isTest, REPO_ROOT, read, sourceRoots, walk } from '../checks/lib.mts'
import type { CriterionResult } from './profile.mts'

const run = (args: string[]) =>
  spawnSync('pnpm', args, { cwd: REPO_ROOT, encoding: 'utf8', shell: process.platform === 'win32' })

const firstLine = (err: unknown): string => (err instanceof Error ? err.message.split('\n')[0] : String(err))

/** 1. Fronteira: o manifesto bate com o que o código faz. */
export function fronteira(id: string): CriterionResult {
  const gate = run(['-s', 'tsx', 'scripts/checks/module-manifest.mts', '--only', id])
  return gate.status === 0
    ? { id: 'fronteira', status: 'ok', detail: 'manifesto × código' }
    : { id: 'fronteira', status: 'falhou', detail: 'manifesto × código', evidence: gate.stdout.trim() }
}

// 2. Vivo: alguém fora do módulo o referencia (inclui import dinâmico preguiçoso do roster de features).
// Conta também referência por STRING DE CAMINHO: o preload não é importado, é apontado por
// `preload: join(__dirname, '../preload/index.js')`. Só `import`/`from` dava vivo=falhou num módulo
// que quatro janelas carregam — vermelho permanente que o veredito aprendeu a tolerar.
export function vivo(source: string): CriterionResult {
  const outside = sourceRoots()
    .flatMap((root) => walk(root, isSource))
    .filter((rel) => !rel.startsWith(`${source}/`) && !isTest(rel))
  const folder = source.split('/').pop() ?? ''
  const isPathRef = (spec: string) => spec.split('/').includes(folder) && /\.(?:js|ts|mjs|cjs)$/.test(spec)
  const importers = outside.filter((rel) => {
    const source = read(rel)
    const imports = [...source.matchAll(/(?:from|import\()\s*'([^']+)'/g)]
    if (imports.some((m) => m[1].split('/').includes(folder))) return true
    return [...source.matchAll(/'([^']+)'/g)].some((m) => isPathRef(m[1]))
  })
  return importers.length > 0
    ? { id: 'vivo', status: 'ok', detail: `${importers.length} arquivo(s) fora do módulo o referenciam` }
    : { id: 'vivo', status: 'falhou', detail: 'ninguém importa este módulo fora dele mesmo' }
}

/** 3. Headless: o contrato inteiro carrega sem Electron. */
export async function headless(contract: readonly string[]): Promise<CriterionResult> {
  const broken: string[] = []
  for (const file of contract) {
    try {
      await import(pathToFileURL(resolvePath(REPO_ROOT, file)).href)
    } catch (err) {
      broken.push(`${file} — ${firstLine(err)}`)
    }
  }
  return broken.length === 0
    ? { id: 'headless', status: 'ok', detail: `${contract.length} arquivo(s) do contrato fora do Electron` }
    : {
        id: 'headless',
        status: 'falhou',
        detail: `${broken.length} de ${contract.length} só carregam no app`,
        evidence: broken.join('\n'),
      }
}

// 4. Import público: o entrypoint carrega e exporta algo — a prova que o consumidor externo tem.
export async function importPublico(source: string): Promise<CriterionResult> {
  const entry = [`${source}/index.ts`, `${source}/index.tsx`].find(exists)
  if (!entry) return { id: 'import-publico', status: 'ausente', detail: `${source}/index.ts(x) não existe` }
  try {
    const mod = (await import(pathToFileURL(resolvePath(REPO_ROOT, entry)).href)) as Record<string, unknown>
    const names = Object.keys(mod)
    return names.length > 0
      ? { id: 'import-publico', status: 'ok', detail: `${entry} exporta ${names.length} símbolo(s)` }
      : { id: 'import-publico', status: 'falhou', detail: `${entry} carrega e não exporta nada` }
  } catch (err) {
    return { id: 'import-publico', status: 'falhou', detail: entry, evidence: firstLine(err) }
  }
}

/**
 * 5. Montagem: existe teste que MONTA e DESMONTA uma instância — função pura não prova ciclo de vida.
 * Verbo de desmontagem é aberto (`dispose`/`stop`/`close`/`deactivate`/`unregister*`) porque cada
 * módulo nomeia o próprio desligamento diferente.
 */
export function montagem(source: string, owns: (rel: string) => boolean): CriterionResult {
  const tests = walk(source, (rel) => isTest(rel) && owns(rel))
  const mounts = tests.filter((rel) => {
    const src = read(rel)
    const monta = /\.start\(/.test(src) || /(?<![A-Za-z0-9_$])new [A-Z]\w*\(/.test(src) || /create[A-Z]\w*\(/.test(src)
    const desmonta = /\.(?:dispose|stop|close|deactivate|unregister\w*)\??\(/.test(src)
    return monta && desmonta
  })
  return mounts.length > 0
    ? { id: 'montagem', status: 'ok', detail: `${mounts.length} suíte(s) montam e desmontam uma instância` }
    : { id: 'montagem', status: 'ausente', detail: 'nenhum teste monta e desmonta uma instância' }
}

// Arquivos que o mapa manual `FEATURE_COMPONENT_HANDLED_FIELDS` cobre — settings-render-coverage.test.ts
// só prova list→JSX (todo path do mapa renderiza); nada prova schema→list, então um campo novo no
// schema que ninguém lembrou de somar ao mapa fica invisível a esse teste, sem erro e sem log.
const FEATURE_FIELD_MAPS = [
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields.ts',
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields-profile.ts',
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields-voice.ts',
]

/** Campo com `.describe(m({label: ...}))`/`.describe(JSON.stringify({label: ...}))` — sinal de campo
 *  de usuário, com ou sem `defineFeatureSchema` por cima. */
const HAS_LABELED_FIELD = /\.describe\(\s*(?:m\(|JSON\.stringify\(\s*\{[^}]*label\s*:)/

/** Alguns campos (credenciais, sobretudo) sobrescrevem o `configPath` real dentro do `.describe()` —
 *  par assimétrico endpoint↔credencial, ex. `elevenLabsApiKey` cujo path real é `apiKeys.elevenLabs`. */
function overrideConfigPath(zodType: unknown): string | undefined {
  const description = (zodType as { description?: unknown })?.description
  if (typeof description !== 'string') return undefined
  try {
    const meta = JSON.parse(description) as { configPath?: unknown }
    return typeof meta.configPath === 'string' ? meta.configPath : undefined
  } catch {
    return undefined
  }
}

type SchemaFields = { exactPaths: Set<string>; looseFields: Set<string> }

/** `defineFeatureSchema` volta { schema, settings }; alguns módulos reexportam `settings` direto
 *  (desestruturado), outros o par inteiro — os dois formatos aparecem no repo. */
function groupsOf(value: unknown): unknown {
  const asDeclaration = value as { groups?: unknown; settings?: { groups?: unknown } }
  return Array.isArray(asDeclaration?.groups) ? asDeclaration.groups : asDeclaration?.settings?.groups
}

function collectExactPaths(mod: Record<string, unknown>, into: Set<string>): boolean {
  let found = false
  for (const value of Object.values(mod)) {
    const groups = groupsOf(value)
    if (!Array.isArray(groups)) continue
    found = true
    for (const group of groups as { fields?: { configPath?: string }[] }[]) {
      for (const field of group.fields ?? []) {
        if (field.configPath) into.add(field.configPath)
      }
    }
  }
  return found
}

/** Sem `defineFeatureSchema` (schema cru, ex. memory.schema.ts) não dá para saber o prefixo real sem
 *  reler quem consome o schema — o nome do campo cru vira checagem por sufixo (`looseFields`). */
function collectRawFields(mod: Record<string, unknown>, fields: SchemaFields): void {
  for (const value of Object.values(mod)) {
    const shape = (value as { shape?: Record<string, unknown> })?.shape
    if (!shape || typeof shape !== 'object') continue
    for (const [key, zodType] of Object.entries(shape)) {
      const override = overrideConfigPath(zodType)
      if (override) fields.exactPaths.add(override)
      else fields.looseFields.add(key)
    }
  }
}

async function extractSchemaFields(schemaFiles: readonly string[]): Promise<SchemaFields | string[]> {
  const fields: SchemaFields = { exactPaths: new Set(), looseFields: new Set() }
  const broken: string[] = []

  for (const file of schemaFiles) {
    try {
      const mod = (await import(pathToFileURL(resolvePath(REPO_ROOT, file)).href)) as Record<string, unknown>
      const foundGroups = collectExactPaths(mod, fields.exactPaths)
      if (!foundGroups && HAS_LABELED_FIELD.test(read(file))) collectRawFields(mod, fields)
    } catch (err) {
      broken.push(`${file} — ${firstLine(err)}`)
    }
  }
  return broken.length > 0 ? broken : fields
}

/**
 * 6. Config → UI: todo campo de configuração de usuário está no mapa que alimenta a UI. Prefere o
 * `configPath` exato de `defineFeatureSchema` (checado por igualdade); sem ele cai para "algum
 * `<prefixo>.<campo>` existe no mapa", checado por sufixo, para não fabricar prefixo errado.
 */
export async function configUi(schemaFiles: readonly string[]): Promise<CriterionResult> {
  const result = await extractSchemaFields(schemaFiles)
  if (Array.isArray(result)) {
    return {
      id: 'config-ui',
      status: 'falhou',
      detail: 'schema não carrega fora do Electron',
      evidence: result.join('\n'),
    }
  }

  const { exactPaths, looseFields } = result
  if (exactPaths.size === 0 && looseFields.size === 0) {
    return { id: 'config-ui', status: 'ok', detail: 'nenhum campo de configuração de usuário declarado' }
  }

  const corpus = FEATURE_FIELD_MAPS.map(read).join('\n')
  const missingExact = [...exactPaths].filter((path) => !corpus.includes(`'${path}'`) && !corpus.includes(`"${path}"`))
  const missingLoose = [...looseFields].filter((key) => !new RegExp(`\\.${key}['"\`]`).test(corpus))
  const total = exactPaths.size + looseFields.size
  const missing = [...missingExact, ...missingLoose]
  return missing.length === 0
    ? { id: 'config-ui', status: 'ok', detail: `${total} campo(s) mapeado(s) em FEATURE_COMPONENT_HANDLED_FIELDS` }
    : {
        id: 'config-ui',
        status: 'falhou',
        detail: `${missing.length} de ${total} campo(s) sem entrada em FEATURE_COMPONENT_HANDLED_FIELDS`,
        evidence: missing.join(', '),
      }
}

/** Suíte Rust do módulo (`cargo test --lib <filtro>`): exige ao menos um teste e nenhum vermelho. */
function rustSuite(filter: string): { passed: number; ok: boolean; output: string } {
  const cargo = spawnSync('cargo', ['test', '--lib', '--quiet', filter], {
    cwd: resolvePath(REPO_ROOT, 'apps/desktop/src-tauri'),
    encoding: 'utf8',
    shell: process.platform === 'win32',
  })
  const output = `${cargo.stdout ?? ''}${cargo.stderr ?? ''}`
  const passed = Number(/test result: ok\. (\d+) passed/.exec(output)?.[1] ?? 0)
  return { passed, ok: cargo.status === 0 && passed > 0, output }
}

/**
 * 7. Contrato: a suíte do módulo passa. Sem suíte não há aprovação — ausência não é sucesso.
 * `rust` é o filtro do módulo Rust que implementa o comportamento desde que o backend Node saiu;
 * a suíte TS que sobra (renderer) e a Rust contam juntas.
 */
export function contrato(source: string, children: readonly string[], rust?: string): CriterionResult {
  const tests = walk(source, (rel) => isTest(rel) && !children.some((child) => rel.startsWith(`${child}/`)))
  if (tests.length === 0 && !rust) return { id: 'contrato', status: 'ausente', detail: 'o módulo não tem suíte' }
  const failures: string[] = []
  const parts: string[] = []
  if (tests.length > 0) {
    const excluded = children.flatMap((child) => ['--exclude', `${child}/**`])
    const vitest = run(['-s', 'vitest', 'run', source, ...excluded, '--reporter=dot'])
    parts.push(`${tests.length} arquivo(s) de teste`)
    if (vitest.status !== 0) failures.push(vitest.stdout.trim().slice(-1500))
  }
  if (rust) {
    const suite = rustSuite(rust)
    parts.push(`${suite.passed} teste(s) Rust em ${rust}`)
    if (!suite.ok) failures.push(suite.output.trim().slice(-1500) || `nenhum teste Rust casou com ${rust}`)
  }
  const detail = parts.join(' + ')
  return failures.length === 0
    ? { id: 'contrato', status: 'ok', detail }
    : { id: 'contrato', status: 'falhou', detail, evidence: failures.join('\n') }
}
