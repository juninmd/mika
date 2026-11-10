/**
 * check:events — pega evento emitido sem ouvinte (integração só aparente) e ouvinte sem emissor.
 * Precisa de cinco resoluções (constante do catálogo, alias local, ponte IPC do renderer, o
 * barramento Rust por `.publish(EventKey::X)`/consumo por match-arm, e os dois consumidores diretos
 * do VAD que nunca passam pelo barramento) ou metade do resultado vira falso positivo — a Stage C
 * (2026-09-22) só enxergava `app.emit_to` casado por SUBSTRING no MESMO arquivo, cega para todo
 * `bus.publish(EventKey::X, ..)`/`bus().publish(..)` e para qualquer consumo Rust que não fosse a
 * ponte HUD (48 violações na rodada que motivou isto — a maioria gate cego, não defeito real; os
 * poucos defeitos reais foram corrigidos nos próprios módulos Rust/TS, não aqui).
 */
import { isSource, isTest, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const CATALOG = 'packages/sdk/src/svc/events.ts'
const BRIDGE = 'apps/desktop/src/shared/mika-event-channels.ts'
// Stage C (2026-09-22): o emissor de produção migrou de `webContents.send` (Electron) para
// `app.emit_to(label, channel, payload)` (Tauri) em Rust — `src/main` que emitia estes eventos
// não existe mais. Os canais Rust são prefixados (`mika:event:chat:assistant-message`), então o
// valor do catálogo aparece como SUBSTRING da constante Rust, não como literal exato.
const RUST_ROOT = 'apps/desktop/src-tauri/src'
const RUST_KEYS = 'apps/desktop/src-tauri/src/events/keys.rs'

const catalogSource = read(CATALOG)
const valueByKey = new Map<string, string>()
for (const match of catalogSource.matchAll(/^\s+([A-Z][A-Z0-9_]*):\s*'([^']+)'/gm)) {
  valueByKey.set(match[1], match[2])
}
const knownValues = new Set(valueByKey.values())

const files = [...sourceRoots().flatMap((root) => walk(root, (rel) => isSource(rel) && !isTest(rel)))].filter(
  (rel) => rel !== CATALOG,
)

/** `const FOO = Events.BAR` e `const FOO = 'chat:bar'` — o alias tem de resolver para o valor. */
function aliasesIn(source: string): Map<string, string> {
  const aliases = new Map<string, string>()
  for (const match of source.matchAll(/(?:const|let)\s+([A-Z][A-Z0-9_]*)\s*=\s*Events\.([A-Z0-9_]+)/g)) {
    const value = valueByKey.get(match[2])
    if (value !== undefined) aliases.set(match[1], value)
  }
  for (const match of source.matchAll(/(?:const|let)\s+([A-Z][A-Z0-9_]*)\s*=\s*'([^']+)'/g)) {
    if (knownValues.has(match[2])) aliases.set(match[1], match[2])
  }
  return aliases
}

const resolve = (token: string, aliases: Map<string, string>): string | null => {
  const constant = /^Events\.([A-Z0-9_]+)$/.exec(token)
  if (constant) return valueByKey.get(constant[1]) ?? null

  const literal = /^'([^']+)'$/.exec(token)
  if (literal) return knownValues.has(literal[1]) ? literal[1] : null

  return aliases.get(token) ?? null
}

const emitted = new Map<string, string>()
const listened = new Map<string, string>()

// `emit` chega por três formas no repo: método (`this.events.emit`), helper importado (`emitEvent`)
// e a variante livre (`emit(...)`) que as features usam. Enxergar só a primeira produz "ouvinte
// morto" para metade do catálogo.
const EMIT = /(?:\.emit|\bemit|\bemitEvent|\bpublish)\s*\(\s*([^,)\s]+)/g
// `registerHandler` é registro de ouvinte com outro nome — `AwarenessService` só usa essa forma.
const LISTEN = /(?:\.(?:on|once|subscribe|off)|\bregisterHandler|\bonEvent)\s*\(\s*([^,)\s]+)/g
// `webContents.send` atravessa o IPC: é emissão E consumo, o ouvinte só mora no renderer.
const SEND = /webContents\??\.send\s*\(\s*([^,)\s]+)/g

for (const file of files) {
  const source = read(file)
  const aliases = aliasesIn(source)

  for (const match of source.matchAll(EMIT)) {
    const value = resolve(match[1], aliases)
    if (value !== null && !emitted.has(value)) emitted.set(value, file)
  }
  for (const match of source.matchAll(LISTEN)) {
    const value = resolve(match[1], aliases)
    if (value !== null && !listened.has(value)) listened.set(value, file)
  }
  for (const match of source.matchAll(SEND)) {
    const value = resolve(match[1], aliases)
    if (value === null) continue
    if (!emitted.has(value)) emitted.set(value, file)
    if (!listened.has(value)) listened.set(value, `${file} (IPC → renderer)`)
  }
}

// Terceira resolução: quem está na ponte tem consumidor no renderer.
const bridgeSource = read(BRIDGE)
for (const value of knownValues) {
  if (bridgeSource.includes(`'${value}'`)) listened.set(value, BRIDGE)
}

// Quarta resolução: o barramento Rust (`crate::events::EventBus`). `EventKey` é uma enum, não uma
// string — o literal exato mora só em `events/keys.rs::as_str()` (`Self::Foo => "bar:baz",`), então
// primeiro isto constrói variante->literal a partir de lá.
const rustKeysSource = read(RUST_KEYS)
const rustValueByVariant = new Map<string, string>()
for (const match of rustKeysSource.matchAll(/Self::(\w+)\s*=>\s*"([^"]+)",/g)) {
  rustValueByVariant.set(match[1], match[2])
}

