import { create } from 'zustand'
import type { MjaiEvent } from '@/types'
import {
  applyRoundEvent,
  EMPTY_CURSOR,
  EMPTY_HISTORY,
  type RoundCursor,
  type RoundHistory,
} from '@/lib/roundHistory'

type RoundHistoryStore = {
  history: RoundHistory
  cursor: RoundCursor
  push: (e: MjaiEvent) => void
}

/** Per-kyoku results of the live game, rebuilt from `mjai-event`s. Reset on a
 *  new game's `start_game` (not a reconnect's); kept after `end_game` so the
 *  last game stays readable. */
export const useRoundHistoryStore = create<RoundHistoryStore>((set) => ({
  history: EMPTY_HISTORY,
  cursor: EMPTY_CURSOR,
  push: (e) =>
    set((s) => {
      const [history, cursor] = applyRoundEvent(s.history, s.cursor, e)
      return { history, cursor }
    }),
}))
