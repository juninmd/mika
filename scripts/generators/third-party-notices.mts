/**
 * Gera `THIRD-PARTY-NOTICES.md` — o aviso de terceiros que o instalador carrega (`bundle.resources`
 * em `tauri.conf.json`) e que a tela Sobre/Licenças mostra.
 *
 * Fontes: `cargo metadata` (Rust), `pnpm licenses list --prod --json` (npm) e o inventário manual de
 * assets e modelos em `scripts/lib/license-inventory.mts`. O cabeçalho carrega o hash dos lockfiles
 * e do inventário: `check:license-policy` reprova o arquivo quando eles mudam e ele não foi
 * regenerado.
 *
 * Uso: `pnpm notices:gen`. Não escreve nada se alguma dependência violar a política de licença.
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  type Component,
  collectNpm,
  collectRust,
  type Delivery,
  inputHash,
  MANUAL_ASSETS,
  NOTICES_FILE,
  REPO_ROOT,
  sha256,
  TEXTS_DIR,
} from '../lib/license-inventory.mts'
import { evaluateLicense } from '../lib/license-policy.mts'

const DELIVERY_LABEL: Record<Delivery, string> = {
  bundled: 'dentro do instalador',
  'on-demand': 'baixado sob demanda (ação do usuário)',
  'user-supplied': 'fornecido pelo usuário, não distribuído',
  'steam-build': 'somente no build Steam',
  'not-shipped': 'não distribuído',
}

const HEADER = `# Avisos de software de terceiros

> **RASCUNHO — requer revisão jurídica antes da publicação.**

O Mika é um produto proprietário de [RAZÃO SOCIAL]. Ele inclui, ou baixa por ação do usuário,
componentes de terceiros distribuídos sob as licenças abaixo. Este arquivo cumpre as exigências de
atribuição dessas licenças. Nada aqui concede direitos sobre o Mika em si — para isso vale o EULA.

Arquivo gerado por \`scripts/generators/third-party-notices.mts\` (\`pnpm notices:gen\`). Não edite à
mão: a próxima geração apaga a edição.
`

const cell = (value: string): string => value.replace(/\|/g, '\\|').replace(/\s+/g, ' ').trim()
const fence = (text: string): string => `\`\`\`text\n${text.replace(/```/g, "'''")}\n\`\`\``

function manualSection(): string {
  const items = MANUAL_ASSETS.map(
    (asset) =>
      `### ${asset.name}\n\n` +
      `- **Uso:** ${asset.role}\n` +
      `- **Licença:** ${asset.license}\n` +
      `- **Titular:** ${asset.holder}\n` +
      `- **Origem:** ${asset.source}\n` +
      `- **Distribuição:** ${DELIVERY_LABEL[asset.delivery]}\n` +
      `- **Nota:** ${asset.note}\n`,
  )
  return `## 1. Assets, modelos e runtimes\n\nInventário conferido na fonte de cada item. Itens marcados "verificar" ou "NÃO verificada" são pendências jurídicas abertas.\n\n${items.join('\n')}`
}

function weakCopyleftSection(components: Component[]): string {
  const weak = components.filter((component) => evaluateLicense(component.license, component.name).weakCopyleft)
  const lines = weak.map(
    (component) =>
      `- ${component.name} ${component.version} (${component.license}) — código-fonte: ${component.repository || 'ver registro do pacote'}`,
  )
  return (
    '## 2. Componentes com copyleft fraco\n\n' +
    'Usados sem modificação e vinculados como bibliotecas. O código-fonte original está disponível no endereço indicado; ' +
    'quem quiser o fonte exato desta versão pode pedi-lo em [E-MAIL DE SUPORTE].\n\n' +
    `${lines.join('\n') || '(nenhum)'}\n`
  )
}

function dependencyTable(title: string, components: Component[]): string {
  const rows = components.map(
    (component) => `| ${cell(component.name)} | ${component.version} | ${cell(component.license)} |`,
  )
  return `## ${title} (${components.length})\n\n| Pacote | Versão | Licença |\n|---|---|---|\n${rows.join('\n')}\n`
}

type TextGroup = { text: string; users: Set<string> }

function addToGroup(groups: Map<string, TextGroup>, text: string, user: string): void {
  const key = sha256(text)
  const group = groups.get(key) ?? { text, users: new Set<string>() }
  group.users.add(user)
  groups.set(key, group)
}

/** Agrupa componentes que carregam o MESMO texto (inclusive a linha de copyright). */
function licenseTexts(components: Component[]): string {
  const groups = new Map<string, TextGroup>()
  const missing = new Map<string, string[]>()

  for (const component of components) {
    const label = `${component.name} ${component.version}`
    for (const text of component.texts) addToGroup(groups, text, label)
    if (component.texts.length === 0) {
      const list = missing.get(component.license) ?? []
      list.push(label)
      missing.set(component.license, list)
    }
  }

  for (const asset of MANUAL_ASSETS) {
    if (asset.textFile === undefined) continue
    const text = readFileSync(join(REPO_ROOT, TEXTS_DIR, asset.textFile), 'utf8')
      .replace(/\r\n/g, '\n')
      .trim()
    addToGroup(groups, text, asset.name)
  }

  const ordered = [...groups.values()].sort((a, b) => [...a.users][0]?.localeCompare([...b.users][0] ?? '') ?? 0)
  const blocks = ordered.map(
    (group, index) =>
      `### Texto ${index + 1}\n\nAplica-se a: ${[...group.users].sort().join(', ')}\n\n${fence(group.text)}\n`,
  )
  const absent = [...missing.entries()].map(
    ([license, users]) =>
      `- ${license} — texto de referência em https://spdx.org/licenses/ — pacotes: ${users.join(', ')}`,
  )
  return (
    `## 5. Textos de licença\n\n${blocks.join('\n')}\n` +
    `### Pacotes sem arquivo de licença no código-fonte\n\nO pacote declara a licença SPDX abaixo no manifesto, sem distribuir o arquivo.\n\n${absent.join('\n') || '(nenhum)'}\n`
  )
}

function violationsOf(components: Component[]): string[] {
  return components.flatMap((component) => {
    const verdict = evaluateLicense(component.license, component.name)
    return verdict.ok
      ? []
      : [`${component.ecosystem}:${component.name}@${component.version} — ${verdict.problems.join('; ')}`]
  })
}

function renderNotices(): { markdown: string; violations: string[] } {
  const rust = collectRust(true)
  const npm = collectNpm(true)
  const all = [...rust, ...npm]
  const violations = violationsOf(all)
  const markdown = [
    HEADER,
    `<!-- mika-notices-input-sha256: ${inputHash()} -->\n`,
    manualSection(),
    weakCopyleftSection(all),
    dependencyTable('3. Dependências Rust', rust),
    dependencyTable('4. Dependências npm (produção)', npm),
    licenseTexts(all),
  ].join('\n')
  return { markdown, violations }
}

const { markdown, violations } = renderNotices()
if (violations.length > 0) {
  process.stderr.write(`notices:gen — ${violations.length} dependência(s) fora da política de licença:\n`)
  for (const violation of violations) process.stderr.write(`  ${violation}\n`)
  process.exit(1)
}
writeFileSync(join(REPO_ROOT, NOTICES_FILE), `${markdown}\n`)
process.stdout.write(`notices:gen — ${NOTICES_FILE} escrito (${(markdown.length / 1024).toFixed(0)} KiB)\n`)
