import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { buildCatalog, collectWrites, generateForModel } from './gen-motions-output'
import { analyzeModel, listModelDirs } from './model-analysis'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')

const FILE_PREFIX = 'mika_'

const CATALOG_PATH = join(
  ROOT,
  'apps',
  'desktop',
  'src',
  'renderer',
  'src',
  'components',
  'live2d',
  'live2d-action-catalog.generated.ts',
)

const args = process.argv.slice(2)
const checkOnly = args.includes('--check')
const wanted = args.filter((arg) => !arg.startsWith('--'))

const models = listModelDirs(ROOT).filter((model) => wanted.length === 0 || wanted.includes(model.name))
if (models.length === 0) {
  console.error('❌ Nenhum modelo Live2D encontrado em apps/desktop/src/renderer/public/live2d')
  process.exit(1)
}

const results = models.map((model) => generateForModel(analyzeModel(model), FILE_PREFIX))

const hardErrors = results.flatMap((result) => result.errors.map((error) => `${result.analysis.name}: ${error}`))
if (hardErrors.length > 0) {
  console.error('\n❌ Geração abortada:\n')
  for (const error of hardErrors) console.error(`  - ${error}`)
  process.exit(1)
}

const writes = results.flatMap(collectWrites)

if (wanted.length === 0) writes.push({ path: CATALOG_PATH, content: buildCatalog(results) })

for (const result of results) {
  const names = result.actions.map((action) => action.name).join(', ')
  console.log(`\n${result.analysis.name} (${result.analysis.stem}) — ${result.actions.length} ações: ${names}`)
  for (const note of result.notes) console.log(`  · ${note}`)
}

if (checkOnly) {
  const drifted: string[] = []
  for (const write of writes) {
    let current: string | null = null
    try {
      current = readFileSync(write.path, 'utf8')
    } catch {
      current = null
    }
    if (current !== write.content) drifted.push(relative(ROOT, write.path))
  }

  console.log('\n---')
  if (drifted.length > 0) {
    console.error(`❌ ${drifted.length} arquivo(s) divergem das definições. Rode: pnpm live2d:gen\n`)
    for (const path of drifted) console.error(`  - ${path}`)
    process.exit(1)
  }
  console.log(`✅ ${writes.length} arquivos conferem com as definições`)
  process.exit(0)
}

for (const write of writes) {
  mkdirSync(join(write.path, '..'), { recursive: true })
  writeFileSync(write.path, write.content)
}

console.log(`\n---\n✅ ${writes.length} arquivos gravados. Confira com: pnpm live2d:gen --check`)
