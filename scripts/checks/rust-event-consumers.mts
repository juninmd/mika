/**
 * check:rust-event-consumers — toda `EventKey` PUBLICADA em produção tem de ser lida por alguém
 * que ASSINA o barramento em produção.
 *
 * Existe porque a classe de defeito acabou de acontecer três vezes seguidas nesta árvore, e nenhuma
 * delas dava erro, log ou teste vermelho:
 *
 *  - `Expression`: a tool `avatar` publicava desde sempre e ninguém encaminhava à janela — o avatar
 *    nunca trocava de expressão;
 *  - `AssistantMessage`: o recap diário e o debate publicavam, e o texto morria no `data` da tool;
 *  - `SpeechRequest`: TRÊS publicadores (`character::speak_action`, `scheduler`, comentário de
 *    jogo) e ZERO assinantes — a Mika não falava nada que não fosse resposta de chat.
 *
 * Os dois lados têm teste e os dois passam: o publicador prova que publica, e o consumidor que não
 * existe não tem o que provar. Só um gate que cruza as duas listas enxerga o buraco.
 *
 * O que conta como consumo: a chave aparecer num arquivo que chama `.subscribe()` fora de teste.
 * Não é o mesmo que "reage certo" — é o piso de alcance (ver AGENTS.md): cobrar a forma exata do
 * `if`/`match` só ensinaria a contornar o gate.
 */
import { read, report, type Violation, walk } from './lib.mts'

const ROOT = 'apps/desktop/src-tauri/src'

/**
 * Publicada sem assinante em Rust, com a razão. Entrada aqui é decisão explícita no diff, e cada
 * uma sai quando o consumidor nascer.
 */
const ACCEPTED: ReadonlyMap<string, string> = new Map([
  [
    'VadAudio',
    'consumido pelo renderer (`mika:event:voice:vad-audio`), não por assinante Rust — a captura mora no webview',
  ],
  ['VadSample', 'idem VadAudio: o medidor de fala é UI, e o throttle de 100 Hz já acontece antes de publicar'],
])

/** Fonte de produção: arquivo de teste inteiro fora, e o `mod tests` do fim cortado. */
function productionSource(file: string): string | null {
  const source = read(file)
  if (source.split('\n').some((line) => line.trim() === '#![cfg(test)]')) return null
  const cut = source.indexOf('#[cfg(test)]')
  return cut === -1 ? source : source.slice(0, cut)
}

const published = new Map<string, string>()
const consumed = new Set<string>()

for (const file of walk(ROOT, (rel) => rel.endsWith('.rs'))) {
  const source = productionSource(file)
  if (!source) continue

  for (const match of source.matchAll(/publish\(\s*EventKey::(\w+)/g)) {
    if (!published.has(match[1])) published.set(match[1], file)
  }

  // Só um arquivo que ASSINA conta como consumidor: citar a chave num `json!` ou num comentário
  // não faz o evento chegar a lugar nenhum.
  if (!source.includes('.subscribe()')) continue
  for (const match of source.matchAll(/EventKey::(\w+)/g)) consumed.add(match[1])
}

const violations: Violation[] = []

if (published.size === 0) {
  violations.push({
    file: ROOT,
    line: 1,
    message: 'nenhum `publish(EventKey::…)` encontrado — o gate deixou de enxergar o barramento e passaria verde à toa',
  })
}

for (const [key, file] of published) {
  if (consumed.has(key) || ACCEPTED.has(key)) continue
  violations.push({
    file,
    line: 1,
    message: `EventKey::${key} é publicada e nenhum assinante Rust a lê — o evento entra no barramento e morre ali; ligue o consumidor ou declare em ACCEPTED com o motivo`,
  })
}

for (const [key, reason] of ACCEPTED) {
  if (published.has(key)) continue
  violations.push({
    file: 'scripts/checks/rust-event-consumers.mts',
    line: 1,
    message: `EventKey::${key} está em ACCEPTED (${reason}) e não é mais publicada — remova a entrada`,
  })
}

report('check:rust-event-consumers', violations, published.size + consumed.size)
