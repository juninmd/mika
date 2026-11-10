import type { ReactNode } from 'react'
import { parseFencedContent } from '../lib/code-fence'
import { highlightText } from '../lib/highlight-text'
import { CodeBlock, InlineCode } from './CodeBlock'

/**
 * Renders `content` with fenced ```code blocks and inline `code` spans as real code (monospace,
 * copyable) instead of raw backticks — shared by the chat history bubble and the speech-bubble
 * overlay (`compact`). Search highlighting only applies to plain-text segments.
 */
export function FencedText({
  content,
  highlight,
  highlightRegex,
  compact,
}: {
  content: string
  highlight?: string
  highlightRegex?: RegExp | null
  compact?: boolean
}): ReactNode {
  return parseFencedContent(content).map((block, blockIndex) => {
    if (block.type === 'code-block') {
      // biome-ignore lint/suspicious/noArrayIndexKey: blocks are a fresh, order-stable parse of `content` on every render.
      return <CodeBlock key={blockIndex} lang={block.lang} code={block.code} compact={compact} />
    }
    return (
      // biome-ignore lint/suspicious/noArrayIndexKey: same rationale as above.
      <span key={blockIndex}>
        {block.nodes.map((node, nodeIndex) =>
          node.type === 'code' ? (
            // biome-ignore lint/suspicious/noArrayIndexKey: same rationale as above.
            <InlineCode key={nodeIndex} value={node.value} />
          ) : (
            // biome-ignore lint/suspicious/noArrayIndexKey: same rationale as above.
            <span key={nodeIndex}>{highlightText(node.value, highlight ?? '', highlightRegex ?? null)}</span>
          ),
        )}
      </span>
    )
  })
}
