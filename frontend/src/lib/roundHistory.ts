import type { MjaiEvent } from '@/types'

/** One finished kyoku of the live game. */
export type RoundResult = {
  bakaze: string
  kyoku: number
  honba: number
  oya: number
  /** Scores from `start_kyoku`, before any riichi stick was paid. */
  startScores: number[]
  outcome: 'hora' | 'ryukyoku'
  /** One entry per winner (several on a double/triple ron). `actor === target` is tsumo. */
  wins: { actor: number; target: number }[]
  /** Net score change per seat: hora/ryukyoku deltas plus −1000 per riichi stick paid. */
  net: number[]
  /** Seats that paid a riichi stick this kyoku. */
  riichi: number[]
}

export type RoundHistory = {
  names: string[]
  rounds: RoundResult[]
}

/** In-progress kyoku bookkeeping, kept outside the published state. */
export type RoundCursor = {
  start: Omit<RoundResult, 'outcome' | 'wins' | 'net' | 'riichi'> | null
  riichi: number[]
  /** Delta arrays already summed into the current result. Majsoul and Riichi
   *  City attach the same combined deltas to every hora of a double ron;
   *  Tenhou sends each winner's own. Skipping exact repeats handles both. */
  seenDeltas: number[][]
  /** True once this kyoku's result row has been appended. */
  closed: boolean
  /** A `start_game` with the same names arrived while rounds were recorded:
   *  either a reconnect (bridges re-send `start_game`) or a rematch. The next
   *  `start_kyoku` decides — a new game always opens at E1 0 honba. */
  maybeResumed: boolean
}

export const EMPTY_HISTORY: RoundHistory = { names: [], rounds: [] }
export const EMPTY_CURSOR: RoundCursor = { start: null, riichi: [], seenDeltas: [], closed: false, maybeResumed: false }

const sameItems = <T>(a: T[], b: T[]) =>
  a.length === b.length && a.every((v, i) => v === b[i])

/** Fold one live mjai event into the round history. Returns the inputs
 *  unchanged (same identity) for events that don't affect it. */
export function applyRoundEvent(
  history: RoundHistory,
  cursor: RoundCursor,
  e: MjaiEvent,
): [RoundHistory, RoundCursor] {
  switch (e.type) {
    case 'start_game':
      if (history.rounds.length > 0 && sameItems(history.names, e.names)) {
        return [history, { ...EMPTY_CURSOR, maybeResumed: true }]
      }
      return [{ names: e.names, rounds: [] }, EMPTY_CURSOR]

    case 'start_kyoku': {
      let rounds = history.rounds
      if (cursor.maybeResumed) {
        rounds = e.bakaze === 'E' && e.kyoku === 1 && e.honba === 0
          ? []
          // A reconnect replays the current kyoku; drop it if it was already recorded.
          : rounds.filter((r) => !(r.bakaze === e.bakaze && r.kyoku === e.kyoku && r.honba === e.honba))
      }
      return [
        rounds === history.rounds ? history : { ...history, rounds },
        {
          start: { bakaze: e.bakaze, kyoku: e.kyoku, honba: e.honba, oya: e.oya, startScores: e.scores },
          riichi: [],
          seenDeltas: [],
          closed: false,
          maybeResumed: false,
        },
      ]
    }

    case 'reach_accepted':
      if (!cursor.start) return [history, cursor]
      return [history, { ...cursor, riichi: [...cursor.riichi, e.actor] }]

    case 'hora':
    case 'ryukyoku': {
      const start = cursor.start
      if (!start) return [history, cursor]
      const deltas = e.deltas ?? []
      const isRepeat = cursor.seenDeltas.some((d) => sameItems(d, deltas))
      const seenDeltas = isRepeat ? cursor.seenDeltas : [...cursor.seenDeltas, deltas]
      const win = e.type === 'hora' ? [{ actor: e.actor, target: e.target }] : []

      if (!cursor.closed) {
        const net = start.startScores.map(
          (_, i) => (isRepeat ? 0 : deltas[i] ?? 0) - 1000 * cursor.riichi.filter((s) => s === i).length,
        )
        const round: RoundResult = { ...start, outcome: e.type, wins: win, net, riichi: cursor.riichi }
        return [
          { ...history, rounds: [...history.rounds, round] },
          { ...cursor, seenDeltas, closed: true },
        ]
      }

      // Further hora of the same kyoku (double/triple ron): merge into the last row.
      const last = history.rounds[history.rounds.length - 1]
      const merged: RoundResult = {
        ...last,
        wins: [...last.wins, ...win],
        net: isRepeat ? last.net : last.net.map((v, i) => v + (deltas[i] ?? 0)),
      }
      return [
        { ...history, rounds: [...history.rounds.slice(0, -1), merged] },
        { ...cursor, seenDeltas },
      ]
    }

    default:
      return [history, cursor]
  }
}
