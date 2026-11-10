import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { analyzeModel, listModelDirs, type ModelAnalysis } from './model-analysis.ts'

const ROOT = join(fileURLToPath(import.meta.url), '..', '..', '..')

const args = process.argv.slice(2)
const asJson = args.includes('--json')
const wanted = args.filter((arg) => !arg.startsWith('--'))

const models = listModelDirs(ROOT).filter((model) => wanted.length === 0 || wanted.includes(model.name))

if (models.length === 0) {
  console.error(
    wanted.length > 0
      ? `❌ Nenhum modelo Live2D encontrado com o nome: ${wanted.join(', ')}`
      : '❌ Nenhum modelo Live2D encontrado em apps/desktop/src/renderer/public/live2d',
  )
  process.exit(1)
}

function pad(text: string, width: number): string {
  return text.length >= width ? text : text + ' '.repeat(width - text.length)
}

function padStart(text: string, width: number): string {
  return text.length >= width ? text : ' '.repeat(width - text.length) + text
}

function printReport(analysis: ModelAnalysis): void {
  console.log(`\n=== ${analysis.name} (${analysis.stem}) ===`)
  console.log(`pasta: ${analysis.dir}`)
  console.log(
    `agregado de ${analysis.referenceMotionCount} motions originais` +
      (analysis.declaredParams ? ` · ${analysis.declaredParams.size} parâmetros no cdi3` : ' · sem cdi3.json'),
  )
  if (analysis.referenceMotionCount === 0) {
    console.log('⚠️  modelo sem motions originais: não há faixa observada nem pose base para conferir.')
  }
  console.log('')

  const header = `${pad('ParamId', 30)} ${padStart('min', 8)} ${padStart('max', 8)} ${padStart('base', 8)}  física  nome`
  console.log(header)
  console.log('-'.repeat(header.length))

  const ids = [...new Set([...(analysis.declaredParams ?? []), ...analysis.stats.keys()])].sort()
  for (const id of ids) {
    const stat = analysis.stats.get(id)
    const range = stat
      ? `${padStart(stat.min.toFixed(2), 8)} ${padStart(stat.max.toFixed(2), 8)} ${padStart(stat.base.toFixed(2), 8)}`
      : `${padStart('-', 8)} ${padStart('-', 8)} ${padStart('-', 8)}`
    const physics = analysis.physicsOutputs.has(id) ? '  ★fis  ' : '        '
    console.log(`${pad(id, 30)} ${range} ${physics} ${analysis.displayNames.get(id) ?? ''}`)
  }

  console.log(
    '\n★fis = destino de saída do physics3.json. Nunca anime direto — mova cabeça/corpo e o cabelo' +
      '\n       balança sozinho. Parâmetro sem min/max nunca é usado pelas motions originais: não há' +
      '\n       faixa observada para ele, então use com parcimônia e confira visualmente.',
  )
}

const analyses = models.map(analyzeModel)

if (asJson) {
  console.log(
    JSON.stringify(
      analyses.map((analysis) => ({
        name: analysis.name,
        stem: analysis.stem,
        referenceMotionCount: analysis.referenceMotionCount,
        physicsOutputs: [...analysis.physicsOutputs].sort(),
        parameters: Object.fromEntries(
          [...analysis.stats.entries()]
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([id, stat]) => [id, { min: stat.min, max: stat.max, base: stat.base, curves: stat.curves }]),
        ),
        declaredOnly: [...(analysis.declaredParams ?? [])].filter((id) => !analysis.stats.has(id)).sort(),
      })),
      null,
      2,
    ),
  )
} else {
  for (const analysis of analyses) printReport(analysis)
}
