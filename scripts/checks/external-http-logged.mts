/**
 * check:external-http-logged — pega saída de rede fora do funil observado: `fetch` cru (rede sai pelo
 * Rust, `browser/guarded_fetch`, desde que o legado Node saiu) e processo filho (modo offline já vazou fala inteira pro edge-tts por
 * essa porta), cujo inventário aqui é conferido nos dois sentidos.
 */
import { isSource, isTest, lineOf, read, report, sourceRoots, type Violation, walk } from './lib.mts'

/**
 * O que cada arquivo do inventário executa, conferido linha a linha.
 *
 * Stage C (2026-09-22): `src/main` inteiro e o backend não-renderer de `src/features/*` saíram —
 * child_process de produção agora é `std::process::Command` em Rust (`src-tauri/`), fora do
 * alcance deste gate TypeScript. Os sites antigos (Steam, MCP client, shell/input adapters,
 * foreground/window enumeration, Piper) morreram junto; o último (`headless-adapters.ts`) saiu com o
 * legado Node do SDK (2026-09-23). Inventário vazio: qualquer processo filho em TS é violação.
 */
const CHILD_PROCESS_SITES: ReadonlyMap<string, string> = new Map()

const files = [
  ...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel)),
  ...sourceRoots()
    .filter((root) => root.startsWith('packages/'))
    .flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel))),
]

/** `.test()` com regex global é stateful: cada padrão é reconstruído por arquivo, de propósito. */
const rawFetch = () => /(?<![.\w])fetch\s*\(/g
/**
 * Detecta pelo IMPORT, não pelo nome da chamada. Procurar `spawn(`/`execFile(` deixava passar
 * `exec()` — que roda com shell, é ainda mais poderoso, e é o que a leitura de janela ativa usa —
 * e também qualquer alias local (`const execAsync = promisify(exec)`). Quem importa
 * `node:child_process` — ou `child_process` sem o prefixo, que o Node resolve igual — cria processo, ponto.
 */
const spawns = () => /child_process|utilityProcess\.fork\s*\(|new Experimental_StdioMCPTransport\s*\(/g

const violations: Violation[] = []
const seen = new Set<string>()

for (const file of files) {
  const source = read(file)

  for (const match of source.matchAll(rawFetch())) {
    const index = match.index ?? 0
    const before = source.slice(Math.max(0, index - 40), index)
    if (/typeof\s+$|:\s*$/.test(before)) continue

    violations.push({
      file,
      line: lineOf(source, index),
      message: 'fetch cru — rede sai pelo backend Rust (guarded_fetch: log, URL redigida, modo offline)',
    })
  }

  const spawnSite = source.match(spawns())
  if (spawnSite === null) continue
  seen.add(file)

  if (!CHILD_PROCESS_SITES.has(file)) {
    violations.push({
      file,
      line: lineOf(source, source.search(spawns())),
      message:
        'cria processo filho fora do inventário de scripts/checks/external-http-logged.mts — se o executável fala com a internet, ele precisa decidir sobre rede antes',
    })
  }
}

for (const site of CHILD_PROCESS_SITES.keys()) {
  if (!seen.has(site)) {
    violations.push({
      file: 'scripts/checks/external-http-logged.mts',
      line: 1,
      message: `inventário cita "${site}", que não cria mais processo filho — remova a entrada antes que ela vire decoração`,
    })
  }
}

report('check:external-http-logged', violations, files.length)
