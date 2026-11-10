/**
 * check:topology — roster de UI ↔ pastas.
 *
 * Protege `specs/01-architecture.md`: uma feature de UI existe porque alguém escreveu à mão numa
 * lista literal. Lista literal não deriva de nada, logo não pode divergir em silêncio — mas só se
 * algo conferir.
 *
 * Migração Tauri (Stage C, 2026-09-22): o roster de boot do processo main Node
 * (`FEATURE_LOADERS`/`src/main/infrastructure/features.ts`) morreu com `src/main` inteiro. O
 * equivalente Rust é `pub mod <feature>;` em `src-tauri/src/modules/features/mod.rs` — "pasta sem
 * registro" já é erro de compilação lá, não precisa de gate: o compilador é o gate.
 *
 * Rotas de UI convivem de propósito e todas são legítimas:
 *  - `features/<nome>/renderer/index.tsx` + `feature-ui-registry.ts` (carregamento tardio, chunk próprio);
 *  - `renderer/settings/<nome>/index.tsx` (núcleo bloqueante: brain/chat/vision/voice) + registry;
 *  - `renderer/settings.tsx` importado por `settings/feature-components.tsx` (painel estático).
 * O gate só acusa a feature que tem `renderer/index` e ficou fora do registry — essa some sem erro.
 */
import { readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { exists, REPO_ROOT, read, report, type Violation } from './lib.mts'

const FEATURES_DIR = 'apps/desktop/src/features'
const UI_ROSTER = 'apps/desktop/src/renderer/src/lib/feature-ui-registry.ts'

const folders = readdirSync(resolve(REPO_ROOT, FEATURES_DIR), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort()

/** Só as chaves do objeto literal nomeado — não qualquer `chave:` do arquivo inteiro. */
function keysOfLiteral(source: string, literalName: string): Set<string> {
  const start = source.indexOf(literalName)
  if (start === -1) throw new Error(`literal ${literalName} não encontrado`)
  const open = source.indexOf('{', start)
  const close = source.indexOf('\n}', open)
  const body = source.slice(open, close === -1 ? undefined : close)

  const keys = new Set<string>()
  for (const match of body.matchAll(/^\s*'([^']+)':/gm)) keys.add(match[1])
  for (const match of body.matchAll(/^\s*([a-z][\w-]*):/gm)) keys.add(match[1])
  return keys
}

const inUi = keysOfLiteral(read(UI_ROSTER), 'FEATURE_UI_LOADERS')
const violations: Violation[] = []

for (const folder of folders) {
  const hasEntry =
    exists(`${FEATURES_DIR}/${folder}/renderer/index.tsx`) || exists(`${FEATURES_DIR}/${folder}/renderer/index.ts`)
  if (hasEntry && !inUi.has(`mika.${folder}`)) {
    violations.push({
      file: UI_ROSTER,
      line: 1,
      message: `features/${folder}/renderer/index existe mas "mika.${folder}" não está no roster de UI — o chunk nunca é carregado`,
    })
  }
}

for (const id of inUi) {
  const name = id.replace(/^mika\./, '')
  const featurePath = `${FEATURES_DIR}/${name}/renderer/index`
  const coreSettingsPath = `apps/desktop/src/renderer/settings/${name}/index`
  const found = [featurePath, coreSettingsPath].some((base) => exists(`${base}.tsx`) || exists(`${base}.ts`))
  if (!found) {
    violations.push({
      file: UI_ROSTER,
      line: 1,
      message: `roster de UI declara "${id}" mas não existe renderer/index correspondente`,
    })
  }
}

report('check:topology', violations, folders.length + inUi.size)
