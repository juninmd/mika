# 09 — Configuração e UI

## Princípio

O usuário configura a Mika inteira sem abrir arquivo, sem saber o nome de um provedor e sem
conseguir se colocar num estado quebrado sem ser avisado.

## Painéis são escritos à mão

**MUST**: cada painel de settings é um componente React manual.

**MUST NOT** gerar UI genérica a partir de schema.

O schema continua sendo contrato de validação e defaults no save — não fonte de layout.

## Padrão Ouro de UI/UX (Mika > Personagem & Mika > Usuário)

As telas de `features/character` e `features/user` definem o padrão estético e de UX do app:

1. **Hero Snapshot / Identidade no Topo:**
   - Cartão de cabeçalho (`GlassCard span="full"`) com avatar estilizado (`rounded-2xl border border-info-rim bg-info-soft text-info`), badge com tracking expandido (`tracking-[0.18em]`), título proeminente (`font-black tracking-tight text-text-main`) e pílulas de fatos rápidos (`bg-surface-raise`, labels uppercase `tracking-[0.14em]`).
   - Indicador de status claro (`StatusPill` ou barra de progresso / completude com percentual).

2. **Composição em Bento Grid & Glass:**
   - Agrupamentos em `GlassSection` com ícone temático e descrição concisa.
   - Grid em `BentoGrid` com `GlassCard`, seções internas com cabeçalhos trackeados (`font-black uppercase tracking-[0.12em] text-text-main`) e divisores visuais (`SectionDivider`).

3. **Controles Formais Exclusivos do SDK:**
   - Exclusivamente `@mika/sdk-ui/settings` (`AuroraField`, `GlassSelect`, `GlassSlider`, `GlassToggle`, `StatusPill`, `StatTile`).
   - Cores semânticas tokenizadas (`text-text-main`, `text-text-muted`, `border-border-subtle`, `bg-surface-raise`). Zero literais de cor.
   - **MUST**: gate `pnpm check:ui-controls` (`scripts/checks/ui-controls.mts`) pega controle de
     formulário cru (`select`/`input`/`checkbox`/`range`/`switch`) fora do design system.

## Toda aba precisa ser alcançável

**MUST**: a navegação mostra tanto abas vindas de feature quanto abas built-in.

> **Por quê:** o filtro do rail escondia qualquer seção cujas abas não estivessem na lista de
> _declarations_ — que só contém abas de feature. Isso apagou da navegação **Credenciais, Skin,
> Ferramentas, Tools e a seção Sistema inteira** (overview, logs, updates). Os painéis existiam,
> compilavam e eram inalcançáveis.

**MUST**: existe teste que falha se um painel registrado ficar sem rota de navegação.

**MUST**: a navegação tem duas camadas — rail vertical de seções e **abas horizontais** dentro da
seção ativa. Item de rail que carrega **uma aba só** é regressão: abre e fecha o mesmo painel.

> **Por quê:** o rail chegou a 14 itens, três deles (Humor, Skin, Cultura) apontando para um painel
> único, e os submenus eram uma segunda coluna vertical ao lado da primeira. Agrupar por domínio —
> Mika (personagem, skin, humor, usuário), Cognição (LLM, memória, visão), Voz, Interação,
> Ferramentas, Conta, Sistema — levou o rail a sete itens e as abas da seção passaram a ser
> horizontais. Gate `check:settings-nav` recusa seção de aba única e aba declarada sem rota.

## Cobertura de campos nos dois sentidos

Dois gates, porque um só deixa passar metade dos erros:

1. **schema → lista** — todo campo do schema está declarado como coberto por algum painel.
2. **lista → JSX** — todo campo declarado aparece de fato em algum painel.

**MUST**: ambos rodam em CI.

> **Por quê:** só o primeiro existia. Dava para declarar um campo como coberto sem renderizar
> nada, e a auditoria ficava verde. Doze campos de schema não tinham UI nenhuma.

Campos legitimamente sem UI (dicionários geridos por ferramenta, caches internos) ficam numa lista
de isenção **explícita e justificada**, e há teste que falha quando a isenção fica obsoleta.

## Cor é contrato, não decoração

**MUST**: toda cor da UI sai de um token do tema. Literal de paleta (`text-slate-500`,
`bg-emerald-500/10`, `bg-white/[0.04]`) é erro de build — gate `pnpm check:ui-style`
(`scripts/checks/ui-style.mts`; renomeado de `check:ui-colors`, hoje também pega fonte abaixo do
piso de legibilidade e `bg-accent`+`text-accent-text` — ver "Acessibilidade e idioma").

