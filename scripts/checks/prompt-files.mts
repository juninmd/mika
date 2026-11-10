/**
 * check:prompt-files — os prompts markdown em `apps/desktop/prompts/<namespace>/*.md` (carregados em
 * Rust: `modules/chat/prompt.rs`, `modules/brain/prompt_loader.rs`) e três coisas que já quebraram
 * em silêncio:
 *
 * 1. pasta de prompts fora do bundle (loader vazio já derrubou um provedor inteiro por request de
 *    zero token) — o build tem de DESCOBRIR as pastas, não listá-las à mão;
 * 2. `{{x}}` num `.md` que nenhum código Rust preenche: o render troca por string vazia e só um
 *    `log::warn` avisa. Heurística: `"x"` tem de aparecer como literal no fonte Rust de produção
 *    (antes do `#[cfg(test)]` de cada arquivo) — quem passa a variável a escreve como chave de `vars`, ou
 *    como `.replace("{{x}}", …)` num prompt embutido com `include_str!`;
 * 3. o PISO DE SEGURANÇA (`## Safety floor`, crise/menor/nada explícito) em `brain/system.md` e
 *    `brain/system-tail.md`: sem o marcador, o piso some do prompt. O loader recusa override que o
 *    tira; este gate recusa o DEFAULT sem ele. (Antes este gate procurava `loadPrompt` em TypeScript:
 *    0 arquivos conferidos desde a migração Tauri.)
 */
import { resolve } from 'node:path'
import { discoverPromptDirEntries } from '../../apps/desktop/build/prompt-dirs.ts'
import { lineOf, REPO_ROOT, read, report, type Violation, walk } from './lib.mts'

const DESKTOP = 'apps/desktop'
const PROMPTS = `${DESKTOP}/prompts`
const BUILD_PROMPTS = `${DESKTOP}/build/build-prompts.mts`
const FLOOR_FILES = [`${PROMPTS}/brain/system.md`, `${PROMPTS}/brain/system-tail.md`]
const FLOOR_MARKER = 'Safety floor'

const violations: Violation[] = []

if (!read(BUILD_PROMPTS).includes('discoverPromptDirEntries(')) {
  violations.push({
    file: BUILD_PROMPTS,
    line: 1,
    message: 'build-prompts.mts precisa usar discoverPromptDirEntries() — lista escrita à mão esquece módulo novo',
  })
}

const markdown = walk(PROMPTS, (rel) => rel.endsWith('.md'))
const discovered = new Set(discoverPromptDirEntries(resolve(REPO_ROOT, DESKTOP)).map((e) => e.dir))
for (const file of markdown) {
  const folder = file.slice(`${DESKTOP}/`.length).split('/').slice(0, 2).join('/')
  if (!discovered.has(folder)) {
    violations.push({
      file,
      line: 1,
      message: `${folder} não é descoberta pelo build — este prompt some do bundle em silêncio`,
    })
  }
}

// Fonte Rust de produção: o que vem antes do primeiro `#[cfg(test)]` (variável só passada em teste não conta).
const rustSource = walk(`${DESKTOP}/src-tauri/src`, (rel) => rel.endsWith('.rs'))
  .map((file) => read(file).split('#[cfg(test)]')[0])
  .join('\n')

for (const file of markdown) {
  const source = read(file)
  for (const match of source.matchAll(/\{\{(\w+)\}\}/g)) {
    if (rustSource.includes(`"${match[1]}"`) || rustSource.includes(match[0])) continue
    violations.push({
      file,
      line: lineOf(source, match.index),
      message: `{{${match[1]}}} não é preenchido por nenhum código Rust (nenhum literal "${match[1]}" nem {{${match[1]}}} no fonte de produção) — vira string vazia no prompt`,
    })
  }
}

for (const file of FLOOR_FILES) {
  if (!read(file).includes(FLOOR_MARKER)) {
    violations.push({
      file,
      line: 1,
      message: `perde o piso de segurança ("${FLOOR_MARKER}") — o loader também o exige dos overrides do usuário`,
    })
  }
}

report('check:prompt-files', violations, markdown.length)
