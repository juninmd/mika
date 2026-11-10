# 12 — Avatar: Live2D e VRM

Como a Mika se move. Cobre a camada procedural (springs, respiração, piscada, lip-sync), as
**ações** — motions autorais geradas por JSON, sem Cubism Editor — e os **dois backends de
renderização**.

## Dois backends, um contrato

**MUST** — existem dois motores de avatar e os dois são de produção:

| Backend | Stack                                                           | Modelo         |
| ------- | --------------------------------------------------------------- | -------------- |
| Live2D  | `pixi-live2d-display` + Cubism Core (`live2dcubismcore.min.js`) | `.model3.json` |
| VRM     | `three.js` + `@pixiv/three-vrm` (MIT), sem Cubism Core          | `.vrm`         |

A escolha é feita pela **extensão do arquivo de modelo**, em
`apps/desktop/src/renderer/src/AppLayers.tsx` (`isVrmModelPath`, de
`apps/desktop/src/renderer/src/components/avatar/vrm-look.ts`) — `.vrm` monta `VrmCanvas`, qualquer
outra coisa monta `Live2DCanvas`. Os dois componentes recebem exatamente as mesmas props, tipadas
uma vez em `apps/desktop/src/renderer/src/components/avatar/avatar-canvas.types.ts`.

**MUST** — todo motor implementa `AvatarEngine`
(`apps/desktop/src/renderer/src/components/avatar/avatar-engine.ts`). Gate:
`pnpm check:avatar-parity`.

**MUST** — diferença de capacidade entre backends é **declarada** em `capabilities`, nunca
silenciada. **[Atualizado]** os dois backends já declaram `bodyRig: true`
(`apps/desktop/src/renderer/src/components/live2d/live2d-engine.ts` e
`apps/desktop/src/renderer/src/components/avatar/vrm-adapter.ts`) — o VRM ganhou coreografia de
corpo própria em `apps/desktop/src/renderer/src/components/avatar/vrm-body-rig.ts`
(`VRM_BODY_ACTIONS`, de `packages/sdk/src/contracts/types/avatar-types.ts`). A regra continua: se
uma capacidade divergir de novo entre os dois, ela é declarada, não silenciada.

> **Por quê:** os dois lados coincidiam por acidente. `VrmAvatar` era uma classe coesa e o lado
> Live2D eram quatro objetos — modelo, `Live2DAnimator`, `Live2DBodyRig`, `Live2DLipSyncController`
> — orquestrados à mão dentro de um `useEffect`. Nenhum tipo dizia o que os dois tinham em comum, e
> os componentes já tinham divergido: `modelPath` era opcional num e obrigatório no outro. Um método
> presente só num backend produz um avatar que reage num modelo e fica parado no outro, e isso não
> aparece em typecheck quando o chamador usa `?.` nem em teste, porque os dois arrastam WebGL.

**MUST NOT** — o contrato cobre comportamento de avatar, não o host de render. A
`PIXI.Application` de um e o `WebGLRenderer` do outro são genuinamente diferentes; forçá-los ao
mesmo molde seria camada que só repassa chamada.

## O sinal de lip-sync é agnóstico de backend

**MUST** — o lip-sync nasce do **áudio real** do TTS, não de fonemas do texto: RMS para abertura e
análise espectral para peso de vogal (`apps/desktop/src/renderer/src/lib/lip-sync-source.ts`). O
mesmo par de refs alimenta os dois motores por `setLipSyncSources`, e cada um decide como vira
parâmetro — `ParamMouthOpenY`/`ParamMouthForm` no Live2D, os presets `aa/ih/ou/ee/oh` no VRM.

## Princípio

> Uma pose que não aparece na tela é pior que pose nenhuma: ela consome código, teste e atenção, e
> não deixa rastro de que falhou.

O animador procedural sempre funciona, em qualquer skin. As ações são o que dá intenção — concordar,
negar, se envergonhar — e existem só onde a skin consegue expressá-las.

## Migração Live2D: pixi.js v6 → v8

`pixi-live2d-display@0.4.0` está abandonado, travado em `pixi.js ^6`. Spike de feasibility
(2026-09-26) mapeou a migração para `untitled-pixi-live2d-engine` + `pixi.js@8`: escopo de arquivo é
contido, mas um bloqueio de CSP (`script-src 'self'`, sem `unsafe-eval`) só se verifica rodando o
binário real — não migrado nesta rodada. Detalhe completo, mapeamento de API e próximo passo:
[12a-live2d-pixi-migration.md](./12a-live2d-pixi-migration.md).

