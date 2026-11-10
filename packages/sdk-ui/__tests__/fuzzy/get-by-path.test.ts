import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { getByPath } from '../../src/utils/get-by-path'

const ITERATIONS = 500
const KEYS = ['a', 'b', 'c', 'd', 'e']

type Tree = { [key: string]: Tree | number }
type Leaves = Array<[path: string, value: number]>

function buildTree(rand: () => number, depth: number, leaves: Leaves, prefix: string[]): Tree {
  const tree: Tree = {}
  for (const key of KEYS) {
    if (rand() < 0.4) continue
    const path = [...prefix, key]
    if (depth === 0 || rand() < 0.4) {
      const value = Math.floor(rand() * 1000)
      tree[key] = value
      leaves.push([path.join('.'), value])
    } else {
      tree[key] = buildTree(rand, depth - 1, leaves, path)
    }
  }
  return tree
}

function randomTree(rand: () => number, depth: number): { tree: Tree; leaves: Leaves } {
  const leaves: Leaves = []
  return { tree: buildTree(rand, depth, leaves, []), leaves }
}

describe('getByPath (fuzzy, seeded)', () => {
  it('resolve toda folha de uma árvore aleatória pelo caminho pontuado', () => {
    const rand = seeded(0xa11ce)
    for (let i = 0; i < ITERATIONS; i++) {
      const { tree, leaves } = randomTree(rand, 3)
      for (const [path, value] of leaves) expect(getByPath(tree, path)).toBe(value)
    }
  })

  it('uma chave literal com ponto ganha do aninhado em qualquer árvore', () => {
    const rand = seeded(0xb0b)
    for (let i = 0; i < ITERATIONS; i++) {
      const { tree, leaves } = randomTree(rand, 3)
      for (const [path] of leaves.filter(([p]) => p.includes('.'))) {
        expect(getByPath({ ...tree, [path]: `literal-${i}` }, path)).toBe(`literal-${i}`)
      }
    }
  })

  it('nunca vaza propriedades herdadas do protótipo como se fossem config', () => {
    const inherited = ['constructor', '__proto__', 'toString', 'hasOwnProperty', 'valueOf']
    const rand = seeded(0xc0de)
    for (let i = 0; i < ITERATIONS; i++) {
      const { tree } = randomTree(rand, 2)
      const key = inherited[Math.floor(rand() * inherited.length)] as string
      const prefix = rand() < 0.5 ? '' : `${KEYS[Math.floor(rand() * KEYS.length)]}.`
      expect(getByPath(tree, `${prefix}${key}`)).toBeUndefined()
    }
  })
})

// --- Added to ensure edge cases aren't missed while maintaining the solid original suite ---
describe('getByPath (fuzzy, edge cases)', () => {
  it('should return undefined when accessing properties on non-objects or nulls', () => {
    const rand = seeded(0x1337)
    for (let i = 0; i < ITERATIONS; i++) {
      const type = Math.floor(rand() * 5)
      let obj: unknown
      if (type === 0) obj = null
      else if (type === 1) obj = undefined
      else if (type === 2) obj = rand() * 100
      else if (type === 3) obj = 'string'
      else obj = rand() > 0.5

      const key = KEYS[Math.floor(rand() * KEYS.length)]
      expect(getByPath(obj, key)).toBeUndefined()
      expect(getByPath(obj, `${key}.b.c`)).toBeUndefined()
    }
  })
})
