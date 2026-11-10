/**
 * check:sidecar-attrition — a catraca da migração Electron → Tauri **sem sidecar Node**.
 *
 * O backend (~52k LOC de TypeScript) não é reescrito de uma vez: `bridge.rs` pergunta ao registro
 * de `native/mod.rs` (alimentado pelos módulos de `modules/*.rs`) se o canal tem dono em Rust e, se
 * não tiver, encaminha ao sidecar. Cada módulo portado tira seus canais daquela seta. Quando ela
 * esvazia, o sidecar é apagado.
 *
 * Esse desenho só é honesto com uma catraca, e por dois motivos distintos:
 *
 *  1. **Progresso vira número, não sensação.** "Migrei bastante" não é verificável; "restam 31 de
 *     44 canais" é. O baseline abaixo é o placar, e ele só pode cair.
 *  2. **Impede o sidecar de voltar a crescer.** O caminho fácil, no meio de uma migração longa, é
 *     acrescentar UM canal novo no lado Node porque lá já tem tudo pronto. Isso alonga a migração
 *     em silêncio: nada quebra, nenhum teste fica vermelho, e o alvo se afasta. Canal novo agora
 *     nasce em Rust, ou o gate reprova.
 *
 * Ele não julga qualidade de porte — quem faz isso são os testes de cada módulo. Ele conta canais.
 */