## A Mika nunca fica travada na tela

**MUST** — a janela visível SEM foco anima em `eco` (15fps, `EFFECT_QUALITY_MAX_FPS` em
`apps/desktop/src/renderer/src/lib/effect-quality.ts`), nunca `paused`: respiração, piscada, saccades e micro-gestos continuam.
Uma companheira de desktop vive na periferia da sua atenção — congelada, ela parece travada.

**MUST** — micro-gestos de idle (4–12s) rodam com ou sem foco. O gate antigo usava `hasFocus`, que
o polling de mouse do main seta a cada 125ms: na prática o idle nunca disparava, e a tabela de
gestos era código morto.

**MUST** — `paused` só acontece com a janela oculta (`document.hidden`) ou com
`prefers-reduced-motion` sem fala. Fala vence foco sempre (boca mexendo com áudio saindo).

> **Por quê:** congelar por falta de foco era "ela fala e a boca não mexe" nos dois sentidos —
> áudio sem animação, e depois animação nenhuma enquanto você trabalhava em outro app.

## A Mika tem corpo, não só cabeça

**MUST** — o animador procedural escreve também **tronco, ombro, braço, mão e perna**, nos canais que
a skin expõe (`Live2DBodyRig`). Até esta camada existir, `ParamArm*`, `ParamHand*` e `ParamLeg*` não
eram escritos em lugar nenhum do repositório: a Mika conversava, respirava e piscava com os braços
soldados na pose de repouso do rig.

**MUST** — braço é cadeia com atraso: ombro → cotovelo → pulso, cada junta perseguindo a ANTERIOR,
com mola mais mole e ganho maior descendo o braço.

> **Por quê:** três molas perseguindo o mesmo alvo dão três atrasos independentes, e o braço lê como
> três peças soltas. Perseguindo a junta anterior, o resultado é follow-through — o que faz um braço
> parecer pendurado e não parafusado.

**MUST** — existe **troca de peso**: uma oscilação lenta (8–18 s, alternando de lado) que assimetriza
os dois braços, move quadril e perna e desencontra os ombros.

> **Por quê:** gente parada não fica parada. Avatar perfeitamente centrado e simétrico lê como
> manequim mesmo com a cabeça se mexendo.

**MUST** — a amplitude é **fração da distância entre o repouso e o extremo daquele lado**, lida do
rig em runtime, por lado. Faixa assimétrica (braço que descansa no máximo e só anda para baixo) sai
proporcional em vez de morrer no clamp.

**MUST NOT** — o ocioso não dirige canal de **variante de braço** (par chave+pose, como
`ParamArmLA`/`ParamArmLB` da haru). Ou trocaria o braço desenhado a cada respiração, ou escreveria
num canal invisível. Trocar de braço é intenção; intenção é ação com nome.

**MUST NOT** — o animador procedural **não escreve em destino de saída do `physics3.json`**. Vale
para ele exatamente como já valia para as motions geradas.

> **Por quê:** havia um bloco "Hair Physics" com seis molas escrevendo em `ParamHairFront`,
> `ParamHairSide`, `ParamHairBack`, `ParamScarf`, `ParamAccessory1` e `ParamBustY`. **Os seis são
> saída de física** nas skins que os têm (haru: HairFront/Side/Back, Scarf · Mao: HairFront/Back,
> Accessory1). A física roda depois e sobrescrevia os seis no mesmo
> quadro: seis integrações por quadro, a 60 fps, para um resultado que nunca chegou a um pixel. O
> balanço de verdade vem de mover cabeça e corpo — que é como o rig foi construído.

**MUST** — um canal tem **um dono**. `ParamBodyUpper` era escrito pela inclinação `nerdVal` e passou
a ser junta do corpo; a inclinação virou **postura**, e não um segundo escritor.

**MUST** — o canal que reage a movimento lê a velocidade da parte que de fato se move. O ahoge lê a
**cabeça**, não o torso: no ocioso é a cabeça que anda, e lendo o torso ele media variação de 0,08
numa faixa de 20 — calculado, escrito e invisível.

**MUST** — enquanto uma ação dirige alguma junta do corpo, as molas do corpo são relaxadas — e
**somente** nesse caso. Quase toda ação é de rosto: relaxar em todas devolveria os braços parados
durante cada reação, que é o estado que esta camada existe para acabar.

