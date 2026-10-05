import { describe, expect, it } from 'vitest'

import { applyRoundEvent, EMPTY_CURSOR, EMPTY_HISTORY } from './roundHistory'
import type { MjaiEvent } from '@/types'

function run(events: MjaiEvent[]) {
  let [history, cursor] = [EMPTY_HISTORY, EMPTY_CURSOR]
  for (const e of events) [history, cursor] = applyRoundEvent(history, cursor, e)
  return history
}

const startGame: MjaiEvent = { type: 'start_game', names: ['a', 'b', 'c', 'd'] }
const startKyoku = (kyoku: number, scores: number[], honba = 0): MjaiEvent => ({
  type: 'start_kyoku', bakaze: 'E', dora_marker: '1m', kyoku, honba, kyotaku: 0, oya: kyoku - 1, scores, tehais: [],
})
const START = [25000, 25000, 25000, 25000]

describe('applyRoundEvent', () => {
  it('records a ron with winner, loser and net deltas', () => {
    const h = run([
      startGame,
      startKyoku(1, START),
      { type: 'hora', actor: 2, target: 0, deltas: [-8000, 0, 8000, 0] },
      { type: 'end_kyoku' },
    ])
    expect(h.names).toEqual(['a', 'b', 'c', 'd'])
    expect(h.rounds).toHaveLength(1)
    expect(h.rounds[0]).toMatchObject({ outcome: 'hora', wins: [{ actor: 2, target: 0 }], net: [-8000, 0, 8000, 0] })
  })

  it('charges riichi sticks on top of the hora deltas', () => {
    // Seat 1 riichis and loses; seat 3 wins and collects the stick via deltas.
    const h = run([
      startGame,
      startKyoku(1, START),
      { type: 'reach_accepted', actor: 1 },
      { type: 'hora', actor: 3, target: 1, deltas: [0, -3900, 0, 4900] },
    ])
    expect(h.rounds[0].net).toEqual([0, -4900, 0, 4900])
    expect(h.rounds[0].riichi).toEqual([1])
  })

  it('counts shared double-ron deltas once (Majsoul / Riichi City)', () => {
    const shared = [-12000, 8000, 4000, 0]
    const h = run([
      startGame,
      startKyoku(1, START),
      { type: 'hora', actor: 1, target: 0, deltas: shared },
      { type: 'hora', actor: 2, target: 0, deltas: shared },
    ])
    expect(h.rounds).toHaveLength(1)
    expect(h.rounds[0].wins).toEqual([{ actor: 1, target: 0 }, { actor: 2, target: 0 }])
    expect(h.rounds[0].net).toEqual(shared)
  })

  it('sums per-winner double-ron deltas (Tenhou)', () => {
    const h = run([
      startGame,
      startKyoku(1, START),
      { type: 'hora', actor: 1, target: 0, deltas: [-8000, 8000, 0, 0] },
      { type: 'hora', actor: 2, target: 0, deltas: [-4000, 0, 4000, 0] },
    ])
    expect(h.rounds[0].net).toEqual([-12000, 8000, 4000, 0])
  })

  it('records draws, including abortive ones without deltas', () => {
    const h = run([
      startGame,
      startKyoku(1, START),
      { type: 'ryukyoku', deltas: [1500, -1500, 1500, -1500] },
      startKyoku(1, [26500, 23500, 26500, 23500], 1),
      { type: 'reach_accepted', actor: 0 },
      { type: 'ryukyoku' },
    ])
    expect(h.rounds.map((r) => r.outcome)).toEqual(['ryukyoku', 'ryukyoku'])
    expect(h.rounds[1]).toMatchObject({ honba: 1, net: [-1000, 0, 0, 0], wins: [] })
  })

  it('resets on a new game but keeps rounds after end_game', () => {
    const played = run([
      startGame,
      startKyoku(1, START),
      { type: 'hora', actor: 0, target: 0, deltas: [6000, -2000, -2000, -2000] },
      { type: 'end_game' },
    ])
    expect(played.rounds).toHaveLength(1)
    const next = applyRoundEvent(played, EMPTY_CURSOR, { type: 'start_game', names: ['e', 'f', 'g', 'h'] })[0]
    expect(next).toEqual({ names: ['e', 'f', 'g', 'h'], rounds: [] })
  })
})
