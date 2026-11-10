/**
 * Relatório de homologação — os campos exigidos pela §10.3 do plano modular.
 *
 * O JSON é o que a árvore guarda (e o que `check:homologation-freshness` reconfere); o markdown é
 * para quem lê. Nenhum dos dois carrega segredo: o ambiente entra como SO/runtime, nunca config.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve as resolvePath } from 'node:path'
import { REPO_ROOT } from '../checks/lib.mts'
import type { ArtifactIdentity } from './artifact.mts'
import type { CriterionResult, Profile } from './profile.mts'
import { REQUIRED } from './profile.mts'

export const REPORT_DIR = 'docs/homologation'

export interface HomologationReport {
  identidade: { id: string; titulo: string; perfil: Profile }
  artefato: ArtifactIdentity
  ambiente: { plataforma: string; node: string; data: string }
  criterios: CriterionResult[]
  estado: 'aprovado' | 'reprovado'
  /** O que este perfil NÃO cobre. Lacuna registrada é lacuna; lacuna omitida vira aprovação falsa. */
  limites: string[]
}

const LIMITS: Record<Profile, string[]> = {
  library: ['UI, IPC e empacotamento não são cobertos por este perfil — exigem o perfil de integração Desktop.'],
  adapter: ['Sem prova de carga, GPU ou matriz por SO; o E2E cobre montagem, não desempenho.'],
  contract: ['Só contratos e tipos: o comportamento por trás deles é provado no backend Rust e no E2E.'],
}

export function buildReport(input: {
  id: string
  titulo: string
  perfil: Profile
  artefato: ArtifactIdentity
  criterios: CriterionResult[]
  estado: 'aprovado' | 'reprovado'
}): HomologationReport {
  return {
    identidade: { id: input.id, titulo: input.titulo, perfil: input.perfil },
    artefato: input.artefato,
    ambiente: { plataforma: process.platform, node: process.version, data: new Date().toISOString().slice(0, 10) },
    criterios: input.criterios,
    estado: input.estado,
    limites: LIMITS[input.perfil],
  }
}

export function writeReport(report: HomologationReport): string {
  const dir = resolvePath(REPO_ROOT, REPORT_DIR)
  mkdirSync(dir, { recursive: true })
  const rel = `${REPORT_DIR}/${report.identidade.id}.json`
  writeFileSync(resolvePath(REPO_ROOT, rel), `${JSON.stringify(report, null, 2)}\n`, 'utf8')
  return rel
}

const MARK: Record<CriterionResult['status'], string> = { ok: 'ok', falhou: 'FALHOU', ausente: 'AUSENTE' }

export function renderMarkdown(report: HomologationReport, savedAt: string): string {
  const { identidade, artefato } = report
  const required = REQUIRED[identidade.perfil]
  const lines = [
    `# Homologação — ${identidade.titulo} (\`${identidade.id}\`)`,
    '',
    `- **perfil**: ${identidade.perfil}`,
    `- **artefato**: commit \`${artefato.commit}\`, árvore \`${artefato.tree}\`, contrato \`${artefato.contract}\``,
    `- **ambiente**: ${report.ambiente.plataforma}, node ${report.ambiente.node}`,
    '',
  ]
  for (const c of report.criterios) {
    const tag = required.includes(c.id) ? 'obrigatório' : 'informativo'
    lines.push(`- **${c.id}** (${tag}): ${MARK[c.status]} — ${c.detail}`)
    if (c.evidence) lines.push('', '```', c.evidence, '```', '')
  }
  lines.push('', `**Estado: ${report.estado}.** Relatório em \`${savedAt}\`.`)
  for (const limite of report.limites) lines.push(`- limite: ${limite}`)
  return `${lines.join('\n')}\n`
}
