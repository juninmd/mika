/**
 * check:versions — `package.json`, `apps/desktop/package.json` e `src-tauri/Cargo.toml` têm a mesma versão?
 *
 * O updater compara a versão do `tauri.conf.json` (que aponta para o package.json), enquanto o
 * binário carrega a do Cargo.toml nos metadados. Divergiram (0.6.0 vs 0.1.0): o instalador e o
 * "Propriedades" do exe diziam versões diferentes, e o suporte não saberia qual build o usuário roda.
 */

import { readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '../..')
const read = (file: string) => readFileSync(join(ROOT, file), 'utf8')

const versions: Record<string, string | undefined> = {
  'package.json': (JSON.parse(read('package.json')) as { version?: string }).version,
  'apps/desktop/package.json': (JSON.parse(read('apps/desktop/package.json')) as { version?: string }).version,
  'apps/desktop/src-tauri/Cargo.toml': /^version\s*=\s*"([^"]+)"/m.exec(read('apps/desktop/src-tauri/Cargo.toml'))?.[1],
}

if (new Set(Object.values(versions)).size > 1 || Object.values(versions).includes(undefined)) {
  console.error('check:versions — versões divergentes:')
  for (const [file, version] of Object.entries(versions)) console.error(`  ${file}: ${version ?? '(ausente)'}`)
  process.exit(1)
}
console.log(`check:versions OK (${Object.values(versions)[0]})`)
