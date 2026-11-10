import { describe, expect, it } from 'vitest'
import { rosterKeyTarget } from '../roster-nav'

// A 3-column roster of 11 cards (9 VRM + 2 Live2D): the grid the Skin screen draws.
const COUNT = 11
const COLUMNS = 3

describe('rosterKeyTarget', () => {
  it.each([
    ['ArrowRight', 0, 1],
    ['ArrowRight', 10, 0],
    ['ArrowLeft', 0, 10],
    ['ArrowLeft', 4, 3],
    ['ArrowDown', 1, 4],
    ['ArrowDown', 6, 9],
    ['ArrowUp', 9, 6],
    ['Home', 7, 0],
    ['End', 2, 10],
  ])('%s from card %i lands on card %i', (key, from, to) => {
    expect(rosterKeyTarget(key, from, COUNT, COLUMNS)).toBe(to)
  })

  // Up/Down never wrap around the grid: a column jump off the edge would land on an unrelated card.
  it.each([
    ['ArrowUp', 1],
    ['ArrowDown', 9],
    ['ArrowDown', 8],
  ])('%s from card %i stays put at the edge', (key, from) => {
    expect(rosterKeyTarget(key, from, COUNT, COLUMNS)).toBe(from)
  })

  it('ignores keys that are not navigation, so Enter/Space still reach the card', () => {
    expect(rosterKeyTarget('Enter', 3, COUNT, COLUMNS)).toBeNull()
    expect(rosterKeyTarget(' ', 3, COUNT, COLUMNS)).toBeNull()
    expect(rosterKeyTarget('a', 3, COUNT, COLUMNS)).toBeNull()
  })

  it('answers nothing for an empty roster', () => {
    expect(rosterKeyTarget('ArrowRight', 0, 0, COLUMNS)).toBeNull()
  })
})