## Ações (grupo `Action`)

**MUST** — toda motion autoral é gerada por `pnpm live2d:gen` a partir de
`scripts/live2d/action-defs.ts`. Editar `.motion3.json` ou `.model3.json` à mão é proibido: a
verificação é `tsx scripts/live2d/live2d-gen-motions.ts --check` (edição manual diverge da
geração). **[Não roda em `pnpm gates`]** — não existe alias `check:live2d-motions` em
`package.json` nem entrada correspondente na lista de `gates`; hoje é passo manual, não gate de CI.

**MUST** — motion é tocada por **nome**, resolvido contra o `model3.json` da skin carregada.

> **Por quê:** o animador tocava por índice — `model.motion('', 17, 2)` — com uma tabela de 0 a 26.
> Esse grupo sem nome só existe na haru. Na outra skin embarcada (Mao) **toda** reação caía no
> `catch`, sem pose e sem erro visível: a skin rodou meses com a expressividade desligada e o único
> sinal era um `warn`. Índice não é conferível por gate; nome é.

**MUST** — o gerador só escreve o grupo `Action`. `Idle`, `TapBody` e demais grupos do fornecedor
são intocáveis.

**MUST NOT** — animar parâmetro que é destino de saída do `physics3.json` (marcado `★fis` em
`pnpm live2d:analyze`). A física sobrescreve no frame seguinte; mover cabeça e corpo faz o cabelo
balançar sozinho.

**MUST** — toda curva sai da pose de repouso e volta para ela. Ação é one-shot (`Loop: false`), com
fade de 0,2 a 0,5 s.

> **Por quê:** ação que termina fora do repouso deixa resíduo. O resíduo acumula a cada disparo até a
> Mika ficar com uma careta permanente que nenhum código explica.

**MUST** — valores de keyframe são **deslocamentos a partir do repouso**, nunca absolutos, e ficam
dentro da faixa que as motions originais da skin usam.

> **Por quê:** o repouso muda por skin. `ParamMouthForm` descansa em **1** na haru, não
> em 0. Curva absoluta escrita para uma skin vira careta na outra. A faixa é medida das motions do
> fornecedor, não chutada.

**MUST** — curva de **braço** usa `unit: 'normalized'`: o deslocamento é fração da faixa, resolvida
por skin. Curva normalizada que sai sem amplitude num modelo é **erro de geração**, não aviso.

> **Por quê:** a escala de braço não é compartilhada — o mesmo "levantar o braço" é uma fração grande
> da faixa no `ParamArmR*` da Mao e +5 no `ParamArmRB` da haru. Um valor absoluto
> escrito para um vira, no outro, uma curva inteira grudada no teto do clamp. Isso **gera arquivo,
> passa no gate** (a máscara bate, a base entra e sai intacta) e na tela é um braço parado.

**MUST NOT** — escrever curva num parâmetro cujo **repouso medido diverge do repouso do rig**.
`ParamArmRA` da haru descansa em 1 nas motions do fornecedor e em **0** no modelo carregado; como os
keyframes são deslocamentos a partir da base medida, a curva saltaria no primeiro quadro e devolveria
1 no fim, deixando resíduo permanente.

**MUST** — parâmetro sem faixa medida (nenhuma motion original o usa) só entra via
`UNOBSERVED_ALLOWLIST`, com o motivo escrito.

> **Por quê:** é exatamente onde o rig deforma sem ninguém perceber. Lista literal, revisada uma vez
> por gente; parâmetro novo nessa situação quebra a geração até alguém justificar.

**MAY** — uma skin não ter uma ação. A Mao não tem `ParamMouthForm`, logo não tem `pout`, e o animador
cai na candidata seguinte. Ação ausente do catálogo é informação; ação presente que não se move, não.

## Máscara de parâmetros

**MUST** — enquanto uma ação toca, o animador procedural **não escreve** nos parâmetros que ela
dirige, até o fim do fade-out.

> **Por quê:** `applyToModel` roda depois do motion manager e reescreve cabeça, olhos, sobrancelhas e
> boca todo frame. Sem ceder os canais, a motion é calculada e apagada no mesmo frame: nenhuma pose,
> nenhum erro, nenhum log. Adicionar os arquivos de motion sem a máscara teria trocado uma falha
> silenciosa por outra.

