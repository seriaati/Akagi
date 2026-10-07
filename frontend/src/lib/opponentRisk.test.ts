import { describe, expect, it } from 'vitest'

import { safeInHand, topRiskTiles } from './opponentRisk'

function riskVec(entries: Record<number, number>, fill = 1): number[] {
  return Array.from({ length: 34 }, (_, i) => entries[i] ?? fill)
}

describe('topRiskTiles', () => {
  it('returns the n highest-risk tiles, highest first', () => {
    const risk = riskVec({ 4: 12.8, 13: 16.2, 20: 13.1, 31: 9.5 })
    expect(topRiskTiles(risk, 3)).toEqual([
      { tile: '5p', risk: 16.2 },
      { tile: '3s', risk: 13.1 },
      { tile: '5m', risk: 12.8 },
    ])
  })

  it('drops zero-risk tiles even when fewer than n remain', () => {
    const risk = riskVec({ 0: 5, 27: 3 }, 0)
    expect(topRiskTiles(risk, 5)).toEqual([
      { tile: '1m', risk: 5 },
      { tile: 'E', risk: 3 },
    ])
  })
})

describe('safeInHand', () => {
  it('returns zero-risk hand tiles once per type, in hand order', () => {
    const risk = riskVec({ 0: 0, 13: 0, 27: 0 })
    expect(safeInHand(risk, ['E', '1m', '5pr', '1m', '2s'])).toEqual(['1m', '5pr', 'E'])
  })

  it('returns nothing when no hand tile is safe', () => {
    expect(safeInHand(riskVec({}), ['1m', '2m'])).toEqual([])
  })
})