> **Por quê:** havia ~1.200 literais. Eles não acompanham tema nenhum, e é nos temas claros que o
> defeito aparece: `text-emerald-400` sobre fundo branco mede 1,9:1, e uma "superfície de realce"
> de branco a 4% sobre branco simplesmente não existe. Nenhum teste, typecheck ou build pega isso.

**MUST**: existe **um** sistema de acento. `--accent` e derivados saem da tripla `--accent-h/s/l`;
reacentuar uma subárvore é `accentVars(hue)`, que reescreve tripla e derivados juntos.

> **Por quê:** havia três concorrendo — acento do tema, hue por seção de navegação e
> `accentHue={n}` cravado em 25 painéis. Como propriedade customizada é substituída no computed
> value, a hue de subárvore não movia `--accent`: o mesmo cartão pintava a barra da esquerda de
> roxo e o gradiente que sai dela de ciano.

**MUST**: verde, âmbar e vermelho significam **estado** (ok / atenção / erro) e nada mais.
Seleção, ação primária e identidade de seção usam o acento.

> **Por quê:** a grade de provedores dava uma cor por provedor — Ollama em 155 (o verde de
> sucesso), OpenRouter em 38 (o âmbar de atenção). Escolher um provedor parecia um diagnóstico.

**MUST**: a polaridade do tema (`Theme.scheme`) chega ao CSS, e a rampa semântica inverte com ela.

## Uma primitiva por trabalho

**MUST**: selo é `StatusPill`, número é `StatTile`, lista vazia é `EmptyState`, cartão é
`GlassCard`/`GlassSection`. Componente novo com o mesmo trabalho é regressão.

> **Por quê:** chegamos a cinco superfícies de cartão e cinco selos, cada um com raio, tracking e
> paleta próprios — às vezes dois deles na mesma tela.

**MUST NOT**: a mesma informação em dois lugares da mesma janela.

> **Por quê:** o contador de ferramentas aparecia na barra de topo, no tile da visão geral e no
> tile do deck de módulos; o estado do boot, em três. E a grade de temas era renderizada por dois
> componentes diferentes, cada um com sua cópia da lógica de salvar.

**MUST**: gate `pnpm check:dead-ui` (`scripts/checks/dead-ui.mts`) pega componente React sem
importador de produção — compila e passa em review "parecendo" ligado, mas nada monta.

## Lista longa não esconde linha

**MUST**: lista grande usa `LazyList` (`content-visibility`), não corte de renderização.

**MUST NOT**: `slice(0, N)` silencioso na camada de exibição. Se o tamanho importa, mostre-o.

> **Por quê:** o terminal de logs guardava 500 e desenhava 250 — com o rodapé anunciando o total.
> O histórico de chat cortava em 300 e, **com busca ativa, cortava antes de filtrar**: o resultado
> na mensagem 350 não aparecia e a tela mandava "refinar a busca", conselho impossível de seguir.

## Seleção de modelo

**MUST**: por seletor com catálogo real, busca e badges de capacidade. Ver [02](02-providers.md).

**MUST NOT** por campo de texto livre.

**MUST NOT** por dropdown com lista embutida no código.

## Credenciais centralizadas

**MUST**: uma aba única com todas as chaves, tokens e URLs, agrupadas por domínio.

**MUST**: campos sensíveis são do tipo senha e nunca aparecem em log.

**SHOULD**: cada provedor tem ação de testar, e o teste é barato (listar modelos serve — se lista,
a credencial vale).

## Estado do sistema é visível

**MUST**: provedor desativado aparece em qualquer aba, com motivo e ação. Ver [03](03-resilience.md).

**MUST**: boot degradado aparece na UI, não só no health endpoint.

**MUST**: o usuário consegue ver o que a Mika aprendeu sobre ele e sobre as pessoas, e apagar.

## Diagramas

Alguns painéis explicam o fluxo do sistema com diagrama.

**MUST**: o diagrama renderizado é verificado — que ele **tem rótulos**, não só caixas e setas.

> **Por quê:** o "Mapa de decisão da resposta" renderizava 6 nós, 14 arestas e **zero texto**. A
> sanitização de HTML remove `foreignObject`, que é onde a lib põe os rótulos por padrão. Passava
> em qualquer teste que só checasse "o SVG existe".

