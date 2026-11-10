import { describe, expect, it } from 'vitest'
import { choiceFocusIndex, choiceGridCols, choiceTargetIndex, isDenseChoice } from '../glass-choice-cards-helpers'

/** Colunas por breakpoint de container, na ordem em que as classes aparecem (a última é a mais larga). */
function columnsOf(classes: string): number[] {
  return [...classes.matchAll(/grid-cols-(\d+)/g)].map((m) => Number(m[1]))
}

describe('choiceTargetIndex — teclado de radiogroup', () => {
  it.each([
    ['ArrowRight', 0, 3, 1],
    ['ArrowDown', 1, 3, 2],
    ['ArrowRight', 2, 3, 0],
    ['ArrowLeft', 0, 3, 2],
    ['ArrowUp', 2, 3, 1],
    ['Home', 2, 3, 0],
    ['End', 0, 3, 2],
  ])('%s a partir de %i (de %i) vai para %i, com volta nas pontas', (key, index, count, expected) => {
    expect(choiceTargetIndex(key, index, count)).toBe(expected)
  })

  it('ignora tecla que não navega (Tab e Enter seguem o fluxo nativo)', () => {
    expect(choiceTargetIndex('Tab', 0, 3)).toBeNull()
    expect(choiceTargetIndex('Enter', 1, 3)).toBeNull()
  })
})

describe('choiceFocusIndex — roving tabindex', () => {
  const options = [{ value: 'a' }, { value: 'b' }, { value: 'c' }]

  it('a opção marcada é a única alcançável por Tab', () => {
    expect(choiceFocusIndex(options, 'c')).toBe(2)
  })

  it('valor fora da lista (config antiga) ainda deixa o grupo alcançável pela primeira opção', () => {
    expect(choiceFocusIndex(options, 'removido')).toBe(0)
  })
})

describe('isDenseChoice', () => {
  it('só é compacto quando nenhuma opção tem descrição', () => {
    expect(isDenseChoice([{ description: undefined }, {}])).toBe(true)
    expect(isDenseChoice([{}, { description: 'explica' }])).toBe(false)
  })
})

describe('choiceGridCols — linhas completas', () => {
  it.each([2, 3, 4, 5, 6])('%i opções: o layout mais largo não deixa card órfão (rico e compacto)', (count) => {
    for (const dense of [false, true]) {
      const widest = columnsOf(choiceGridCols(count, dense)).at(-1) ?? 1
      expect(count % widest, `${count} opções, dense=${dense}, ${widest} colunas`).toBe(0)
    }
  })

  it('3 opções ricas nunca quebram em 2+1: ou empilham ou ficam lado a lado', () => {
    expect(columnsOf(choiceGridCols(3, false))).toEqual([1, 3])
  })

  it('acima de 6 opções usa o layout de 6 em vez de cair numa coluna só', () => {
    expect(choiceGridCols(9, false)).toBe(choiceGridCols(6, false))
  })

  it('uma opção só ocupa a linha inteira', () => {
    expect(columnsOf(choiceGridCols(1, false))).toEqual([1])
  })
})
