import { describe, expect, it } from 'vitest'
import { seeded } from '../../../sdk/__tests__/fuzzy/prng'
import { cn } from '../../src/utils/cn'

const rand = seeded(55)

const CLASS_POOL = [
  'p-2',
  'p-4',
  'text-sm',
  'text-lg',
  'font-bold',
  'text-micro',
  'text-caption',
  'bg-red-500',
  'bg-blue-500',
  'flex',
  'grid',
  'hidden',
  'block',
]

function randomClass(): string {
  return CLASS_POOL[Math.floor(rand() * CLASS_POOL.length)] as string
}

function randomInput(): unknown {
  const type = Math.floor(rand() * 7)
  switch (type) {
    case 0:
      return randomClass()
    case 1:
      return undefined
    case 2:
      return null
    case 3:
      return false
    case 4:
      return ''
    case 5: {
      const obj: Record<string, boolean> = {}
      for (let i = 0; i < 3; i++) {
        obj[randomClass()] = rand() > 0.5
      }
      return obj
    }
    case 6: {
      const arr = []
      for (let i = 0; i < 3; i++) {
        arr.push(rand() > 0.5 ? randomClass() : undefined)
      }
      return arr
    }
  }
}

describe('cn - Fuzzy Tests', () => {
  it('should not crash and always return a string', () => {
    for (let i = 0; i < 1000; i++) {
      const args = []
      for (let j = 0; j < Math.floor(rand() * 10) + 1; j++) {
        args.push(randomInput())
      }
      const result = cn(...args)
      expect(typeof result).toBe('string')
    }
  })
})
