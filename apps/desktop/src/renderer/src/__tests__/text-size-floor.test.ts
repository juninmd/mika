import { readdirSync, readFileSync } from 'node:fs'
import { join, relative, sep } from 'node:path'
import { describe, expect, it } from 'vitest'

// Text under 10px is unreadable on settings and wizard surfaces; only the overlay HUD and game widgets,
// which size text to a fixed card or chip, may go below it.
const ROOTS = ['apps/desktop/src', 'packages/sdk-ui/src'].map((dir) => join(process.cwd(), dir))
const ALLOWED = [
  /widgets\//,
  /TokenCostBadge\.tsx$/,
  /\/dock\//,
  /NeuralWheel\w*\.tsx$/,
  /MoodHistoryTrail\.tsx$/,
  /MikaContextMenu\.tsx$/,
]
const TOO_SMALL = /text-\[[0-9](?:\.\d+)?px\]/

function tsxFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === 'node_modules' || entry.name === '__tests__' ? [] : tsxFiles(path)
    return entry.name.endsWith('.tsx') ? [path] : []
  })
}

describe('text size floor', () => {
  const files = ROOTS.flatMap(tsxFiles).map((path) => ({
    path,
    rel: relative(process.cwd(), path).split(sep).join('/'),
  }))

  it('scans the renderer surfaces', () => {
    expect(files.length).toBeGreaterThan(200)
  })

  it('keeps settings and wizard text at 10px or larger', () => {
    const offenders = files
      .filter(({ rel }) => !ALLOWED.some((pattern) => pattern.test(rel)))
      .filter(({ path }) => TOO_SMALL.test(readFileSync(path, 'utf8')))
      .map(({ rel }) => rel)
    expect(offenders).toEqual([])
  })
})