import { readdirSync, readFileSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'

const ROOT = resolve(import.meta.dirname, '../..')
const CONTRACTS = join(ROOT, 'packages/sdk/src/contracts')
const CHANNELS = join(CONTRACTS, 'desktop-ipc-channels.ts')
const DESKTOP_IPC = join(CONTRACTS, 'desktop-ipc.ts')
const NATIVE = join(ROOT, 'apps/desktop/src-tauri/src/native/mod.rs')
const MODULES = join(ROOT, 'apps/desktop/src-tauri/src/modules')
const BRIDGE = join(ROOT, 'apps/desktop/src-tauri/src/bridge.rs')

/**
 * Quantos canais ainda descem para o sidecar. **Só pode cair.**
 *
 * 43 = 49 canais únicos do contrato (32 invoke + 4 send + 8 shell + 5 rotas locais) menos os 6 de
 * janela que o Rust já possuía desde o início. Os dois de `auto-launch` ficam na conta: apesar de
 * serem `DESKTOP_SHELL_CHANNELS`, é o sidecar que os serve hoje.
 *
 * O ponto de partida foi 43, medido em 2026-09-21 com o registro nativo vazio — o estado exato de
 * antes da migração começar. A primeira onda (settings, telemetry, update, desktop, system, health,
 * llm) tirou 26 canais de lá; a segunda tirou mais 3 (voz e wizard).
 *
 * Os que sobram NÃO são os mais fáceis que ficaram para depois — são os que dependem de estado vivo
 * do processo Node, e cada frente documentou o porquê no cabeçalho do próprio módulo. A terceira
 * onda portou `brain` e `inference`, e a quarta ligou `mika:chat-send` ao laço de turno em Rust
 * (14 → 13) e `mika:voice-transcribe` ao Whisper, depois que `prepare_whisper` passou a baixar o
 * `tokenizer.json` que faltava (13 → 12).
 *
 * A quinta onda levou `chat-history`, `chat-clear` e `chat-cancel` (12 → 9), com o histórico em
 * `modules/chat/history.rs` e um registro de `TurnAbortHandle` por janela.
 *
 * O que trava cada um dos 9, hoje: `tools`/`plugin-*` (os registros só populam depois que os
 * módulos EXECUTAM — não é falta de motor JS: os 12 plugins são todos do bundle, nenhum de
 * terceiro) e `vad-audio`. Este último NÃO espera mais o barramento de eventos, que já existe em
 * `src/events.rs`: espera `barge-in-recorder` e `interruption-detector` saírem do sidecar, porque
 * enquanto eles escutarem o `EventBusService` do Node nenhum barramento Rust os alcança.
 *
 * A regressão que a quinta onda abriu em `chat-history`/`chat-clear` — histórico só em memória,
 * perdido no restart — foi fechada na sequência: `chat/chat_db.rs` persiste em rusqlite no MESMO
 * arquivo que o TS já usa (`<app_data>/plugins/mika.chat/chat.db`), com as mesmas colunas e o
 * mesmo índice único de `messages-table.ts`, e adota no primeiro boot a sessão mais recente que o
 * TS tiver deixado. Fica o registro do que ela ensinou: canal reivindicado não é canal migrado.
 * O placar mede quem SERVE, não quem serve bem, e essa diferença só aparece lendo o handler do
 * outro lado.
 *
 * Isto só desce. Quem migrar um módulo baixa este número no mesmo commit: enquanto ele não desce, o
 * gate aceita de volta a regressão que o módulo acabou de resolver.
 *
 * 9 -> 8 em 2026-09-21: `voice:vad-audio` reivindicado por `modules/voice/vad_audio.rs` — o último
 * canal de voz. Barge-in publica `ForceStopTts` de verdade (já consumido por
 * `spawn_force_stop_listener`, ligado desde a sétima onda) e aborta o turno de LLM em curso via
 * `chat::turns::abort_turn`.
 *
 * 1 -> 0 em 2026-09-22: `mika:plugin-test` reivindicado por `modules/settings/mod.rs`. **Nenhum
 * canal desce mais para o sidecar.** Destravou quando as 8 implementações genéricas de
 * `testConnection` passaram a existir em Rust, cada uma registrada a partir de chamador de produção
 * real, e quando os provedores que o ramo de teste consulta deixaram de ser ficção: TTS remoto (4),
 * STT em nuvem (2) e LLM multi-provedor (google/openrouter/openaiCompatible em loopback).
 *
 * O que AINDA degrada nomeado neste canal, sem esconder: `piper`/`kokoro` (TTS local ONNX),
 * embeddings (nenhum provedor portado) e `openaiCompatible` com host remoto (mesma guarda de SSRF
 * fail-closed do catálogo). São lacunas da FEATURE, não do canal — o teste continua dizendo a
 * verdade sobre cada uma.
 *
 * 2 -> 1 em 2026-09-22: `mika:plugin-rpc` reivindicado por `modules/plugins/native.rs`. O bloqueio
 * não era "é código, não dado para portar" — era o DENOMINADOR: com o registro vazio, reivindicar
 * devolveria `handler-missing` para os 25 métodos vivos. Os quatro donos foram portados na mesma
 * rodada (`features/person_catalog`, `features/mood`, `gamer`, `memory/social`) e 24 dos 25
 * despacham de verdade. O que sobra é `person_catalog_image`, sem handler de propósito: pede fetch
 * HTTPS revalidado contra SSRF a cada hop mais cache com teto de MIME/tamanho, e o renderer
 * (`use-person-image.ts`) já lê a ausência como "sem foto".
 *
 * 6 -> 2 em 2026-09-22: os QUATRO canais de tool (`mika:tools-list`, `mika:tools-list-by-plugin`,
 * `mika:tool-execute`, `tool-approval-response`) reivindicados de uma vez por
 * `modules/tools/native.rs`. De uma vez porque `bridge.rs` roteia por canal inteiro: reivindicar a
 * resposta de aprovação sem o pedido penduraria toda aprovação até o backstop de timeout — o mapa
 * de pendentes ficaria no processo errado.
 *
 * O que mudou não foi a tabela de degradação esvaziar; foi o argumento cair. Ele era "tool que
 * existe e recusa é pior para o modelo que tool que não existe, logo não reivindicar", e tratava o
 * sidecar como alternativa disponível. Com o objetivo sendo APAGÁ-LO, cada canal aqui dentro
 * mantinha o processo Node inteiro vivo por causa de um punhado de ações — que foram atacadas em
 * frentes paralelas em vez de servirem de motivo para adiar. O que restar delas degrada com
 * `errorCode` nomeado, igual ao que o TypeScript faz quando a peça de baixo não responde.
 *
 * 8 -> 7 em 2026-09-21: `mika:plugin-ui-catalog` reivindicado por `modules/settings/plugin_ui.rs`.
 * `mika:settings-declarations` (o outro canal que dependia do mesmo `FeatureRegistry`) continua NO
 * sidecar: medido ao vivo, o registro nativo hoje publica 13 das 14 abas de settings que o TS
 * declara — falta só `voice` (sem `modules/voice/feature.rs` ainda). `plugin-ui-catalog` não tem
 * essa dependência: lê `views`/`settingsPanels`, não `settings`, e os dois únicos
 * `FeatureDefinition` que declaram algo ali (`gamer-companion`, `person-catalog`) já registram as
 * próprias views/painéis. Ver o cabeçalho de `modules/settings/mod.rs` para a medição completa.
 */
const BASELINE = 0

/**
 * O ponto de partida real, medido em 2026-09-21 com o registro nativo vazio. **Constante, nunca
 * desce** — é justamente o contrário do `BASELINE`.
 *
 * Existe porque o placar estava mentindo. `migrated` era `BASELINE - stillOnSidecar`, ou seja o
 * progresso desde o ÚLTIMO reset da catraca: como o baseline desce a cada onda, logo depois de
 * baixá-lo a conta dá zero, e o gate anunciava "0 migrados, 0%" com 34 dos 43 canais já em Rust.
 * Um placar que zera toda vez que alguém avança é pior que nenhum — ele faz o trabalho feito
 * parecer não ter acontecido, e é exatamente o número que alguém olha para decidir o que fazer.
 */
const STARTING_POINT = 43

type Violation = { file: string; message: string }
const violations: Violation[] = []
const rel = (file: string) => relative(ROOT, file).replaceAll('\\', '/')

const channelsSource = readFileSync(CHANNELS, 'utf8')
const desktopIpcSource = readFileSync(DESKTOP_IPC, 'utf8')
const nativeSource = readFileSync(NATIVE, 'utf8')
const bridgeSource = readFileSync(BRIDGE, 'utf8')

/** Extrai os literais de um bloco `export const NOME = { ... } as const`. */
function channelsOf(source: string, constName: string): string[] {
  const block = new RegExp(`export const ${constName} = \\{([\\s\\S]*?)\\} as const`).exec(source)?.[1]
  if (block === undefined) return []
  return [...block.matchAll(/:\s*'([^']+)'/g)].map((match) => match[1])
}

/*
 * Piso de alcance. Um `channelsOf` que passa a devolver vazio (renomearam a const, trocaram aspas,
 * partiram o arquivo) faria a conta dar ZERO e o gate anunciar migração completa sem nada migrado.
 * É a classe de falso verde que já mordeu aqui antes — gate que lê fonte tem de provar que leu.
 */
const invokeChannels = channelsOf(channelsSource, 'DESKTOP_INVOKE_CHANNELS')
const sendChannels = channelsOf(channelsSource, 'DESKTOP_SEND_CHANNELS')
const shellChannels = channelsOf(channelsSource, 'DESKTOP_SHELL_CHANNELS')

if (invokeChannels.length < 20 || sendChannels.length < 3 || shellChannels.length < 6) {
  violations.push({
    file: rel(CHANNELS),
    message:
      `leu de menos: ${invokeChannels.length} invoke, ${sendChannels.length} send, ${shellChannels.length} shell. ` +
      'O gate não consegue contar o que resta no sidecar e passaria verde à toa',
  })
}

/*
 * Rotas locais (`system:getRecentLogs`, `llm:getProviderHealth`, …) moram em desktop-ipc.ts, fora
 * das constantes de canal, e também são servidas pelo sidecar. Esquecê-las subestimaria o que falta.
 */
const localChannels = [...desktopIpcSource.matchAll(/channel:\s*'([a-z][\w-]*:[\w:.-]+)'/g)].map((m) => m[1])

/* Os 6 canais de janela nunca estiveram no sidecar — são do Rust desde o início. Os dois de
 * auto-launch são a exceção declarada em `bridge.rs`, e contam como dívida. */
const rustOwnedShell = [
  ...(/pub const SHELL_CHANNELS: \[&str; \d+\] = \[([\s\S]*?)\];/.exec(bridgeSource)?.[1] ?? '').matchAll(/"([^"]+)"/g),
].map((match) => match[1])

