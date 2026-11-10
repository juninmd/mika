/**
 * Próxima versão a partir da última tag `vX.Y.Z` e dos commits convencionais desde ela.
 *
 * A tag é a fonte da verdade da versão: o workflow de release não commita o bump de volta na `main`,
 * porque um push do próprio CI dispararia outra release. Sem saída = nada a lançar.
 *
 * Uso: `tsx scripts/versioning/next-version.mts` → imprime `0.7.0` ou nada.
 */
import { execFileSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const TAG_RE = /^v(\d+)\.(\d+)\.(\d+)$/
const BREAKING_RE = /^\w+(\([^)]*\))?!:|^BREAKING[ -]CHANGE:/m
const FEAT_RE = /^feat(\([^)]*\))?:/

export function nextVersion(lastTag: string, commits: string[]): string | null {
  const match = TAG_RE.exec(lastTag)
  if (!match) throw new Error(`tag "${lastTag}" fora do formato vX.Y.Z`)
  if (commits.length === 0) return null
  const [major, minor, patch] = match.slice(1).map(Number)

  if (commits.some((c) => BREAKING_RE.test(c))) {
    return major === 0 ? `0.${minor + 1}.0` : `${major + 1}.0.0`
  }
  if (commits.some((c) => FEAT_RE.test(c))) return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
}

function git(...args: string[]): string {
  return execFileSync('git', args, { encoding: 'utf8' }).trim()
}

/** Mensagens completas dos commits desde a última tag `vX.Y.Z` — a mesma janela da versão e das notas. */
export function commitsSinceLastTag(): { lastTag: string; commits: string[] } {
  const lastTag = git('describe', '--tags', '--abbrev=0', '--match', 'v[0-9]*.[0-9]*.[0-9]*')
  const commits = git('log', `${lastTag}..HEAD`, '--format=%B%x00')
    .split('\0')
    .map((c) => c.trim())
    .filter(Boolean)
  return { lastTag, commits }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const { lastTag, commits } = commitsSinceLastTag()
  const version = nextVersion(lastTag, commits)
  if (version) process.stdout.write(version)
}
