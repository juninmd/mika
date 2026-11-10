/**
 * check:file-naming — o nome do arquivo segue AGENTS.md §2?
 *
 * Duas regras de §2 que se respondem só pelo NOME, sem ler o arquivo:
 *
 *  1. `index` é reservado para o barril — nada de `index.<algo>.ts`;
 *  2. PascalCase só para componente React — um `.ts`/`.mts` (sem JSX) é kebab-case.
 *
 * A regra 2 existia só como prosa e `components/live2d/` acumulou 25 arquivos fora dela
 * (`Live2DAnimator.ts`, `Live2DAnimatorBlink.ts`, `PlaceholderAvatar.ts`, …), enquanto os testes
 * do MESMO diretório já eram `live2d-blink.test.ts`. Quem procurava "o arquivo do blink" tinha de
 * tentar as duas grafias, e em sistema de arquivos que ignora caixa (Windows, macOS) um import com
 * a caixa errada passa local e quebra na CI do Linux.
 *
 * Fica de fora de propósito: teste que leva o nome do componente que prova
 * (`ProviderChoiceGrid.test.ts` ao lado de `ProviderChoiceGrid.tsx`), e o sufixo com ponto
 * (`.service.ts`, não `-service.ts`) — este não é decidível pelo nome: `define-module.ts` e
 * `tauri-renderer-runtime.ts` terminam em hífen + palavra reservada e estão certos.
 */
import { isSource, isTest, report, sourceRoots, type Violation, walk } from './lib.mts'

const ROOTS = [...sourceRoots(), 'apps/desktop/e2e', 'scripts']

const files = ROOTS.flatMap((root) => walk(root, isSource))
const violations: Violation[] = []

for (const file of files) {
  const name = file.slice(file.lastIndexOf('/') + 1)

  if (/^index\..+\.(ts|tsx|mts|mjs)$/.test(name)) {
    violations.push({ file, line: 1, message: '`index` é reservado para o barril; dê ao arquivo o nome do que ele é' })
  }

  if (/^[A-Z]/.test(name) && !name.endsWith('.tsx') && !isTest(file)) {
    violations.push({
      file,
      line: 1,
      message: 'PascalCase só para componente React (.tsx); arquivo sem JSX é kebab-case',
    })
  }
}

report('check:file-naming', violations, files.length)
