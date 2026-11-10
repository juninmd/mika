/**
 * check:module-ownership — todo arquivo de produção tem um módulo dono. Arquivo fora de qualquer
 * `source` não entra em homologação nenhuma: em 2026-09-19 eram 613 de 1293, incluindo voice,
 * vision, preload e o renderer inteiro, todos "homologados" só no nome.
 */
import { loadManifests, ownerOf } from '../homologation/ownership.mts'
import { isSource, isTest, report, sourceRoots, type Violation, walk } from './lib.mts'

const sources = (await loadManifests()).map((entry) => entry.manifest.source)
const files = sourceRoots().flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))

const violations: Violation[] = files
  .filter((rel) => ownerOf(rel, sources) === undefined)
  .map((file) => ({ file, line: 1, message: 'nenhum módulo é dono deste arquivo; ele fica fora da homologação' }))

report('check:module-ownership', violations, files.length)
