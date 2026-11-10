/**
 * Notas da release a partir dos commits convencionais desde a última tag — as mesmas que decidem a
 * versão em `next-version.mts`. Vão para o corpo da release do GitHub E para o `notes` do
 * `latest.json`, que é o que o updater do app mostra antes de baixar: sem isso o usuário via só um
 * número de versão.
 *
 * Uso: `tsx scripts/versioning/release-notes.mts` → imprime markdown (ou nada).
 */
import { pathToFileURL } from 'node:url'
import { commitsSinceLastTag } from './next-version.mts'

const HEADER_RE = /^(\w+)(?:\(([^)]*)\))?(!)?:\s*(.+)$/
const BREAKING_FOOTER_RE = /^BREAKING[ -]CHANGE:/m
const SQUASH_PR_RE = /\s*\(#\d+\)$/

/** Ordem de leitura: o que pode quebrar algo vem primeiro. Tipo fora daqui (chore, ci…) não entra. */
const SECTIONS = [
  { key: 'breaking', title: '## ⚠️ Mudanças incompatíveis' },
  { key: 'feat', title: '## ✨ Novidades' },
  { key: 'fix', title: '## 🐛 Correções' },
  { key: 'perf', title: '## ⚡ Desempenho' },
] as const

function classify(commit: string): { section: string; line: string } | null {
  const match = HEADER_RE.exec(commit.split('\n')[0] ?? '')
  if (!match) return null
  const [, type, scope, bang, subject] = match
  const section = bang || BREAKING_FOOTER_RE.test(commit) ? 'breaking' : type
  const text = subject.replace(SQUASH_PR_RE, '')
  return { section, line: scope ? `- **${scope}:** ${text}` : `- ${text}` }
}

export function releaseNotes(commits: string[]): string {
  const entries = commits.map(classify).filter((entry) => entry !== null)
  return SECTIONS.map(({ key, title }) => {
    const lines = entries.filter((entry) => entry.section === key).map((entry) => entry.line)
    return lines.length > 0 ? [title, ...lines].join('\n') : ''
  })
    .filter(Boolean)
    .join('\n\n')
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  process.stdout.write(releaseNotes(commitsSinceLastTag().commits))
}
