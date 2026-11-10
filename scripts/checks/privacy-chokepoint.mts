/**
 * check:privacy-chokepoint — pega redação de PII e autorização de rede chamadas por fora da nascente
 * declarada (repetição campo a campo pelo chamador já reincidiu duas vezes aqui). Fora das nascentes
 * abaixo, a chamada é proibida.
 *
 * Stage C (2026-09-22): as nascentes TypeScript (`src/main/services/vision/*`,
 * `infrastructure/os/foreground-window.ts`, `browser.service.ts`) saíram com `src/main`. O gate
 * ficou escopado ao que resta em TypeScript (`src/features/*`) — hoje vazio de propósito (ver
 * `REDACTION_FUNNELS`/`NETWORK_FUNNELS` abaixo).
 *
 * `specs/11-quality-gates.md` ("Ainda não automatizados") listava como `[não verificado]` se este
 * gate cobre o porte Rust de `person-catalog` e `character`. Medido: `person_catalog/pii.rs`
 * (`redact_pii`/`redact_secrets`) é chamado de três nascentes de entrada (RPC do painel, tool-call
 * do modelo, extração passiva de menção) — três chamadores, nunca um funil único, e nenhum gate
 * cobria isso; a seção `RUST_REDACTION_FUNNELS` abaixo fecha o buraco. `character` não tem código de
 * redação nenhum — não tem `redact_pii`/`redact_secrets` em lugar nenhum do módulo, então não há
 * "redação por chamador" ali para o gate proteger; se um caminho novo em `character` vier a chamar
 * `redact_pii`, este mesmo gate (roots abaixo incluem `features/character`) já vai exigir a
 * declaração da nascente, do mesmo jeito que faz para `person_catalog`.
 */
import { isSource, isTest, lineOf, read, report, type Violation, walk } from './lib.mts'

/** Onde `redactPii` pode aparecer, e por quê. Fora daqui é redação por chamador. Vazio: nenhuma
 * nascente de PII sobrevive em TypeScript — a redação de tela é toda Rust agora (ver acima). */
const REDACTION_FUNNELS: ReadonlyMap<string, string> = new Map([])

/** Serviços onde autorizar rede é responsabilidade de um método só. Vazio pelo mesmo motivo. */
const NETWORK_FUNNELS: ReadonlyMap<string, string> = new Map([])

const files = [...walk('apps/desktop/src/features', (rel) => isSource(rel) && !isTest(rel))]

const violations: Violation[] = []
const funnelsSeen = new Set<string>()
const networkFunnelsSeen = new Set<string>()

for (const file of files) {
  const source = read(file)

  for (const match of source.matchAll(/(?<![.\w])redactPii\s*\(/g)) {
    if (REDACTION_FUNNELS.has(file)) {
      funnelsSeen.add(file)
      continue
    }
    violations.push({
      file,
      line: lineOf(source, match.index ?? 0),
      message:
        'redactPii fora das nascentes declaradas em scripts/checks/privacy-chokepoint.mts — redija onde o dado nasce, não em cada consumidor',
    })
  }

  const funnelName = NETWORK_FUNNELS.get(file)
  if (funnelName === undefined) continue
  networkFunnelsSeen.add(file)

  // No serviço com funil declarado, só o próprio funil chama a guarda de rede.
  const body = source.split(`private async ${funnelName}`)
  if (body.length < 2) {
    violations.push({
      file,
      line: 1,
      message: `funil de rede "${funnelName}" declarado no gate e ausente do arquivo`,
    })
    continue
  }

  const outsideFunnel = body[0] + body[1].split('\n  }').slice(1).join('\n  }')
  for (const match of outsideFunnel.matchAll(/(?<![.\w])assertNetworkAllowed\s*\(/g)) {
    violations.push({
      file,
      line: lineOf(outsideFunnel, match.index ?? 0),
      message: `assertNetworkAllowed fora de ${funnelName} — chamar só metade do par deixa a rede sair no modo offline`,
    })
  }
}

for (const funnel of REDACTION_FUNNELS.keys()) {
  if (!funnelsSeen.has(funnel)) {
    violations.push({
      file: 'scripts/checks/privacy-chokepoint.mts',
      line: 1,
      message: `nascente "${funnel}" declarada e não redige mais nada — remova a entrada ou restaure a redação`,
    })
  }
}

for (const funnel of NETWORK_FUNNELS.keys()) {
  if (!networkFunnelsSeen.has(funnel)) {
    violations.push({
      file: 'scripts/checks/privacy-chokepoint.mts',
      line: 1,
      message: `funil de rede "${funnel}" declarado e o arquivo não existe — o gate não está olhando o serviço`,
    })
  }
}

// Lado Rust: `person-catalog` e `character` — dado de terceiro nunca sai de log/telemetria/erro
// (AGENTS.md, "Segurança"), e a mesma redação-por-chamador que reincidiu duas vezes no TS pode
// reincidir aqui. Escopo dos dois módulos de feature onde o `[não verificado]` da spec apontava.
const RUST_ROOTS = [
  'apps/desktop/src-tauri/src/modules/features/person_catalog',
  'apps/desktop/src-tauri/src/modules/features/character',
]

/** Arquivo Rust onde `redact_pii`/`redact_secrets` (de `person_catalog::pii`) pode ser chamado, e por
 * quê — cada um é uma nascente de entrada distinta, não repetição do mesmo dado. */
const RUST_REDACTION_FUNNELS: ReadonlyMap<string, string> = new Map([
  ['apps/desktop/src-tauri/src/modules/features/person_catalog/rpc.rs', 'entrada pelo painel de edição (RPC)'],
  ['apps/desktop/src-tauri/src/modules/features/person_catalog/tool.rs', 'entrada por tool-call do modelo'],
  [
    'apps/desktop/src-tauri/src/modules/features/person_catalog/ingest.rs',
    'extração passiva de menção no chat (ingestão extraída de lifecycle.rs)',
  ],
])

const rustFunnelsSeen = new Set<string>()

// Definição das funções, não chamador — excluído da varredura de nascente.
const RUST_REDACTION_SOURCE = 'apps/desktop/src-tauri/src/modules/features/person_catalog/pii.rs'

for (const root of RUST_ROOTS) {
  for (const file of walk(
    root,
    (rel) => rel.endsWith('.rs') && !/_tests?\.rs$/.test(rel) && rel !== RUST_REDACTION_SOURCE,
  )) {
    const source = read(file)
    const cut = source.indexOf('#[cfg(test)]')
    const production = cut === -1 ? source : source.slice(0, cut)

    for (const match of production.matchAll(/(?<![.\w])redact_(?:pii|secrets)\s*\(/g)) {
      if (RUST_REDACTION_FUNNELS.has(file)) {
        rustFunnelsSeen.add(file)
        continue
      }
      violations.push({
        file,
        line: lineOf(production, match.index ?? 0),
        message:
          'redact_pii/redact_secrets fora das nascentes Rust declaradas em scripts/checks/privacy-chokepoint.mts — redija onde o dado nasce, não em cada consumidor',
      })
    }
  }
}

for (const funnel of RUST_REDACTION_FUNNELS.keys()) {
  if (!rustFunnelsSeen.has(funnel)) {
    violations.push({
      file: 'scripts/checks/privacy-chokepoint.mts',
      line: 1,
      message: `nascente Rust "${funnel}" declarada e não redige mais nada — remova a entrada ou restaure a redação`,
    })
  }
}

report(
  'check:privacy-chokepoint',
  violations,
  REDACTION_FUNNELS.size + NETWORK_FUNNELS.size + RUST_REDACTION_FUNNELS.size,
)
