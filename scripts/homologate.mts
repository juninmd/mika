/**
 * pnpm homologate <id> — homologa UM módulo, sem subir o app; prova intermediária entre o gate de
 * arquivo e o E2E completo. Provas rodam por PERFIL (`homologation/profile.mts`); saída é o relatório
 * JSON em `docs/homologation/<id>.json` (conferido por `check:homologation-freshness`) + markdown.
 */

import { isTest, walk } from './checks/lib.mts'
import { identify } from './homologation/artifact.mts'
import { configUi, contrato, fronteira, headless, importPublico, montagem, vivo } from './homologation/criteria.mts'
import { childSources, loadManifests, ownedBy } from './homologation/ownership.mts'
import { type CriterionResult, profileOf, verdict } from './homologation/profile.mts'
import { buildReport, renderMarkdown, writeReport } from './homologation/report.mts'

const id = process.argv[2]
if (!id) {
  process.stdout.write('uso: pnpm homologate <id-do-modulo>\n')
  process.exit(1)
}

const entries = await loadManifests()
const manifest = entries.find((entry) => entry.manifest.id === id)?.manifest

if (!manifest) {
  process.stdout.write(`módulo "${id}" não encontrado entre os ${entries.length} manifestos da árvore\n`)
  process.exit(1)
}

const sources = entries.map((entry) => entry.manifest.source)
const owns = ownedBy(manifest.source, sources)
const perfil = profileOf(manifest.source, manifest.profile)

const criterios: CriterionResult[] = [fronteira(manifest.id), vivo(manifest.source), await headless(manifest.contract)]
if (perfil === 'library') {
  criterios.push(await importPublico(manifest.source), montagem(manifest.source, owns))
}
if (perfil === 'contract') criterios.push(await importPublico(manifest.source))
const schemaFiles = walk(manifest.source, (rel) => owns(rel) && rel.endsWith('.schema.ts') && !isTest(rel))
if (schemaFiles.length > 0) criterios.push(await configUi(schemaFiles))
criterios.push(contrato(manifest.source, childSources(manifest.source, sources), manifest.rust))

const estado = verdict(perfil, criterios)
const report = buildReport({
  id: manifest.id,
  titulo: manifest.title,
  perfil,
  artefato: identify(manifest.source, manifest.contract, owns),
  criterios,
  estado,
})

const savedAt = writeReport(report)
process.stdout.write(renderMarkdown(report, savedAt))
process.exit(estado === 'aprovado' ? 0 : 1)