**MUST** — a máscara é exatamente o conjunto de parâmetros das curvas da motion. Faltando um, a curva
some; sobrando um, o avatar congela num eixo que ninguém está dirigindo. O gate compara os dois.

**MUST NOT** — ceder os parâmetros de boca enquanto a Mika fala. Voz ganha de pose.

> **Por quê:** uma ação de 4 s segurando a boca devolveria o "ela fala e a boca não mexe" que o
> lip-sync levou meses para perder.

## Captura reprodutível

**SHOULD** — screenshot de avatar no E2E usa `window.__mikaLive2D.freeze(acao, segundos)`.

> **Por quê:** a pose dependia de `Math.random()` (piscada, saccade, ocioso) e de ruído Perlin com
> semente aleatória. O mesmo teste fotografava uma pose diferente a cada execução, então os
> screenshots do avatar só provavam que o canvas não estava em branco. `freeze` semeia o estado,
> desliga o aleatório e calcula a pose em passos fixos — sem `sleep`, sem esperar frame.

**MUST NOT** — usar `sleep` para "esperar a pose chegar" na camada procedural. O estado em N segundos
é calculado.

**MUST NOT** — conferir **ação** com `freeze()`. Ele não aplica motion nenhuma.

> **Por quê:** `Live2DModel.update(dt)` só ACUMULA tempo; quem aplica motions, física e pose é
> `internalModel.update()`, e ela roda dentro do `_render` do Pixi — que está parado durante o
> congelamento. Medido com `readParams`: com e sem ação, **todos** os parâmetros do modelo saem
> idênticos do laço. Um teste de ação escrito sobre `freeze` fica verde sem nunca ter havido pose, e
> foi exatamente assim que três ações de braço passaram na conferência com a Mika de braços cruzados.

**MUST** — conferência de ação roda com o **ticker ligado** e afirma sobre **parâmetro**, não sobre
pixel.

> **Por quê:** comparar screenshots aceita qualquer diferença, e respiração e piscada garantem
> diferença mesmo com o braço soldado. `ParamArmRB > 0.5` diz o que a foto não distingue.

**MUST** — `freeze()` zera a pose antes de simular (molas, piscada, temporizadores) e desenha um
quadro ao terminar.

> **Por quê:** as molas guardavam posição e velocidade do que estava na tela um instante antes — as
> da cabeça são rígidas e convergiam dentro da captura, as do corpo não. E o Pixi desenha só dentro
> do ticker: sem o quadro manual, a pose era calculada e nunca pintada.

## Sonda de canal (`@probe`)

**SHOULD** — para responder _este canal move alguma coisa NESTA skin?_, usar
`window.__mikaLive2D.probe()` (força parâmetros crus por cima de tudo) e `readParams()` (lê o valor
corrente). A sonda vive em `apps/desktop/e2e/specs/live2d-arm-probe.spec.ts`, fora da execução padrão
(`MIKA_LIVE2D_PROBE=1`).

**MUST** — homologação de skin alternativa roda com `MIKA_E2E_SKIN=<Pasta>` (o fixture semeia
`character.modelPath`). Sem isso a suíte só vê a skin padrão, e as outras quatro nunca são olhadas.

**MUST** — toda varredura de sonda inclui um **controle positivo** (um canal que comprovadamente move
o desenho).

> **Por quê:** "este parâmetro não faz nada nesta skin" e "a sonda não está chegando ao modelo"
> produzem exatamente a mesma imagem, e são conclusões opostas. Sem o controle, a segunda é lida como
> a primeira — foi o que quase enterrou a camada de braço da skin padrão.

## Comandos

| Comando                                            | Para quê                                                                                 |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `pnpm live2d:analyze [Skin]`                       | Parâmetros, faixa segura, saídas de física e pose de repouso                             |
| `pnpm live2d:gen`                                  | Gera as motions, registra no `model3.json`, atualiza o catálogo do renderer              |
| `tsx scripts/live2d/live2d-gen-motions.ts --check` | Sem divergência com as definições + as 7 conferências (manual, não roda em `pnpm gates`) |

## Definition of Done de uma ação nova

1. `tsx scripts/live2d/live2d-gen-motions.ts --check` sai 0
2. A ação foi vista em cada skin que a recebeu, **com o app rodando** — pose parada na base significa
   que não tocou, e `freeze()` não distingue os dois casos (ver acima)
3. O diff do `model3.json` é só o grupo `Action`
4. Skin que ficou sem a ação está explicada no relatório do gerador (`pulada 'x': ...`)