if (rustOwnedShell.length < 6) {
  violations.push({
    file: rel(BRIDGE),
    message: `leu ${rustOwnedShell.length} canais em SHELL_CHANNELS; sem eles a conta do que falta fica inflada`,
  })
}

/*
 * O que os módulos nativos reivindicam: os literais dentro dos `channels()` do registro.
 *
 * O bloco `#[cfg(test)]` é CORTADO antes da varredura. Sem isso, um fixture de teste que declarasse
 * canais literais contaria como migrado, e o placar subiria sem uma linha de produção ter mudado —
 * a sonda negativa deste gate foi exatamente quem mostrou o furo.
 */
/**
 * Todos os `.rs` sob `modules/`, RECURSIVAMENTE.
 *
 * Recursivo porque módulo grande vira diretório (a regra de 300 linhas por arquivo obriga), e uma
 * leitura só da raiz devolveria zero canal para ele — o gate voltaria a contar como sidecar o que
 * já migrou, verde. É a mesma cegueira que este arquivo já teve quando os módulos saíram de
 * `native/mod.rs` para `modules/*.rs`.
 */
function rustSourcesUnder(dir: string): { path: string; source: string }[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return rustSourcesUnder(path)
    return entry.name.endsWith('.rs') ? [{ path, source: readFileSync(path, 'utf8') }] : []
  })
}

