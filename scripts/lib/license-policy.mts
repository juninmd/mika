/**
 * Política de licença das dependências de PRODUÇÃO — pura, sem I/O.
 *
 * Existe separada de `license-inventory.mts` para ser testável sem `cargo`/`pnpm`: a pergunta "esta
 * expressão SPDX é vendável na Steam?" é lógica pura, e é a que um erro de leitura de `OR`/`AND`
 * transformaria em instalador com GPL dentro, sem ninguém perceber.
 *
 * Regras (decisão de produto, não de jurídico — `deny.toml` espelha a mesma lista para o Rust):
 * - GPL/AGPL: sempre reprovadas, mesmo com exceção de linkagem (a exceção é caso a caso, e vira
 *   entrada em `POLICY_EXCEPTIONS` com motivo, nunca padrão).
 * - LGPL: reprovada, salvo `WITH <exceção>` explícita na expressão.
 * - Não-comerciais / source-available (CC-BY-NC, SSPL, BUSL, Elastic, Commons-Clause): reprovadas.
 * - Copyleft fraco (MPL, EPL, CDDL): aprovadas, mas listadas à parte com oferta de fonte.
 * - `OR`: basta UMA alternativa aprovada (escolhe-se a menos restritiva); `AND`: todas.
 * - Licença ausente/desconhecida: reprovada — "sem licença" é "todos os direitos reservados".
 */

export type Verdict = {
  ok: boolean
  /** Ids efetivamente escolhidos (uma alternativa por `OR`). */
  ids: string[]
  weakCopyleft: boolean
  problems: string[]
}

/** `nome` do pacote → motivo. Vazio de propósito: toda entrada é uma decisão jurídica registrada. */
export const POLICY_EXCEPTIONS: Readonly<Record<string, string>> = {}

// Sem exigir hífen nem fim de texto: o id SPDX sem versão (`GPL`, `AGPL`, `LGPL`) e a grafia legada que
// o npm ainda aceita (`GPLv3`, `GPL3`, `AGPLv3`, `LGPLv2.1`) também são copyleft — um id desconhecido cai
// em "aprovada" em `classify`, então o que o regex não nomeia vira instalador com GPL dentro.
const STRONG_COPYLEFT = /^A?GPL/i
const LGPL = /^LGPL/i
const NO_ASSERTION = /^NOASSERTION$/i
const NON_COMMERCIAL = /^(?:CC-BY-NC|CC-BY-ND|SSPL|BUSL|Elastic|Commons-Clause|LicenseRef-Proprietary|Proprietary)/i
const WEAK_COPYLEFT = /^(?:MPL|EPL|CDDL|EUPL|OSL)-/i

type Node = { ids: string[]; weak: boolean; problems: string[] }

function tokenize(expression: string): string[] {
  // `A/B` e `A / B` são a grafia legada de `A OR B` ainda presente em crates antigas.
  const normalized = expression.replace(/\s*\/\s*/g, ' OR ')
  return normalized.match(/\(|\)|[^\s()]+/g) ?? []
}

function classify(id: string, hasException: boolean): Node {
  if (NO_ASSERTION.test(id)) return { ids: [id], weak: false, problems: [`${id}: licença não declarada`] }
  if (STRONG_COPYLEFT.test(id)) return { ids: [id], weak: false, problems: [`${id}: copyleft forte`] }
  if (LGPL.test(id) && !hasException) return { ids: [id], weak: false, problems: [`${id}: LGPL sem exceção`] }
  if (NON_COMMERCIAL.test(id)) return { ids: [id], weak: false, problems: [`${id}: não permite venda`] }
  return { ids: [id], weak: WEAK_COPYLEFT.test(id) || LGPL.test(id), problems: [] }
}

/** Menos restritiva primeiro: aprovada sem copyleft < aprovada com copyleft fraco < reprovada. */
const rank = (node: Node): number => (node.problems.length > 0 ? 2 : node.weak ? 1 : 0)

class Parser {
  private position = 0
  constructor(private readonly tokens: string[]) {}

  parse(): Node {
    const node = this.or()
    if (this.position < this.tokens.length) throw new Error(`token inesperado "${this.tokens[this.position]}"`)
    return node
  }

  private peek(): string | undefined {
    return this.tokens[this.position]
  }

  private or(): Node {
    const alternatives = [this.and()]
    while (this.peek()?.toUpperCase() === 'OR') {
      this.position++
      alternatives.push(this.and())
    }
    return alternatives.reduce((best, next) => (rank(next) < rank(best) ? next : best))
  }

  private and(): Node {
    const parts = [this.atom()]
    while (this.peek()?.toUpperCase() === 'AND') {
      this.position++
      parts.push(this.atom())
    }
    return {
      ids: parts.flatMap((part) => part.ids),
      weak: parts.some((part) => part.weak),
      problems: parts.flatMap((part) => part.problems),
    }
  }

  private atom(): Node {
    const token = this.peek()
    if (token === undefined) throw new Error('expressão termina cedo demais')
    this.position++
    if (token === '(') {
      const inner = this.or()
      if (this.peek() !== ')') throw new Error('parêntese não fechado')
      this.position++
      return inner
    }
    const hasException = this.peek()?.toUpperCase() === 'WITH'
    if (hasException) this.position += 2
    return classify(token.replace(/\+$/, '-or-later'), hasException)
  }
}

export function evaluateLicense(expression: string | null | undefined, packageName = ''): Verdict {
  const text = (expression ?? '').trim()
  if (text === '' || /^(?:unknown|unlicensed|see license in .*)$/i.test(text)) {
    return { ok: false, ids: [], weakCopyleft: false, problems: ['licença ausente ou desconhecida'] }
  }

  let node: Node
  try {
    node = new Parser(tokenize(text)).parse()
  } catch (error) {
    return {
      ok: false,
      ids: [],
      weakCopyleft: false,
      problems: [`expressão SPDX ilegível "${text}": ${String(error)}`],
    }
  }

  const excepted = packageName in POLICY_EXCEPTIONS
  return {
    ok: node.problems.length === 0 || excepted,
    ids: node.ids,
    weakCopyleft: node.weak,
    problems: excepted ? [] : node.problems,
  }
}
