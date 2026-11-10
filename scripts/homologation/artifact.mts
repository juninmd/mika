/**
 * Identidade do artefato homologado. `frozen.hash` só responde se a API pública mudou, não se a
 * implementação testada ainda é a que está na árvore — troca de algoritmo/prompt/versão invalida
 * a homologação mesmo com API idêntica.
 */

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { exists, isSource, isTest, REPO_ROOT, read, walk } from '../checks/lib.mts'

export interface ArtifactIdentity {
  /** SHA do HEAD, sufixado com `+dirty` quando a árvore tem alteração local. */
  commit: string
  /** Digest da implementação: fontes e prompts do módulo, sem os testes. */
  tree: string
  /** Digest dos arquivos declarados em `contract` — o mesmo insumo do `frozen`. */
  contract: string
  /** Dependências declaradas pelo workspace dono do módulo. */
  deps: Record<string, string>
  /** Digest do lockfile: registrado como evidência, não como gatilho de invalidação. */
  lockfile: string
}

const digest = (parts: string[]): string => createHash('sha256').update(parts.join('\0')).digest('hex').slice(0, 16)

/** Assets contam: um `.md` de prompt trocado muda a fala sem tocar uma linha de TypeScript. */
const isImplementation = (rel: string): boolean => (isSource(rel) || rel.endsWith('.md')) && !isTest(rel)

function gitDescribe(): string {
  try {
    const head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: REPO_ROOT, encoding: 'utf8' }).trim()
    const dirty = execFileSync('git', ['status', '--porcelain'], { cwd: REPO_ROOT, encoding: 'utf8' }).trim()
    return dirty ? `${head}+dirty` : head
  } catch {
    return 'sem-git'
  }
}

/** `package.json` do workspace que contém `source`, quando houver (`packages/<x>/package.json`). */
function ownerManifest(source: string): string | null {
  const parts = source.split('/')
  for (let depth = parts.length; depth > 0; depth--) {
    const candidate = `${parts.slice(0, depth).join('/')}/package.json`
    if (exists(candidate)) return candidate
  }
  return null
}

export function identify(
  source: string,
  contract: readonly string[],
  owns: (rel: string) => boolean = () => true,
): ArtifactIdentity {
  const files = walk(source, (rel) => isImplementation(rel) && owns(rel))
  const owner = ownerManifest(source)
  const pkg = owner ? (JSON.parse(read(owner)) as { dependencies?: Record<string, string> }) : {}

  return {
    commit: gitDescribe(),
    tree: digest(files.map((rel) => `${rel}:${digest([read(rel)])}`)),
    contract: digest(contract.filter(exists).map((rel) => read(rel))),
    deps: pkg.dependencies ?? {},
    lockfile: exists('pnpm-lock.yaml') ? digest([read('pnpm-lock.yaml')]) : 'ausente',
  }
}