const moduleFiles = rustSourcesUnder(MODULES)

/**
 * Remove os itens `#[cfg(test)]`, casando as chaves do item em vez de cortar o arquivo no primeiro
 * que aparecer.
 *
 * Cortar no primeiro cega o gate: um item só-de-teste declarado ANTES do código de produção
 * transforma todo o resto do arquivo em "teste" aos olhos daqui, o módulo devolve zero canal, e o
 * placar volta a contar como sidecar o que já migrou. Aconteceu ao vivo em `chat.rs`, com o canal
 * já reivindicado e o gate verde em "0 migrados". Os pisos lá embaixo não pegam: são globais, e um
 * arquivo cego por vez se esconde na soma dos outros.
 *
 * Exigir que todo `#[cfg(test)]` abrisse um `mod tests` resolveria a cegueira, mas o gate passaria
 * a ditar estilo: `vad.rs` tem um `#[cfg(test)] fn session_ptr` dentro do `impl`, que é idiomático
 * e é o que torna testável a sessão ONNX compartilhada. Gate conta canal; não legisla sobre onde
 * mora o helper de teste.
 *
 * Nem todo item tem chave: `mod`/`fn`/`impl` fecham em `}`, mas `const`/`use`/`type` fecham em `;`
 * — e `const` é justamente a forma que cegou `chat.rs`. Procurar chave direto faria o scanner pular
 * o `const` e engolir a função de PRODUÇÃO seguinte, trocando uma cegueira por outra.
 *
 * O scanner pula literal de string e comentário, senão uma chave ou um ponto-e-vírgula dentro de
 * `"…{…"` (ou num comentário sobre o próprio corte) desbalanceia a contagem e o gate volta a errar
 * em silêncio.
 */
const TEST_ATTRIBUTE = '#[cfg(test)]'

/** Índice da aspa que fecha o literal aberto em `quote`, pulando `\"`. */
function endOfString(source: string, quote: number): number {
  for (let cursor = quote + 1; cursor < source.length; cursor++) {
    if (source[cursor] === '\\') cursor++
    else if (source[cursor] === '"') return cursor
  }
  return source.length
}

/** Índice da quebra de linha que fecha o comentário aberto em `slash`. */
function endOfLineComment(source: string, slash: number): number {
  const line = source.indexOf('\n', slash)
  return line === -1 ? source.length : line
}

/** Índice do `}` ou `;` que fecha o item começado em `from`. */
function endOfItem(source: string, from: number): number {
  let depth = 0

  for (let cursor = from; cursor < source.length; cursor++) {
    const char = source[cursor]

    if (char === '"') cursor = endOfString(source, cursor)
    else if (char === '/' && source[cursor + 1] === '/') cursor = endOfLineComment(source, cursor)
    else if (char === '{') depth++
    else if (char === '}' && --depth === 0) return cursor
    // `;` fora de qualquer bloco encerra um item sem corpo (`const`, `use`, `type`).
    else if (char === ';' && depth === 0) return cursor
  }

  return source.length
}

function stripTestItems(source: string): string {
  let out = ''
  let index = 0

  while (index < source.length) {
    const start = source.indexOf(TEST_ATTRIBUTE, index)
    if (start === -1) return out + source.slice(index)

    out += source.slice(index, start)
    index = endOfItem(source, start + TEST_ATTRIBUTE.length) + 1
  }

  return out
}

const claimSources = [nativeSource, ...moduleFiles.map(({ source }) => source)].map(stripTestItems)

/**
 * Os canais que um `channels()` devolve.
 *
 * O corpo raramente traz os literais: o estilo dos módulos é `fn channels(…) { CHANNELS }`, com
 * `const CHANNELS: &[&str] = &[…]` no topo do arquivo. Ler só o corpo devolve zero — foi assim
 * que o placar ficou preso em "0 migrados" com sete módulos já entregues.
 */
function claimedIn(source: string): string[] {
  const bodies = [...source.matchAll(/fn channels\(&self\) -> &'static \[&'static str\] \{([\s\S]*?)\n\s*\}/g)]

  /*
   * Resolve um pedaço de código a canais: os literais que ele já tem, mais o valor de cada
   * constante que ele nomeia. São DOIS níveis de indireção no estilo real do repositório —
   * `channels()` devolve `CHANNELS`, que é `&[CH_GET_ALL, CH_SET, …]`, que são `&str` literais.
   * Parar no primeiro nível conta zero canal para o módulo e o gate credita tudo ao sidecar.
   */
  const resolve = (snippet: string, depth: number): string[] => {
    const literals = [...snippet.matchAll(/"([^"]+)"/g)].map((match) => match[1])
    if (depth === 0) return literals

    const identifiers = [...snippet.matchAll(/\b([A-Z][A-Z0-9_]{2,})\b/g)].map((match) => match[1])
    const fromConsts = identifiers.flatMap((name) => {
      const value = new RegExp(`const ${name}\\s*:[^=]*=\\s*&?\\[?([\\s\\S]*?);`).exec(source)?.[1]
      return value === undefined ? [] : resolve(value, depth - 1)
    })

    return [...literals, ...fromConsts]
  }

  return bodies.flatMap(([, body]) => resolve(body, 2))
}

