import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const ARTIFACTS_DIR = join(__dirname, '..', 'screenshots')

/** Caminho de um artefato (PNG/JSON) desta run, para `page.screenshot({ path })`. */
export const artifactPath = (file: string): string => join(ARTIFACTS_DIR, file)

/**
 * Artefato final verificável de um spec sem PNG (`AGENTS.md` §5.3): grava em
 * `e2e/screenshots/<name>.json` o ESTADO que o spec acabou de afirmar — só o que ele afirmou, nada
 * a mais. `global-setup.ts` recria a pasta a cada run, então o arquivo é sempre o desta execução.
 * Chame DEPOIS das asserções: spec que falhou não deixa artefato de "passou".
 */
export function writeArtifact(name: string, state: unknown): void {
  mkdirSync(ARTIFACTS_DIR, { recursive: true })
  writeFileSync(join(ARTIFACTS_DIR, `${name}.json`), `${JSON.stringify(state, null, 2)}\n`)
}
