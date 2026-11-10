/**
 * check:config-read-paths — "chamador → schema", pega `ConfigStore::get("raiz.folha")`/`set(...)`
 * cujo caminho não corresponde a NENHUM campo declarado em `config/schemas/*.rs` — o typo que
 * `check:config-keys` não vê, porque aquele confere só os membros do `ConfigKey` enum (que hoje
 * cobre credenciais e alguns atalhos), não toda string literal passada a `ConfigStore::get`/`set`.
 *
 * `specs/11-quality-gates.md` ("Ainda não automatizados") listava isto como `[não verificado]` —
 * o teste TS equivalente (`config-read-paths` contra `ConfigStore::get` do Node) nunca teve par no
 * Rust. Medido: hoje são 69 caminhos literais únicos em produção, todos válidos — o gate garante que
 * a próxima renomeação de campo (ou o próximo typo) não passa silenciosa, como `apiKeys.openai`
 * (minúsculo) já quase passou uma vez (ver `config/store.rs::API_KEYS_PREFIX`).
 *
 * Só string literal com PONTO conta como caminho de config — `Value::get("tabId")`,
 * `obj.get("pluginId")` (chave de JSON genérico, sem ponto) não é `ConfigStore::get` e não deve
 * virar falso positivo.
 */
import { configSchemaLeafPaths, lineOf, read, report, type Violation, walk } from './lib.mts'

const ROOT = 'apps/desktop/src-tauri/src'
const CALL = /\.(?:get|set)\(\s*"([a-zA-Z][\w-]*(?:\.[\w-]+)+)"/g

const schema = configSchemaLeafPaths()

const violations: Violation[] = []
let callSites = 0

for (const file of walk(
  ROOT,
  (rel) => rel.endsWith('.rs') && !rel.startsWith('apps/desktop/src-tauri/src/config/schemas/'),
)) {
  const source = read(file)
  const cut = source.indexOf('#[cfg(test)]')
  const production = cut === -1 ? source : source.slice(0, cut)

  for (const match of production.matchAll(CALL)) {
    callSites++
    const path = match[1] as string
    if (schema.has(path)) continue
    violations.push({
      file,
      line: lineOf(production, match.index ?? 0),
      message: `"${path}" não corresponde a nenhum campo de config/schemas/*.rs — typo, campo removido, ou schema desatualizado`,
    })
  }
}

// Piso de alcance: um regex que pare de casar (ou um schema vazio) não deve sair verde em silêncio.
if (callSites < 50) {
  violations.push({
    file: 'scripts/checks/config-read-paths.mts',
    line: 1,
    message: `só ${callSites} chamada(s) reconhecida(s) — piso de alcance quebrado, confira CALL antes de confiar no resultado`,
  })
}
if (schema.size < 200) {
  violations.push({
    file: 'scripts/checks/config-read-paths.mts',
    line: 1,
    message: `configSchemaLeafPaths() só achou ${schema.size} campo(s) — confira scripts/checks/lib.mts antes de confiar no resultado`,
  })
}

report('check:config-read-paths', violations, callSites)
