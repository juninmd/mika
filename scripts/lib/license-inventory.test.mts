import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { inputHash, TEXTS_DIR } from './license-inventory.mts'

const roots: string[] = []

function repoWith(eol: '\n' | '\r\n'): string {
  const root = mkdtempSync(join(tmpdir(), 'mika-license-inputs-'))
  roots.push(root)
  mkdirSync(join(root, TEXTS_DIR), { recursive: true })
  const lines = (text: string) => text.split('\n').join(eol)
  writeFileSync(join(root, 'Cargo.lock'), lines('# lock\nversion = 4\n\n[[package]]\nname = "a"\n'))
  writeFileSync(join(root, 'pnpm-lock.yaml'), lines('lockfileVersion: 9\n'))
  writeFileSync(join(root, TEXTS_DIR, 'apache-2.0.txt'), lines('Apache License\nVersion 2.0\n'))
  return root
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

describe('inputHash', () => {
  it('é o mesmo num checkout com CRLF (Windows) e com LF — senão check:license-policy reprova a release', () => {
    expect(inputHash(repoWith('\r\n'))).toBe(inputHash(repoWith('\n')))
  })

  it('muda quando o CONTEÚDO do Cargo.lock muda', () => {
    const changed = repoWith('\n')
    writeFileSync(join(changed, 'Cargo.lock'), '# lock\nversion = 4\n\n[[package]]\nname = "b"\n')
    expect(inputHash(changed)).not.toBe(inputHash(repoWith('\n')))
  })

  it('ignora subpasta dentro de license-texts (um __pycache__ não derruba o gate)', () => {
    const root = repoWith('\n')
    const before = inputHash(root)
    mkdirSync(join(root, TEXTS_DIR, '__pycache__'))
    expect(inputHash(root)).toBe(before)
  })
})
