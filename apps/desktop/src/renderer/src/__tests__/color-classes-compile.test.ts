import { readdirSync, readFileSync, statSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join, relative } from 'node:path'
import { __unstable__loadDesignSystem as loadDesignSystem } from 'tailwindcss'
import { describe, expect, it } from 'vitest'

// A color class whose token is not in @theme compiles to nothing, silently: `ring-focus` had no
// `--color-focus` and 122 focus rings fell back to currentColor, and `border-primary-rim` left a
// bare `border` painted in the text color. Tailwind itself is the judge of what compiles.
const root = process.cwd()
const cssPath = join(root, 'apps/desktop/src/renderer/src/styles.css')
const require = createRequire(cssPath)

const COLOR_CLASS =
  /^(?:[\w-]+:|\[[^\]]*\]:)*!?-?(bg|text|border(?:-[trblxyse])?|ring|ring-offset|outline|fill|stroke|from|via|to|divide|placeholder|decoration|shadow|caret|accent)-([a-z][\w-]*)(\/[\d.]+|\/\[[^\]]+\])?!?$/
// Model ids in a provider catalog look like `text-*` classes but are data, not class names.
const NOT_A_CLASS = /^text-(embedding|multilingual-embedding)-/

function sources(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name)
    if (name === 'node_modules' || name === '__tests__') continue
    if (statSync(path).isDirectory()) sources(path, out)
    else if (/\.tsx?$/.test(name) && !name.includes('.test.')) out.push(path)
  }
  return out
}

function colorCandidates(): Map<string, string> {
  const found = new Map<string, string>()
  const dirs = ['apps/desktop/src/renderer', 'apps/desktop/src/features', 'packages/sdk-ui/src']
  for (const file of dirs.flatMap((d) => sources(join(root, d)))) {
    for (const literal of readFileSync(file, 'utf8').matchAll(/(["'`])((?:(?!\1)[^\\\n]){3,})\1/g)) {
      for (const token of literal[2].split(/\s+/)) {
        if (COLOR_CLASS.test(token) && !NOT_A_CLASS.test(token) && !found.has(token)) {
          found.set(token, relative(root, file))
        }
      }
    }
  }
  return found
}

async function designSystem(css: string) {
  return loadDesignSystem(css, {
    base: dirname(cssPath),
    loadStylesheet: async (id: string, base: string) => {
      if (id !== 'tailwindcss') return { path: id, base, content: '' }
      const path = require.resolve('tailwindcss/index.css')
      return { path, base: dirname(path), content: readFileSync(path, 'utf8') }
    },
  })
}

describe('color classes', () => {
  const css = readFileSync(cssPath, 'utf8')

  it('every color utility used in the UI compiles to a rule', async () => {
    const ds = await designSystem(css)
    const candidates = colorCandidates()
    expect(candidates.size).toBeGreaterThan(200)
    const compiled = ds.candidatesToCss([...candidates.keys()])
    const dead = [...candidates].filter((_, i) => !compiled[i]).map(([cls, file]) => `${cls} (${file})`)
    expect(dead).toEqual([])
  })

  it('catches a class whose theme token is missing', async () => {
    const ds = await designSystem(css.replace(/^\s*--color-focus:.*$/m, ''))
    expect(ds.candidatesToCss(['ring-focus', 'ring-accent'])).toEqual([null, expect.any(String)])
  })
})
