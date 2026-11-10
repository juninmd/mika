# 12a — Migração Live2D: pixi.js v6 → v8 (spike 2026-09-26, NÃO migrado)

Apêndice de [12-avatar-render.md](./12-avatar-render.md). Registra o resultado do feasibility spike
do work package Live2D-at-VRM-parity: por que a migração não aconteceu nesta rodada e o que falta
para decidir de verdade.

**Contexto:** `pixi-live2d-display@0.4.0` (peer `pixi.js ^6`) não publica desde 2022-09-04; `pixi.js`
está travado em `6.5.10`, dois majors atrás do `8.21.0` atual. `untitled-pixi-live2d-engine@1.4.0`
(MIT, peer `pixi.js ^8.13.1`, Cubism 2-5, `push` 6 dias antes desta rodada) é o único fork vivo que
salta direto para pixi v8 — `pixi-live2d-display-advanced` fica em v7, um passo intermediário.

**API do model (o que os 14 arquivos `Live2DAnimator*.ts` tocam) é quase idêntica** — confirmado lendo
o arquivo de tipos (index.d.ts, pasta types) do pacote empacotado (não a documentação): `Live2DModel.motion()`/`.expression()`,
`internalModel.coreModel.setParameterValueById`/`getParameterValueById`, `internalModel.motionManager
.stopAllMotions()`, `internalModel.settings.name`/`.url`, `internalModel.width`/`.height` — todos
presentes com a mesma assinatura. `live2d-animator.ts` e os 13 arquivos irmãos não precisariam mudar.

**O que quebra, além de `live2d-loader.ts`:**
- `PIXI.Application` vira `new Application()` + `await app.init(options)` assíncrono (v6 era
  construtor síncrono) e `app.view` vira `app.canvas` — `live2d-stage.ts` monta a ordem
  `containerRef.appendChild → carregar modelo → ligar ticker` de forma síncrona hoje; o `mount`
  inteiro precisa virar assíncrono, não só o loader.
- É preciso `extensions.add(Live2DPlugin)` **antes** de criar o `Application` — passo novo, sem
  equivalente hoje.
- `PIXI.Graphics` (`beginFill`/`drawCircle`/`endFill`, `lineStyle`+`arc`) e o construtor posicional de
  `PIXI.Text(str, style)` foram removidos em v8 (API fluente + `Text({ text, style })`) —
  `placeholder-avatar.ts` (o fallback quando o modelo falha ao carregar) precisa de reescrita completa,
  não só ajuste de import.

**Bloqueio real, verificado — não especulado:** `apps/desktop/src-tauri/tauri.conf.json` fixa
`script-src 'self'` (sem `unsafe-eval`). `@pixi/unsafe-eval` está no `package.json` exatamente por
isso — ele substitui a geração de código via `new Function()` que o pixi.js usa internamente por uma
alternativa compatível com esse CSP. Conferido no npm: `@pixi/unsafe-eval` nunca publicou build para
pixi.js v8 (`npm view @pixi/unsafe-eval versions` para no `7.4.3`, cujo `peerDependencies` ainda pede
`@pixi/core@7.4.3`). **Não verificado nesta rodada** (E2E proibido neste run, e é a única forma real
de checar): se o renderer do pixi.js v8 ainda gera código via `Function()` em algum caminho (batching,
geometria) sob este CSP, o canvas quebra em silêncio — sem erro de typecheck, sem teste unitário, só
um console CSP violation e uma tela preta que só um screenshot pegaria. A alternativa (adicionar
`unsafe-eval` ao CSP) é regressão de segurança que esta rodada não tem mandato para decidir sozinha.

**Decisão:** não migrar agora. Escopo de arquivo é contido (7 arquivos, todos dentro da área do
Live2D — `live2d-loader.ts`, `live2d-stage.ts`, `live2d-stage-model.ts`, `Live2DCanvas.tsx`,
`placeholder-avatar.ts`, `live2d-action-catalog.generated.ts`, `pixi-unsafe-eval.ts` —, não
renderer-wide), mas o bloqueio de CSP/`unsafe-eval` só se resolve rodando o binário real sob esse CSP
e lendo o console — exatamente o que este run não permite. Migrar sem essa checagem arrisca trocar
"dependência travada há 2 anos" por "avatar quebrado em produção, descoberto só no primeiro
screenshot".

**Próximo passo concreto:** rodada com E2E liberado — `tauri build --no-bundle`, subir
`untitled-pixi-live2d-engine` + `pixi.js@8` num branch descartável, abrir o app real e ler o console
do devtools por violação de CSP ao interagir com o avatar. Se limpo, o resto (reescrever
`live2d-stage.ts` + `placeholder-avatar.ts`) é o trabalho mecânico já mapeado acima.
