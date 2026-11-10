/**
 * Posse de arquivo: cada fonte pertence ao módulo de `source` MAIS ESPECÍFICO que o contém. Sem isso
 * um módulo-pai (`brain`, `main`, `renderer`) somava a árvore inteira dos filhos: o digest caducava a
 * cada mudança num filho e a fronteira declarava deps que não eram dele.
 */
import { resolve as resolvePath } from 'node:path'
import { pathToFileURL } from 'node:url'
import { REPO_ROOT, sourceRoots, walk } from '../checks/lib.mts'

export type Manifest = import('@mika/sdk').ModuleManifest

export interface ManifestEntry {
  file: string
  manifest: Manifest
}

export const manifestFiles = (): string[] =>
  sourceRoots().flatMap((root) => walk(root, (rel) => rel.endsWith('.module.ts')))

export async function loadManifest(file: string): Promise<Manifest | undefined> {
  const loaded = (await import(pathToFileURL(resolvePath(REPO_ROOT, file)).href)) as Record<string, unknown>
  return Object.values(loaded).find(
    (value): value is Manifest => typeof value === 'object' && value !== null && 'id' in value && 'source' in value,
  )
}

export async function loadManifests(): Promise<ManifestEntry[]> {
  const entries: ManifestEntry[] = []
  for (const file of manifestFiles()) {
    const manifest = await loadManifest(file)
    if (manifest) entries.push({ file, manifest })
  }
  return entries
}

const contains = (source: string, rel: string): boolean => rel === source || rel.startsWith(`${source}/`)

/** Sources de módulos aninhados dentro de `source` — o que o pai NÃO possui. */
export const childSources = (source: string, all: readonly string[]): string[] =>
  all.filter((other) => other !== source && contains(source, other))

/** Filtro de walk: o arquivo é do módulo, e não de um filho dele. */
export const ownedBy =
  (source: string, all: readonly string[]) =>
  (rel: string): boolean =>
    !childSources(source, all).some((child) => contains(child, rel))

/** O módulo dono de `rel`, ou undefined quando nenhum `source` o contém. */
export function ownerOf(rel: string, all: readonly string[]): string | undefined {
  return all.filter((source) => contains(source, rel)).sort((a, b) => b.length - a.length)[0]
}