// `EMIT_TO` continua à parte: canais IPC prefixados (`mika:event:chat:...`) onde o valor do
// catálogo é SUBSTRING da constante Rust, não o literal exato — caso de `chat/send.rs`,
// `chat/assistant_bridge.rs`, `features/character/expression_bridge.rs`.
const RUST_EMIT_TO = /\.emit_to\(/
// Publicação real: `bus.publish(EventKey::X, ..)` / `bus().publish(EventKey::X, ..)` /
// `crate::events::bus().publish(crate::events::EventKey::X, ..)` — `\s*` cruza linha (chamada
// multilinha é o estilo predominante do repo).
const RUST_PUBLISH = /\.publish\(\s*(?:crate::events::)?EventKey::(\w+)/g
// Consumo real: `match envelope.key { EventKey::X => .. }`, `EventKey::X | EventKey::Y => ..`,
// `if let EventKey::X = envelope.key`, `envelope.key == EventKey::X` / `key == EventKey::X`, e o
// guard de filtro `envelope.key != EventKey::X { continue; }` (`affinity_notice.rs`) — igualdade E
// desigualdade contam: as duas provam que o código LÊ `envelope.key` para decidir o que fazer com
// esta chave específica, que é a definição de "alguém escuta" deste gate.
const RUST_LISTEN_ARM = /EventKey::(\w+)\s*(?:=>|\|)/g
const RUST_LISTEN_EQ = /(?:[=!]=\s*EventKey::(\w+)|EventKey::(\w+)\s*[=!]=)/g
const RUST_LISTEN_IF_LET = /if\s+let\s+EventKey::(\w+)\s*=/g
// Tabela de despacho: `const BRIDGED: &[(EventKey, &str)] = &[(EventKey::X, "canal"), ..]` —
// `modules/hud_bridge.rs::channel_for` varre isto contra `envelope.key`, então uma entrada aqui é
// tão consumo quanto um match-arm, só que via `.find()` em vez de `match`. A vírgula sozinha
// também casaria com `.publish(EventKey::X, json!(..))` — mesma forma sintática, significado
// oposto (emissor, não consumidor). Exige aspas logo depois da vírgula (o segundo elemento da
// tupla é sempre `&str`) para não creditar todo publish como "alguém escuta" — probe sintético
// que pegou isso: `.probe-tmp/` (2026-09-22, apagado depois de confirmar).
const RUST_LISTEN_TABLE = /\(EventKey::(\w+),\s*"/g

// `VadAudio`/`VadSample` nunca passam pelo barramento para o consumidor real — `vad_pipeline.rs`
// chama `BargeInRecorder::absorb`/`InterruptionDetector::feed_vad_sample` DIRETO, na mesma chamada
// síncrona (ver o cabeçalho de `vad_pipeline.rs`: "nunca pelo `crate::events::EventBus`... aceitável
// para `VadSample`, não para o áudio bruto"). Resolução estreita, não um allowlist: cada chave abaixo
// tem o nome exato da função que a consome, provado pelos próprios testes daqueles dois arquivos.
const RUST_DIRECT_CONSUMERS: Record<string, RegExp> = {
  VadAudio: /\.absorb\(/,
  VadSample: /\.feed_vad_sample\(/,
}

for (const file of walk(RUST_ROOT, (rel) => rel.endsWith('.rs'))) {
  const full = read(file)
  if (full.split('\n').some((line) => line.trim() === '#![cfg(test)]')) continue
  const cut = full.indexOf('#[cfg(test)]')
  const source = cut === -1 ? full : full.slice(0, cut)

  if (RUST_EMIT_TO.test(source)) {
    for (const value of knownValues) {
      if (!emitted.has(value) && source.includes(value)) emitted.set(value, `${file} (Rust emit_to)`)
    }
  }

  for (const match of source.matchAll(RUST_PUBLISH)) {
    const value = rustValueByVariant.get(match[1])
    if (value !== undefined && !emitted.has(value)) emitted.set(value, `${file} (Rust bus.publish)`)
  }

  for (const regex of [RUST_LISTEN_ARM, RUST_LISTEN_EQ, RUST_LISTEN_IF_LET, RUST_LISTEN_TABLE]) {
    for (const match of source.matchAll(regex)) {
      const variant = match[1] ?? match[2]
      const value = variant === undefined ? undefined : rustValueByVariant.get(variant)
      if (value !== undefined && !listened.has(value)) listened.set(value, `${file} (Rust consumer)`)
    }
  }

  for (const [variant, callSite] of Object.entries(RUST_DIRECT_CONSUMERS)) {
    const value = rustValueByVariant.get(variant)
    if (value !== undefined && !listened.has(value) && callSite.test(source)) {
      listened.set(value, `${file} (Rust direct call, sem barramento)`)
    }
  }
}

const violations: Violation[] = []

for (const [key, value] of valueByKey) {
  const hasEmitter = emitted.has(value)
  const hasListener = listened.has(value)

  if (hasEmitter && !hasListener) {
    violations.push({
      file: `${CATALOG} (Events.${key})`,
      line: 1,
      message: `emitido em ${emitted.get(value)} e ninguém escuta — integração aparente`,
    })
  }
  if (!hasEmitter && hasListener) {
    violations.push({
      file: `${CATALOG} (Events.${key})`,
      line: 1,
      message: `escutado em ${listened.get(value)} e ninguém emite — ouvinte morto`,
    })
  }
  if (!hasEmitter && !hasListener) {
    violations.push({
      file: `${CATALOG} (Events.${key})`,
      line: 1,
      message: 'declarado no catálogo, sem emissor e sem ouvinte',
    })
  }
}

report('check:events', violations, valueByKey.size)
