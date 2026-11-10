import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, extname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { THEMES } from '../themes'

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(HERE, '../../../../../..')
const STYLES_PATH = resolve(HERE, '../styles.css')

const SCAN_ROOTS = [
  resolve(HERE, '../..'),
  resolve(HERE, '../../../features'),
  resolve(REPO_ROOT, 'packages/sdk-ui/src'),
]

const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx', '.css'])

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'dist') continue
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) walk(full, out)
    else if (SOURCE_EXTENSIONS.has(extname(full))) out.push(full)
  }
  return out
}

const sourceFiles = SCAN_ROOTS.flatMap((root) => walk(root))
const stylesCss = readFileSync(STYLES_PATH, 'utf8')

const CSS_DECLARATION = /(?:^|[;{])[ \t]*(--[\w-]+)\s*:\s*([^;]+);/gm

function cssDeclarations(css: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of css.matchAll(CSS_DECLARATION)) found.set(match[1] as string, (match[2] as string).trim())
  return found
}

const declaredInCss = cssDeclarations(stylesCss)

const SCOPED_INLINE = /['"](--[\w-]+)['"]\s*(?:as\s+\w+\s*\])?\s*:/g

const VAR_REFERENCE = /var\(\s*(--[\w-]+)\s*([,)])/g

function collectScopedInlineTokens(): Set<string> {
  const scoped = new Set<string>()
  for (const file of sourceFiles) {
    if (extname(file) === '.css') continue
    for (const match of readFileSync(file, 'utf8').matchAll(SCOPED_INLINE)) scoped.add(match[1] as string)
  }
  return scoped
}

function collectOrphanTokens(scopedInline: Set<string>): Map<string, string[]> {
  const orphans = new Map<string, string[]>()
  for (const file of sourceFiles) {
    for (const match of readFileSync(file, 'utf8').matchAll(VAR_REFERENCE)) {
      const token = match[1] as string
      if (match[2] !== ')' || declaredInCss.has(token) || scopedInline.has(token)) continue
      const callers = orphans.get(token) ?? []
      if (callers.length < 3) callers.push(relative(REPO_ROOT, file).replaceAll('\\', '/'))
      orphans.set(token, callers)
    }
  }
  return orphans
}

describe('piso de tokens de tema (:root do styles.css)', () => {
  it('define todo token que o renderer lê sem fallback', () => {
    const orphans = collectOrphanTokens(collectScopedInlineTokens())

    expect(
      Object.fromEntries(orphans),
      'token lido sem fallback e sem definição: a declaração inteira vira `unset` em runtime. ' +
        'Defina no :root de styles.css (com o valor de THEMES[0]) ou passe um fallback no var().',
    ).toEqual({})
  })

  it('copia os valores de THEMES[0] em vez de inventar cor', () => {
    const floor: Record<string, keyof (typeof THEMES)[0]['colors']> = {
      '--bg': 'background',
      '--surface': 'surface',
      '--glass-layer': 'glassLayer',
      '--primary': 'primary',
      '--text': 'text',
      '--text-muted': 'textMuted',
      '--border': 'border',
      '--neon-primary': 'neonAccent',
      '--neon-secondary': 'neonSecondary',
      '--neural-purple': 'neuralPurple',
      '--neural-cyan': 'neuralCyan',
      '--neural-mint': 'neuralMint',
      '--neural-coral': 'neuralCoral',
      '--mika-text': 'text',
      '--mika-accent': 'accent',
    }
    const defaultTheme = THEMES[0]
    if (!defaultTheme) throw new Error('THEMES está vazio')

    for (const [token, colorKey] of Object.entries(floor)) {
      expect(declaredInCss.get(token), `${token} sumiu do :root de styles.css`).toBe(defaultTheme.colors[colorKey])
    }
  })
})

const LOCAL_CLASS = /(?<![-\w/])((?:vt|scan|thinking|electron)-[a-z][a-z0-9]*(?:-[a-z0-9]+)*)\b/g

// `\bimport\(`: cobre `typeof import('./kebab-named-file')` — sem isto, o caminho de um
// import dinâmico/type-only era lido como classe local e reprovava por um nome de arquivo.
const MODULE_PATH_LINE = /^\s*import\b|\bfrom\s+['"]|\bimport\(/

const COMMENT_LINE = /^\s*(?:\/\/|\*|\/\*)/

function stripInlineComment(line: string): string {
  const at = line.indexOf('//')
  return at === -1 ? line : line.slice(0, at)
}

function collectDeclaredClasses(): Set<string> {
  const declared = new Set<string>()
  for (const match of stylesCss.matchAll(/\.([a-z][\w-]*)\s*(?:,|\{|:)/g)) declared.add(match[1] as string)
  for (const match of stylesCss.matchAll(/@utility\s+([a-z][\w-]*)/g)) declared.add(match[1] as string)
  return declared
}

function localClassesUsedIn(file: string): Set<string> {
  const names = new Set<string>()
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    if (MODULE_PATH_LINE.test(line) || COMMENT_LINE.test(line)) continue
    for (const match of stripInlineComment(line).matchAll(LOCAL_CLASS)) names.add(match[1] as string)
  }
  return names
}

function collectOrphanClasses(declared: Set<string>): Map<string, string> {
  const orphans = new Map<string, string>()
  for (const file of sourceFiles) {
    if (extname(file) === '.css') continue
    for (const name of localClassesUsedIn(file)) {
      if (declared.has(name) || orphans.has(name)) continue
      orphans.set(name, relative(REPO_ROOT, file).replaceAll('\\', '/'))
    }
  }
  return orphans
}

describe('classes locais do projeto', () => {
  it('tem regra em styles.css para toda classe local usada no renderer', () => {
    const orphans = collectOrphanClasses(collectDeclaredClasses())

    expect(Object.fromEntries(orphans), 'classe local sem regra em styles.css — o estilo é ignorado').toEqual({})
  })
})