**MUST**: rótulos de diagrama sobrevivem à sanitização (não dependem de HTML embutido no SVG).

## Tela cheia da janela do avatar

**MUST**: a tela cheia é uma preferência (`app.fullscreen`), não um estado só da janela. O
interruptor de Configurações (`WindowModeCard`, aba Resumo), o **F11** com a janela em foco e o item
"Alternar tela cheia" da paleta chamam o mesmo canal (`window-toggle-fullscreen`), que grava a chave;
um observador do `ConfigStore` (`src-tauri/src/fullscreen.rs`) aplica `set_fullscreen`. Reaplicada
quando o avatar é revelado no boot; o Tauri devolve tamanho e posição de antes ao sair.

**MUST NOT**: F11 virar atalho global do SO (`shortcuts.rs`) — engoliria a tela cheia do jogo ou do
navegador do usuário. Vale só com a janela do avatar em foco (`WINDOW_SHORTCUTS` em `app-shortcuts.ts`).

**MUST**: o estado mostrado vem do backend (`window-get-fullscreen` + evento `window:fullscreen`),
nunca do último clique: o SO pode tirar a janela da tela cheia por fora (gesto no macOS), e
`on_resized` desliga a preferência para os dois lados não divergirem.

## Onboarding

**MUST**: o primeiro uso pede o mínimo: um provedor, uma credencial, um modelo. Persona e
relacionamento têm default utilizável.

**MUST**: pular a configuração é permitido, e o app diz claramente o que fica indisponível.

## Falha isolada

**MUST**: falha de um subsistema visual derruba só ele, nunca a janela.

> **Por quê:** sem WebGL o Pixi lança, e a exceção subia até a barreira raiz: máquina sem GPU
> (VM, área de trabalho remota, driver quebrado) perdia dock, HUD, chat e o botão de Configurações
> — tudo trocado por uma tela de erro fatal — quando o que faltava era só o desenho da personagem.

## Tipografia é empacotada

**MUST**: as fontes vêm do bundle. A CSP não permite host de fonte externo.

> **Por quê:** as três famílias vinham de `fonts.googleapis.com`. Num app que "roda local no PC do
> usuário", sem rede a folha nunca chegava e `--font-display: "Space Grotesk", serif` caía no
> `serif` do sistema: todo título do wizard e das Configurações em Times New Roman. E o app pingava
> o Google a cada janela aberta, com uma etapa de privacidade no próprio onboarding.

## Acessibilidade e idioma

**MUST**: a UI é navegável por teclado.

**MUST**: o foco visível é um só, derivado do acento, e vale para todo controle.

**MUST NOT**: repetir em texto o estado que o próprio controle já mostra.

> **Por quê:** cada `GlassToggle` trazia um selo "ATIVO"/"INATIVO" a 2cm do interruptor, em toda
> linha dos 17 painéis. Não acrescenta leitura — para leitor de tela o estado vem de
> `role="switch"` + `aria-checked`, e o selo era um `<span>` mudo.

**MUST**: textos de interface vivem em arquivo de tradução, não espalhados. (A **fala** da Mika não
é tradução — é gerada no idioma configurado. Ver [07](07-persona-and-relationship.md).)

**MUST**: gate `pnpm check:ui-i18n` (`scripts/checks/ui-i18n.mts`) pega texto de interface escrito
direto no código em vez de `t()`; `pnpm check:i18n-orphans` (`scripts/checks/i18n-orphans.mts`) pega
chave do dicionário que nenhum código cita.

> **Por quê:** a varredura por screenshot em en-US/ja-JP achou ~120 strings fixas espalhadas por 30
> arquivos, e cada tela enxugada deixava chaves órfãs para trás (133 numa só varredura) — cada uma
> custa 10 traduções mantidas para nada.
>
> O gate cobre também `packages/sdk-ui/src` e, além de texto entre tags e props, braço de ternário
> (`ok ? 'Salvo' : …`), `setError(…)`, `throw new Error('…')`, `addNotification(…)`, o fallback de
> `?? '…'`/`toDisplayError(e, '…')` e `alt=`. Texto com contagem usa `t(key, { count })`: a chave base é a
> forma `other`, e `<chave>.one/.few/.many` existem só nos idiomas cuja regra de `Intl.PluralRules` as usa
> (`i18n.plural.test.ts`). O idioma do SO só vale para perfil novo, e quem o grava é o Rust no primeiro boot
> (`config/locale.rs::seed_first_run`); o renderer só lê `settings-ui-prefs.locale`.
