import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// Unlayered CSS beats every Tailwind utility; the global focus rule outside a layer squared rounded
// controls on focus (`border-radius: inherit`) and overrode `focus-visible:outline-none`.
const css = readFileSync(join(process.cwd(), 'apps/desktop/src/renderer/src/styles.css'), 'utf8')

function enclosingLayer(source: string, index: number): string | null {
  let depth = 0
  for (let i = index; i >= 0; i--) {
    if (source[i] === '}') depth++
    else if (source[i] === '{') {
      if (depth === 0) {
        const header = source.slice(source.lastIndexOf('\n', i) + 1, i).trim()
        return header.startsWith('@layer') ? header : enclosingLayer(source, i - 1)
      }
      depth--
    }
  }
  return null
}

describe('global focus ring', () => {
  const at = css.indexOf(':focus-visible {')

  it('exists', () => {
    expect(at).toBeGreaterThan(-1)
  })

  it('lives in the base layer so utilities can override it', () => {
    expect(enclosingLayer(css, at)).toBe('@layer base')
  })

  it('skips programmatic focus targets, which framed whole dialogs and dropdown panels', () => {
    const selector = css.slice(css.lastIndexOf('\n', at) + 1, at)
    expect(selector).toContain('[tabindex]:not([tabindex="-1"])')
  })

  it('uses the focus token, not the raw accent', () => {
    expect(css.slice(at, css.indexOf('}', at))).toContain('var(--color-focus)')
  })

  it('never rewrites the control shape', () => {
    expect(css).not.toMatch(/border-radius:\s*inherit/)
  })
})
