import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { isNamedIcon } from '../../Icons'

// The backend names each tab's icon; an unknown name silently degrades to a two-letter monogram.
const RUST_SRC = join(process.cwd(), 'apps/desktop/src-tauri/src')
const TAB_ICON = /"tabIcon":\s*"([A-Za-z0-9]+)"/g

function rustFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return rustFiles(path)
    return entry.name.endsWith('.rs') ? [path] : []
  })
}

function declaredTabIcons(): Set<string> {
  const icons = new Set<string>()
  for (const file of rustFiles(RUST_SRC)) {
    for (const match of readFileSync(file, 'utf8').matchAll(TAB_ICON)) icons.add(match[1])
  }
  return icons
}

describe('tab icons declared by the backend', () => {
  it('reads enough declarations to mean something', () => {
    expect(declaredTabIcons().size).toBeGreaterThanOrEqual(8)
  })

  it('every declared icon resolves in the renderer map', () => {
    const missing = [...declaredTabIcons()].filter((name) => !isNamedIcon(name))
    expect(missing).toEqual([])
  })
})