const nativeClaimed = new Set(claimSources.flatMap(claimedIn))

/*
 * Piso de alcance do lado Rust, e não é hipotético: este gate lia SÓ `native/mod.rs`. Quando os
 * módulos passaram a morar em `modules/*.rs`, ele seguiu verde anunciando "0 migrados, 0%" com
 * sete módulos e mais de trinta canais já reivindicados. Gate que perde o alvo não fica vermelho,
 * fica otimista.
 *
 * `channels()` é obrigatório no trait, então todo `impl NativeModule` tem exatamente um. Se a
 * varredura achar menos deles do que implementações, é a LEITURA que quebrou — não o código.
 */
const implementations = claimSources.reduce(
  (total, source) => total + [...source.matchAll(/impl NativeModule for/g)].length,
  0,
)
const channelFns = claimSources.reduce(
  (total, source) => total + [...source.matchAll(/fn channels\(&self\) -> &'static \[&'static str\] \{/g)].length,
  0,
)

if (implementations < 5 || channelFns !== implementations) {
  violations.push({
    file: rel(MODULES),
    message:
      `leu ${channelFns} channels() para ${implementations} impl NativeModule. ` +
      'O gate não está enxergando os módulos e contaria como sidecar o que já migrou',
  })
}

/*
 * E ter achado os `channels()` não basta: o piso acima passou verde enquanto a extração devolvia
 * zero canal, porque ela parava na indireção da constante. Todo módulo implementado reivindica ao
 * menos um canal — se o total de canais não alcança o total de módulos, quem quebrou foi a leitura.
 */
if (nativeClaimed.size < implementations) {
  violations.push({
    file: rel(MODULES),
    message:
      `${implementations} módulos implementados e só ${nativeClaimed.size} canal(is) extraído(s) — ` +
      'a leitura dos literais quebrou; o placar estaria medindo menos do que já migrou',
  })
}

const everything = new Set([...invokeChannels, ...sendChannels, ...shellChannels, ...localChannels])
const stillOnSidecar = [...everything].filter(
  (channel) => !rustOwnedShell.includes(channel) && !nativeClaimed.has(channel),
)

if (stillOnSidecar.length > BASELINE) {
  violations.push({
    file: rel(NATIVE),
    message:
      `${stillOnSidecar.length} canais no sidecar, acima do baseline de ${BASELINE}. ` +
      `Canal novo nasce em Rust: ${stillOnSidecar.slice(0, 5).join(', ')}…`,
  })
}

if (violations.length > 0) {
  console.error(`✗ check:sidecar-attrition — ${violations.length} violação(ões)`)
  for (const violation of violations) console.error(`  ${violation.file}  ${violation.message}`)
  process.exit(1)
}

const migrated = STARTING_POINT - stillOnSidecar.length
const percent = Math.round((migrated / STARTING_POINT) * 100)
/* Os que faltam, nominalmente. Um placar que diz só "restam 9" obriga quem for portar o próximo a
 * redescobrir QUAIS — e a lista cabe numa linha porque ela encolhe. Quando esvaziar, some. */
console.log(
  `✓ check:sidecar-attrition — ${migrated}/${STARTING_POINT} canais migrados (${percent}%), ` +
    `${stillOnSidecar.length} ainda no sidecar [catraca: ${BASELINE}]` +
    (stillOnSidecar.length > 0 ? `\n  restam: ${[...stillOnSidecar].sort().join(', ')}` : ''),
)

/*
 * Baixar o BASELINE é parte de migrar um módulo, não uma tarefa à parte: enquanto ele não desce, o
 * gate aceita a regressão de volta. Quando `stillOnSidecar` chegar a 0, o passo seguinte é a fase
 * 10 do plano — apagar `src/sidecar`, o `externalBin` e este gate junto.
 */
if (stillOnSidecar.length < BASELINE) {
  console.log(`  baixe BASELINE para ${stillOnSidecar.length} em scripts/checks/sidecar-attrition.mts`)
}
