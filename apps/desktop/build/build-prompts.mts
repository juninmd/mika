/**
 * Junta todo `prompts/*.md` de módulo em `out/prompts`, que é o que o bundle do Tauri leva como
 * recurso e o que o Rust resolve em runtime (`prompts_root` em `chat/prompt.rs`, `health.rs` e
 * `brain/reflection.rs`).
 *
 * Existe porque este passo era um efeito colateral de `build-sidecar.mts`, apagado em 2026-09-22
 * junto com o sidecar. O diretório `out/sidecar/prompts` continuava no disco de quem já tinha
 * buildado antes, então nada quebrava LOCALMENTE — e num checkout limpo a Mika subiria com todo
 * prompt vazio. Essa falha não dá erro: o `.md` ausente vira string vazia, a requisição sai sem
 * system prompt, o provedor recusa com 400 e o modelo é reescrito (já aconteceu neste repo).
 *
 * A cópia é dupla de propósito, igual à do sidecar: plana (`analyze-screen.prompt.md`) porque os
 * loaders pedem pelo nome do arquivo, e sob `namespace/` porque `loadPrompt('brain', …)` pede
 * pelo par módulo+nome. Duas cópias de 60 arquivos de texto custam menos que uma regra sobre qual
 * loader usa qual forma.
 */
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { discoverPromptDirEntries } from './prompt-dirs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const outPrompts = resolve(root, 'out/prompts')

rmSync(outPrompts, { recursive: true, force: true })
mkdirSync(outPrompts, { recursive: true })

let copied = 0
for (const { namespace, dir } of discoverPromptDirEntries(root)) {
  const from = resolve(root, dir)
  if (!existsSync(from)) continue
  // Flat copy is root-level `.md` only: a nested folder (`brain/awareness/`) copied here would be
  // listed by the editor as a fake namespace, and loaders would read it instead of the override.
  for (const name of readdirSync(from)) {
    if (name.endsWith('.md')) copyFileSync(resolve(from, name), resolve(outPrompts, name))
  }
  cpSync(from, resolve(outPrompts, namespace), { recursive: true })
  copied += 1
}

if (copied === 0) {
  throw new Error(
    'nenhuma pasta de prompts encontrada — o bundle sairia sem system prompt, que é falha muda em runtime',
  )
}

console.log(`prompts: ${copied} pasta(s) -> ${outPrompts}`)
