import { existsSync, readdirSync } from 'node:fs'
import { join, resolve } from 'node:path'

/**
 * Raiz única de prompts de módulo, fora de `src/main`/`src/features` — essas árvores são só a UI
 * (`renderer/`) hoje e vão sumir na migração Tauri; o pipeline de prompt não pode depender delas.
 * Cada subpasta direta é um namespace (`prompts/brain`, `prompts/vision`, …).
 */
const PROMPTS_ROOT = 'prompts'

/** Uma capacidade extraída para `packages/` leva os próprios prompts junto; sem esta raiz o `.md` some do bundle sem erro nem log. */
const PACKAGE_ROOT = '../../packages'

/** Pasta de prompts é pasta com `.md`: uma pasta `prompts/` pode guardar só código, não assets. */
function holdsMarkdown(dir: string): boolean {
  return existsSync(dir) && readdirSync(dir).some((name) => name.endsWith('.md'))
}

/** Subpastas diretas de `base` — vazio quando `base` não existe. */
function subdirsOf(base: string): string[] {
  if (!existsSync(base)) return []
  return readdirSync(base, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
}

/**
 * Toda pasta `prompts/` de módulo, relativa à raiz de `apps/desktop`. Descoberta automática: uma lista
 * escrita à mão em `electron.vite.config.ts` deixava `.md` de fora do bundle sem erro visível.
 */
export interface PromptDirEntry {
  /** Namespace do registry (`vision` unifica os 5 handlers que apontam para a mesma pasta). */
  namespace: string
  /** Caminho relativo a `apps/desktop`. */
  dir: string
}

export function discoverPromptDirs(desktopRoot: string): string[] {
  return discoverPromptDirEntries(desktopRoot).map((e) => e.dir)
}

/**
 * Igual a `discoverPromptDirs`, mas com o namespace de cada pasta — o nome da pasta do módulo
 * (`brain`, `character`, …), exceto `vision`, cujos 5 handlers compartilham uma pasta e um só
 * `registerPromptModule`.
 */
export function discoverPromptDirEntries(desktopRoot: string): PromptDirEntry[] {
  const found: PromptDirEntry[] = []

  const promptsRoot = resolve(desktopRoot, PROMPTS_ROOT)
  for (const name of subdirsOf(promptsRoot)) {
    if (holdsMarkdown(join(promptsRoot, name))) found.push({ namespace: name, dir: `${PROMPTS_ROOT}/${name}` })
  }

  const packages = resolve(desktopRoot, PACKAGE_ROOT)
  for (const name of subdirsOf(packages)) {
    if (holdsMarkdown(join(packages, name, 'src', 'prompts'))) {
      found.push({ namespace: name, dir: `${PACKAGE_ROOT}/${name}/src/prompts` })
    }
  }

  return found.sort((a, b) => a.dir.localeCompare(b.dir))
}
