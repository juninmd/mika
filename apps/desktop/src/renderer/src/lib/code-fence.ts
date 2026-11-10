// Hand-rolled fence parser — no markdown dependency. Covers exactly what the chat surfaces need:
// fenced ```lang blocks and inline `code` spans (WORK PACKAGE 6 — the model can now execute code
// via the `code` tool, and its reply must render as real code, not raw backticks — see the
// speech-bubble/chat-history smoke defect this closes).

export type InlineNode = { type: 'text'; value: string } | { type: 'code'; value: string }

export type FencedBlock =
  | { type: 'text'; nodes: InlineNode[] }
  | { type: 'code-block'; lang: string | null; code: string }

const FENCE = '```'
// Lang tags are short bare identifiers (js, ts, python, c++) — anything else on the fence's first
// line is treated as part of the code itself, not a tag (lenient: LLM output isn't always spec-correct).
const LANG_TAG_RE = /^[a-zA-Z0-9_+-]{1,20}$/
const INLINE_CODE_RE = /`([^`\n]+)`/g

function splitLangAndCode(raw: string): { lang: string | null; code: string } {
  const newlineIdx = raw.indexOf('\n')
  if (newlineIdx === -1) return { lang: null, code: raw }
  const candidate = raw.slice(0, newlineIdx).trim()
  // An empty opening line (fence on its own line, no tag) is still a line terminator to strip —
  // only a NON-empty, invalid candidate means the "first line" is actual code, not a tag attempt.
  if (!candidate) return { lang: null, code: raw.slice(newlineIdx + 1) }
  if (LANG_TAG_RE.test(candidate)) return { lang: candidate, code: raw.slice(newlineIdx + 1) }
  return { lang: null, code: raw }
}

function parseInline(text: string): InlineNode[] {
  const nodes: InlineNode[] = []
  let last = 0
  for (const match of text.matchAll(INLINE_CODE_RE)) {
    const index = match.index ?? 0
    if (index > last) nodes.push({ type: 'text', value: text.slice(last, index) })
    nodes.push({ type: 'code', value: match[1] })
    last = index + match[0].length
  }
  // Also fires when there was no match at all — a single text node carrying the whole string.
  if (last < text.length || nodes.length === 0) nodes.push({ type: 'text', value: text.slice(last) })
  return nodes
}

/**
 * Splits `input` into text/code-block segments. An unterminated trailing fence (the LLM is still
 * streaming the block) still becomes a code block instead of leaking raw backticks mid-stream.
 */
export function parseFencedContent(input: string): FencedBlock[] {
  const blocks: FencedBlock[] = []
  let cursor = 0

  while (cursor < input.length) {
    const fenceStart = input.indexOf(FENCE, cursor)
    if (fenceStart === -1) {
      blocks.push({ type: 'text', nodes: parseInline(input.slice(cursor)) })
      break
    }
    if (fenceStart > cursor) {
      blocks.push({ type: 'text', nodes: parseInline(input.slice(cursor, fenceStart)) })
    }

    const afterOpen = fenceStart + FENCE.length
    const fenceEnd = input.indexOf(FENCE, afterOpen)
    if (fenceEnd === -1) {
      const { lang, code } = splitLangAndCode(input.slice(afterOpen))
      blocks.push({ type: 'code-block', lang, code })
      break
    }

    const { lang, code } = splitLangAndCode(input.slice(afterOpen, fenceEnd))
    blocks.push({ type: 'code-block', lang, code: code.endsWith('\n') ? code.slice(0, -1) : code })
    cursor = fenceEnd + FENCE.length
  }

  return blocks
}
