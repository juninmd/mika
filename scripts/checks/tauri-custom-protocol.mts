/**
 * check:tauri-custom-protocol — o shell declara a feature que separa produção de desenvolvimento?
 *
 * `tauri::is_dev()` é literalmente `!cfg!(feature = "custom-protocol")` (tauri-2.11.6/src/lib.rs:309).
 * Quem não declara a feature nunca a liga, e o app SE CONSIDERA em desenvolvimento para sempre:
 * carrega `build.devUrl` da tauri.conf.json em vez do bundle que o `tauri-build` embarcou —
 * inclusive num binário compilado com `--release`.
 *
 * Não é hipótese. O primeiro boot real desta migração abriu as duas janelas em BRANCO, e o motivo
 * só apareceu anexando um depurador ao WebView2: o exe de `target/release/` estava carregando
 * `http://localhost:5273`. Nenhum gate podia ver — compila, tipa, `cargo test` passa, o `pnpm build`
 * do renderer fica verde, e o bundle embarcado está lá, correto, sendo ignorado. Sem dev server no
 * ar (o caso de qualquer usuário) a janela fica branca e o app não diz uma palavra.
 *
 * O gate confere três pontas, porque cada uma sozinha passa verde à toa:
 *  1. a feature existe em `[features]` e encadeia para `tauri/custom-protocol`;
 *  2. quem empacota para valer usa o CLI do Tauri, não `cargo build` — só o CLI liga a feature
 *     (e só ele copia `externalBin`/`resources`). Vale para o `package.json`, os workflows de CI e
 *     os scripts de `steam/`;
 *  3. o `beforeBuildCommand` reconstrói TODA metade que o bundle leva pronta — hoje o renderer e
 *     os prompts. Na rodada de 2026-09-21 ele só construía o renderer, e o `tauri build`
 *     empacotava qualquer bundle de backend que estivesse no disco: o app subiu com um backend de
 *     37 minutos antes, anterior às correções, e os sintomas apontavam para código já consertado.
 */

import { readdirSync, readFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '../..')
const CARGO = join(ROOT, 'apps/desktop/src-tauri/Cargo.toml')
const CONF = join(ROOT, 'apps/desktop/src-tauri/tauri.conf.json')
const PKG = join(ROOT, 'apps/desktop/package.json')

type Violation = { file: string; message: string }
const violations: Violation[] = []
const rel = (file: string) => relative(ROOT, file).replaceAll('\\', '/')

const cargo = readFileSync(CARGO, 'utf8')
const conf = JSON.parse(readFileSync(CONF, 'utf8')) as {
  build?: { devUrl?: string; frontendDist?: string; beforeBuildCommand?: string }
}
const pkg = JSON.parse(readFileSync(PKG, 'utf8')) as { scripts?: Record<string, string> }

/* Piso de alcance: sem `devUrl` o invariante não morde, e o gate passaria verde sem conferir nada. */
if (!conf.build?.devUrl) {
  violations.push({
    file: rel(CONF),
    message:
      'build.devUrl não existe — este gate existe para o caso em que ele existe; ou o campo mudou de nome, ou o gate perdeu o alvo',
  })
}

/* Piso de alcance: sem `frontendDist` não há bundle embarcado para o app preferir. */
if (!conf.build?.frontendDist) {
  violations.push({
    file: rel(CONF),
    message: 'build.frontendDist não existe — não há bundle embarcado que o modo dev possa estar ignorando',
  })
}

if (!/^\s*custom-protocol\s*=\s*\[[^\]]*"tauri\/custom-protocol"/m.test(cargo)) {
  violations.push({
    file: rel(CARGO),
    message:
      'feature `custom-protocol = ["tauri/custom-protocol"]` ausente — `tauri::is_dev()` é `!cfg!(feature = "custom-protocol")`, ' +
      'então sem ela TODO build (mesmo --release) carrega build.devUrl e a janela abre em branco na máquina do usuário',
  })
}

/* O empacotamento de verdade passa pelo CLI: `cargo build` não liga a feature nem copia os recursos. */
const buildScript = pkg.scripts?.['tauri:build']
if (!buildScript) {
  violations.push({
    file: rel(PKG),
    message: 'script `tauri:build` sumiu — o gate deixou de saber por onde o app é empacotado',
  })
} else if (!/(^|\s)tauri\s+build(\s|$)/.test(buildScript)) {
  violations.push({
    file: rel(PKG),
    message: `\`tauri:build\` não usa o CLI do Tauri ("${buildScript}") — só \`tauri build\` liga custom-protocol e copia externalBin/resources`,
  })
}

