/**
 * check:module-manifest — pega manifesto (deps/emits/consumes/config) desalinhado do código real,
 * nos dois sentidos: uso não declarado (fronteira vazou) e declaração não usada (apodreceu).
 * `--print` gera as listas a partir do código; `frozen` trava o hash do contrato do módulo.
 */
import { createHash } from 'node:crypto'
import { manifestFiles as listManifests, loadManifest, loadManifests, ownedBy } from '../homologation/ownership.mts'
import { exists, isSource, isTest, read, report, type Violation, walk } from './lib.mts'

const CATALOG = 'packages/sdk/src/svc/events.ts'
const PRINT = process.argv.includes('--print')
// `--only <id>`: homologação prova UM módulo; sem isso um manifesto sujo alhures reprovava todos.
const ONLY = process.argv.includes('--only') ? process.argv[process.argv.indexOf('--only') + 1] : null

const valueByKey = new Map<string, string>()
for (const match of read(CATALOG).matchAll(/^\s+([A-Z][A-Z0-9_]*):\s*'([^']+)'/gm)) {
  valueByKey.set(match[1], match[2])
}
const knownEventValues = new Set(valueByKey.values())

const manifestFiles = listManifests()
const allSources = (await loadManifests()).map((entry) => entry.manifest.source)

const violations: Violation[] = []

function compare(file: string, kind: string, declared: readonly string[], used: Set<string>): void {
  const declaredSet = new Set(declared)
  for (const item of used) {
    if (!declaredSet.has(item)) {
      violations.push({ file, line: 1, message: `${kind}: o código usa "${item}" e o manifesto não declara` })
    }
  }
  for (const item of declaredSet) {
    if (!used.has(item)) {
      violations.push({ file, line: 1, message: `${kind}: o manifesto declara "${item}" e o código não usa` })
    }
  }
}

function collectEventAliases(source: string): Map<string, string> {
  const aliases = new Map<string, string>()
  for (const match of source.matchAll(/(?:const|let)\s+([A-Z][A-Z0-9_]*)\s*=\s*Events\.([A-Z0-9_]+)/g)) {
    const value = valueByKey.get(match[2])
    if (value !== undefined) aliases.set(match[1], value)
  }
  return aliases
}

/** Resolve um token de chamada (constante Events.X, literal ou alias local) pro valor do evento, ou undefined. */
function resolveEventToken(token: string, aliases: Map<string, string>): string | undefined {
  const constant = /^Events\.([A-Z0-9_]+)$/.exec(token)
  if (constant) return valueByKey.get(constant[1])

  const literal = /^'([^']+)'$/.exec(token)
  if (literal) return knownEventValues.has(literal[1]) ? literal[1] : undefined

  return aliases.get(token)
}

// Três resoluções (constante, alias local, literal conhecido); sem as três, falso positivo.
function eventsIn(source: string, verbs: string): Set<string> {
  const found = new Set<string>()
  const aliases = collectEventAliases(source)

  // Com ponto e sem: emissão pode chegar como porta injetada.
  const call = new RegExp(
    String.raw`(?<![A-Za-z0-9_$])(?:${verbs})\(\s*(Events\.[A-Z0-9_]+|'[^']+'|[A-Z][A-Z0-9_]*)`,
    'g',
  )
  for (const match of source.matchAll(call)) {
    const value = resolveEventToken(match[1], aliases)
    if (value !== undefined) found.add(value)
  }
  return found
}

const asList = (items: Iterable<string>): string =>
  [...items]
    .sort()
    .map((item) => `'${item}'`)
    .join(', ')

for (const file of manifestFiles) {
  const manifestSource = read(file)

  // Manifesto é lido pelo gate em Node puro; import de runtime aqui derruba o gate, não o produto.
  for (const match of manifestSource.matchAll(/^import\s[^\n]*from\s+'([^']+)'/gm)) {
    if (match[1] !== '@mika/sdk/contracts') {
      violations.push({
        file,
        line: 1,
        message: `manifesto importa "${match[1]}"; só '@mika/sdk/contracts' — ele é carregado pelo gate, fora do app`,
      })
    }
  }

  const manifest = await loadManifest(file)

  if (!manifest) {
    violations.push({ file, line: 1, message: 'não exporta nenhum manifesto de defineModule' })
    continue
  }

  if (ONLY !== null && manifest.id !== ONLY) continue

  if (!exists(manifest.source)) {
    violations.push({ file, line: 1, message: `source "${manifest.source}" não existe no disco` })
    continue
  }

  const owns = ownedBy(manifest.source, allSources)
  const sources = walk(
    manifest.source,
    (rel) => isSource(rel) && !isTest(rel) && !rel.endsWith('.module.ts') && owns(rel),
  )
  const body = sources.map(read).join('\n')

  // Fronteira à esquerda, como em `eventsIn`: sem ela qualquer identificador TERMINADO em
  // TOKENS casa, e `UNSUPPORTED_TOKENS.has(x)` virava a dependência "has".
  const usedDeps = new Set([...body.matchAll(/(?<![A-Za-z0-9_$])TOKENS\.([A-Za-z0-9_]+)/g)].map((m) => m[1]))
  const usedEmits = eventsIn(body, 'emit')
  const usedConsumes = eventsIn(body, 'on|once|off')

  if (PRINT) {
    process.stdout.write(
      `${manifest.id}\n  deps: [${asList(usedDeps)}]\n  emits: [${asList(usedEmits)}]\n  consumes: [${asList(usedConsumes)}]\n`,
    )
    continue
  }

  compare(file, 'deps', manifest.deps ?? [], usedDeps)
  compare(file, 'emits', manifest.emits ?? [], usedEmits)
  compare(file, 'consumes', manifest.consumes ?? [], usedConsumes)

  const configPrefixes = manifest.config ?? []
  const readKeys = new Set(
    [...body.matchAll(/(?:get|set|subscribe)<[^>]*>?\(\s*'([a-z][A-Za-z0-9]*\.[A-Za-z0-9.]+)'/g)].map((m) => m[1]),
  )
  for (const key of readKeys) {
    if (!configPrefixes.some((prefix) => key.startsWith(prefix))) {
      violations.push({ file, line: 1, message: `config: lê "${key}" fora dos prefixos declarados` })
    }
  }

  for (const contractFile of manifest.contract) {
    if (!exists(contractFile)) {
      violations.push({ file, line: 1, message: `contract cita "${contractFile}", que não existe` })
    }
  }

  if (manifest.frozen) {
    const hash = createHash('sha256')
    for (const contractFile of manifest.contract.filter(exists)) hash.update(read(contractFile))
    const actual = hash.digest('hex').slice(0, 16)
    if (actual !== manifest.frozen.hash) {
      violations.push({
        file,
        line: 1,
        message: `módulo congelado em ${manifest.frozen.at}: contrato mudou (${manifest.frozen.hash} → ${actual}). Reabra a homologação e regrave o hash`,
      })
    }
  }
}

if (!PRINT) report('check:module-manifest', violations, manifestFiles.length)
