/**
 * check:release-docs — o `steam/README.md` é o passo a passo de quem configura o repositório para
 * lançar; um secret/variável que o workflow lê e o README não cita faz o canal ficar desligado em
 * silêncio (o job só emite `::warning::`), e um nome citado que nenhum workflow lê manda a pessoa
 * cadastrar um secret que não faz nada.
 *
 * Pergunta sobre texto de dois arquivos, sem objeto em runtime: gate, não teste de contrato.
 */
import { lineOf, read, report, type Violation, walk } from './lib.mts'

const README = 'steam/README.md'
const readme = read(README)
const workflows = walk('.github/workflows', (rel) => /\.ya?ml$/.test(rel))
const violations: Violation[] = []

const used = new Map<string, string>()
for (const file of workflows) {
  for (const match of read(file).matchAll(/\b(?:secrets|vars)\.([A-Z][A-Z0-9_]+)\b/g)) {
    if (match[1] !== 'GITHUB_TOKEN' && !used.has(match[1])) used.set(match[1], file)
  }
}

for (const [name, file] of used) {
  if (!new RegExp(`\\b${name}\\b`).test(readme)) {
    violations.push({ file: README, line: 1, message: `não documenta \`${name}\`, lido em ${file}` })
  }
}

// Nomes na família dos que o release lê: citado como código no README, mas ausente de todo workflow.
const FAMILY = /^(?:STEAM|TAURI|APPLE|WINDOWS)_[A-Z0-9_]+$/
for (const match of readme.matchAll(/`([A-Z][A-Z0-9_]+)`/g)) {
  if (FAMILY.test(match[1]) && !used.has(match[1])) {
    violations.push({
      file: README,
      line: lineOf(readme, match.index),
      message: `cita \`${match[1]}\`, que nenhum workflow lê`,
    })
  }
}

report('check:release-docs', violations, workflows.length + 1)
