/**
 * check:homologation-freshness — pega relatório de homologação salvo que não bate mais com o
 * artefato atual (tree/contract/deps mudaram), e módulo que nunca foi homologado. `frozen.hash` só
 * protege a API pública, não a implementação testada.
 */
import { identify } from '../homologation/artifact.mts'
import { loadManifests, type Manifest, ownedBy } from '../homologation/ownership.mts'
import type { HomologationReport } from '../homologation/report.mts'
import { REPORT_DIR } from '../homologation/report.mts'
import { read, report, type Violation, walk } from './lib.mts'

const entries = await loadManifests()
const manifests = new Map<string, Manifest>(entries.map((entry) => [entry.manifest.id, entry.manifest]))
const sources = entries.map((entry) => entry.manifest.source)

const reports = walk(REPORT_DIR, (rel) => rel.endsWith('.json'))
const violations: Violation[] = []
const reported = new Set<string>()

for (const file of reports) {
  const stored = JSON.parse(read(file)) as HomologationReport
  const id = stored.identidade.id
  reported.add(id)
  const manifest = manifests.get(id)
  if (!manifest) {
    violations.push({ file, line: 1, message: `relatório de "${id}", que não existe mais como módulo` })
    continue
  }

  if (stored.estado !== 'aprovado') {
    violations.push({ file, line: 1, message: `homologação de "${id}" está ${stored.estado}` })
    continue
  }

  const atual = identify(manifest.source, manifest.contract, ownedBy(manifest.source, sources))
  const mudou: string[] = (['tree', 'contract'] as const).filter((campo) => atual[campo] !== stored.artefato[campo])
  if (JSON.stringify(atual.deps) !== JSON.stringify(stored.artefato.deps)) mudou.push('deps')

  if (mudou.length > 0) {
    violations.push({
      file,
      line: 1,
      message: `homologação de "${id}" é de outro artefato (${mudou.join(', ')} mudou). Rode \`pnpm homologate ${id}\``,
    })
  }
}

// Sem relatório o módulo nunca provou nada; antes disso ele passava por não ter o que comparar.
for (const { file, manifest } of entries) {
  if (!reported.has(manifest.id)) {
    violations.push({
      file,
      line: 1,
      message: `"${manifest.id}" nunca foi homologado. Rode \`pnpm homologate ${manifest.id}\``,
    })
  }
}

report('homologation-freshness', violations, reports.length)