/*
 * Tudo que o `tauri build` empacota e não compila junto tem de nascer na MESMA rodada, ou ele leva
 * o que estiver no disco, de qualquer idade.
 *
 * Eram duas metades: o renderer e o bundle do backend Node (`externalBin`). O sidecar caiu em
 * 2026-09-22 e levou `build:sidecar` junto — mas `copyPrompts` era efeito colateral DELE, e os 60
 * `.md` continuavam no disco de quem já tinha buildado antes. Num checkout limpo o app subiria com
 * todo prompt vazio, que é falha muda: `.md` ausente vira string vazia, a requisição sai sem
 * system prompt e o provedor recusa. Por isso `build:prompts` entrou aqui no lugar — a exigência
 * não é "tem sidecar", é "toda metade não-compilada é reconstruída".
 */
const before = conf.build?.beforeBuildCommand ?? ''
for (const required of ['build:renderer', 'build:prompts']) {
  if (!before.includes(required)) {
    violations.push({
      file: rel(CONF),
      message: `beforeBuildCommand não chama \`${required}\` ("${before}") — o \`tauri build\` empacotaria a metade que estiver velha no disco`,
    })
  }
}

/*
 * A ponta 2 também vale para quem NÃO é o `package.json`: os workflows e os scripts do Steam. Um
 * `cargo build --release` num passo de CI produz um binário que se acha em desenvolvimento (janela
 * em branco) e sem `bundle.resources` — exatamente o defeito acima, num lugar que ninguém roda no
 * dia a dia. Comentários (`#`) ficam de fora: é onde o motivo é explicado.
 */
const CARGO_RELEASE = /\bcargo\s+build\b.*(--release|\s-r\b)/
const listDir = (dir: string, accept: (name: string) => boolean) =>
  readdirSync(join(ROOT, dir))
    .filter(accept)
    .map((name) => `${dir}/${name}`)
const packagingScripts = [
  ...listDir('.github/workflows', (name) => /\.ya?ml$/.test(name)),
  ...listDir('steam', (name) => !name.endsWith('.md')),
]
for (const file of packagingScripts) {
  readFileSync(join(ROOT, file), 'utf8')
    .split(/\r?\n/)
    .forEach((line, index) => {
      if (line.trimStart().startsWith('#') || !CARGO_RELEASE.test(line)) return
      violations.push({
        file: `${file}:${index + 1}`,
        message:
          '`cargo build --release` empacota sem o CLI do Tauri — use `tauri build` (liga custom-protocol e copia resources)',
      })
    })
}

/*
 * Ponta 4: a feature `e2e` (ganchos de teste: `mika:e2e-hook`, override da Wikipedia) existe e NUNCA
 * entra num empacotamento que vai ao usuário. Sem a feature o código nem é compilado; com ela, qualquer
 * processo que defina `MIKA_E2E=1` ganha publicar evento no barramento, sinal de energia, `quit`. O
 * único script que a liga é o `build:e2e` do `package.json`; workflow e `steam/` não podem.
 */
if (!/^\s*e2e\s*=\s*\[/m.test(cargo)) {
  violations.push({
    file: rel(CARGO),
    message: 'feature `e2e = [...]` ausente — sem ela os ganchos de teste voltam a ser compilados incondicionalmente',
  })
}
const E2E_FEATURE = /--features?[ =]\S*\be2e\b/
for (const file of packagingScripts) {
  readFileSync(join(ROOT, file), 'utf8')
    .split(/\r?\n/)
    .forEach((line, index) => {
      if (line.trimStart().startsWith('#') || !E2E_FEATURE.test(line)) return
      violations.push({
        file: `${file}:${index + 1}`,
        message:
          '`--features e2e` num empacotamento distribuído — o binário carregaria os ganchos de teste (mika:e2e-hook)',
      })
    })
}
for (const [name, script] of Object.entries(pkg.scripts ?? {})) {
  if (name !== 'build:e2e' && E2E_FEATURE.test(script)) {
    violations.push({
      file: rel(PKG),
      message: `script \`${name}\` liga a feature e2e — só \`build:e2e\` pode (binário de teste, nunca distribuído)`,
    })
  }
}

if (violations.length > 0) {
  console.error(`✗ check:tauri-custom-protocol — ${violations.length} violação(ões)`)
  for (const violation of violations) console.error(`  ${violation.file}  ${violation.message}`)
  process.exit(1)
}

console.log(
  '✓ check:tauri-custom-protocol — feature declarada, empacotamento pelo CLI e `e2e` fora dos pacotes distribuídos',
)
