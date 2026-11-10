import { describe, expect, it } from 'vitest'
import { parseFencedContent } from '../code-fence'

describe('parseFencedContent', () => {
  it('returns a single text node for plain text with no backticks', () => {
    const blocks = parseFencedContent('hello world')
    expect(blocks).toEqual([{ type: 'text', nodes: [{ type: 'text', value: 'hello world' }] }])
  })

  it('extracts a fenced block with a language tag, dropping the tag and the fence markers', () => {
    const blocks = parseFencedContent('before\n```js\nconsole.log(1)\n```\nafter')
    expect(blocks).toEqual([
      { type: 'text', nodes: [{ type: 'text', value: 'before\n' }] },
      { type: 'code-block', lang: 'js', code: 'console.log(1)' },
      { type: 'text', nodes: [{ type: 'text', value: '\nafter' }] },
    ])
  })

  it('treats a fence with no valid language tag as a bare code block', () => {
    const blocks = parseFencedContent('```\n1 + 1\n```')
    expect(blocks).toEqual([{ type: 'code-block', lang: null, code: '1 + 1' }])
  })

  it('keeps a single-line fence (no newline at all) as one bare code block', () => {
    const blocks = parseFencedContent('```1 + 1```')
    expect(blocks).toEqual([{ type: 'code-block', lang: null, code: '1 + 1' }])
  })

  it('renders inline code spans separately from surrounding text', () => {
    const blocks = parseFencedContent('run `code()` now')
    expect(blocks).toEqual([
      {
        type: 'text',
        nodes: [
          { type: 'text', value: 'run ' },
          { type: 'code', value: 'code()' },
          { type: 'text', value: ' now' },
        ],
      },
    ])
  })

  it('does not let an inline backtick span cross a newline', () => {
    const blocks = parseFencedContent('a `b\nc` d')
    expect(blocks).toEqual([{ type: 'text', nodes: [{ type: 'text', value: 'a `b\nc` d' }] }])
  })

  it('treats an unterminated trailing fence as an in-progress code block (streaming LLM output)', () => {
    const blocks = parseFencedContent('Aqui está:\n```js\nfunction f() {\n  return 1')
    expect(blocks).toEqual([
      { type: 'text', nodes: [{ type: 'text', value: 'Aqui está:\n' }] },
      { type: 'code-block', lang: 'js', code: 'function f() {\n  return 1' },
    ])
  })

  it('handles multiple fenced blocks in the same message', () => {
    const blocks = parseFencedContent('```js\na()\n```\nmeio\n```python\nb()\n```')
    expect(blocks).toEqual([
      { type: 'code-block', lang: 'js', code: 'a()' },
      { type: 'text', nodes: [{ type: 'text', value: '\nmeio\n' }] },
      { type: 'code-block', lang: 'python', code: 'b()' },
    ])
  })

  it('returns an empty array for empty input', () => {
    expect(parseFencedContent('')).toEqual([])
  })
})
