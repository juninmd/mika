/**
 * check:license-policy — o instalador vendido só carrega licença que a política aceita, e o aviso de
 * terceiros que ele carrega é o de HOJE.
 *
 * Duas perguntas, cada uma com a classe de defeito que já existiu em projetos como este:
 * 1. Alguma dependência de produção (Rust via `cargo metadata`, npm via `pnpm licenses --prod`) tem
 *    GPL/AGPL/LGPL-sem-exceção, licença não-comercial ou nenhuma licença? Uma dependência nova puxa
 *    transitivas sem que ninguém leia a licença de cada uma — `scripts/lib/license-policy.mts` é a
 *    regra, `deny.toml` a espelha para o `cargo deny`.
 * 2. `THIRD-PARTY-NOTICES.md` existe e o hash do cabeçalho (Cargo.lock + pnpm-lock + inventário
 *    manual + textos) bate com o de agora? Sem isso o aviso envelhece em silêncio a cada
 *    `cargo update`/`pnpm add`, e o instalador distribui atribuição de uma árvore que não é a dele.
 *    Correção: `pnpm notices:gen`.
 */
import { collectNpm, collectRust, inputHash, NOTICES_FILE } from '../lib/license-inventory.mts'
import { evaluateLicense } from '../lib/license-policy.mts'
import { exists, read, report, type Violation } from './lib.mts'

const HASH_HEADER = /mika-notices-input-sha256:\s*([0-9a-f]{64})/

function noticesViolation(): Violation | null {
  if (!exists(NOTICES_FILE)) {
    return { file: NOTICES_FILE, line: 1, message: 'ausente — rode `pnpm notices:gen`' }
  }
  const recorded = HASH_HEADER.exec(read(NOTICES_FILE))?.[1]
  if (recorded === undefined) {
    return { file: NOTICES_FILE, line: 1, message: 'sem hash de entrada no cabeçalho — rode `pnpm notices:gen`' }
  }
  if (recorded !== inputHash()) {
    return {
      file: NOTICES_FILE,
      line: 1,
      message: 'desatualizado em relação a Cargo.lock/pnpm-lock/inventário — rode `pnpm notices:gen`',
    }
  }
  return null
}

const components = [...collectRust(false), ...collectNpm(false)]
const violations: Violation[] = []

for (const component of components) {
  const verdict = evaluateLicense(component.license, component.name)
  if (verdict.ok) continue
  violations.push({
    file: component.ecosystem === 'rust' ? 'Cargo.lock' : 'pnpm-lock.yaml',
    line: 1,
    message: `${component.name}@${component.version} (${component.license || 'sem licença'}): ${verdict.problems.join('; ')}`,
  })
}

const stale = noticesViolation()
if (stale !== null) violations.push(stale)

// Piso de alcance: um coletor que passe a devolver vazio (cargo/pnpm mudou de formato) não sai verde.
if (components.length < 500) {
  violations.push({
    file: 'scripts/checks/license-policy.mts',
    line: 1,
    message: `coletou só ${components.length} dependência(s) — o coletor quebrou, confira cargo metadata / pnpm licenses`,
  })
}

report('check:license-policy', violations, components.length)
