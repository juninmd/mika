/**
 * check:config-keys — pega `ConfigKey` sem nenhum consumidor (promessa não cumprida; já aconteceu
 * com 4 chaves de privacidade sem schema, UI ou leitura). Só chaves com caminho (`raiz.folha`);
 * raízes puras são namespaces endereçados dinamicamente.
 */
import { isSource, isTest, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const ENUM = 'packages/sdk/src/contracts/schemas/config-keys.enum.ts'

const source = read(ENUM)
const members = new Map<string, string>()
for (const match of source.matchAll(/^\s+(\w+)\s*=\s*'([^']+)',?$/gm)) {
  if (match[2].includes('.')) members.set(match[1], match[2])
}

// Stage C (2026-09-22): a leitura/escrita de produção migrou para Rust (`store.get`/`store.set`)
// junto com `src/main` — sem isto, toda `ConfigKey` só implementada no backend Rust acusava falso
// "sem consumidor".
const RUST_CONSUMERS = walk('apps/desktop/src-tauri/src', (rel) => rel.endsWith('.rs'))
  .map((rel) => read(rel))
  .join('\n')

const consumers =
  [...sourceRoots().flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))]
    .filter((rel) => rel !== ENUM)
    .map((rel) => read(rel))
    .join('\n') +
  '\n' +
  RUST_CONSUMERS

const violations: Violation[] = []

for (const [member, path] of members) {
  const byMember = new RegExp(`ConfigKey\\.${member}\\b`).test(consumers)
  const byLiteral = consumers.includes(`'${path}'`) || consumers.includes(`"${path}"`)
  if (byMember || byLiteral) continue

  violations.push({
    file: ENUM,
    line: 1,
    message: `ConfigKey.${member} ("${path}") não é lida nem escrita em lugar nenhum — implemente ou remova`,
  })
}

report('check:config-keys', violations, members.size)
