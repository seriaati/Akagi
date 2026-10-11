// Game-history Zustand store + persisted PT-rule preference.
//
// Shape: a denormalised cache of `GameRecord[]` plus the active filter
// and the user's chosen PT rule. Records are loaded once on bridge
// startup via `invoke('list_game_history')` and updated incrementally
// via the `history-recorded` Tauri event. Filtering happens in-memory
// (the page is filter-driven and the dataset is small enough to scan).
//
// The PT rule and filter live in the same store so a re-render of the
// History page can subscribe to a single hook and recalculate. The
// rule is persisted to localStorage; the filter is session-only.

import { create } from 'zustand'

import type { GameRecord, HistoryFilter } from '@/types'
import { DEFAULT_CUSTOM_RULE, type PtRule } from '@/lib/ptCalc'

const PT_RULE_KEY = 'akagi.history.ptRule'

const DEFAULT_RULE: PtRule = {
  kind: 'majsoul',
  lobby: 'jade',
  dan: 'jakketsu_3',
}

function loadRule(): PtRule {
  if (typeof localStorage === 'undefined') return DEFAULT_RULE
  try {
    const raw = localStorage.getItem(PT_RULE_KEY)
    if (!raw) return DEFAULT_RULE
    const parsed = JSON.parse(raw) as PtRule
    if (
      parsed.kind === 'majsoul' ||
      parsed.kind === 'tenhou' ||
      parsed.kind === 'custom'
    ) {
      return parsed
    }
    return DEFAULT_RULE
  } catch {
    return DEFAULT_RULE
  }
}

function saveRule(rule: PtRule) {
  if (typeof localStorage === 'undefined') return
  try {
    localStorage.setItem(PT_RULE_KEY, JSON.stringify(rule))
  } catch {
    /* quota — ignore */
  }
}

function todayRange(): Pick<HistoryFilter, 'started_after' | 'started_before'> {
  const now = new Date()
  const y = now.getFullYear()
  const m = now.getMonth()
  const d = now.getDate()
  return {
    started_after: new Date(y, m, d).toISOString(),
    started_before: new Date(y, m, d + 1).toISOString(),
  }
}

type HistoryStore = {
  records: GameRecord[]
  /** True while the initial fetch is in flight. */
  loading: boolean
  filter: HistoryFilter
  /** True until the user picks a date range; while set, the History
   *  page re-anchors the range to the current day each time it opens. */
  followToday: boolean
  rule: PtRule

  setRecords: (records: GameRecord[]) => void
  setLoading: (loading: boolean) => void
  /** Insert at the top (newest-first ordering by start time). */
  prepend: (record: GameRecord) => void
  /** Drop a record by id; no-op if absent. */
  remove: (id: string) => void
  setFilter: (filter: HistoryFilter) => void
  /** User-chosen date range; stops following today. */
  setDateRange: (started_after?: string, started_before?: string) => void
  /** Re-anchor the date range to today if still following it. */
  refreshToday: () => void
  setRule: (rule: PtRule) => void
  /** Reset to a sensible default custom rule (full editable). */
  resetCustom: () => void
}

export const useHistoryStore = create<HistoryStore>((set) => ({
  records: [],
  loading: false,
  // Default to 4-player only — 3p / 4p are separate analysis modes (uma,
  // starting score, rank distribution all differ) and must never be
  // averaged together. The Game History page exposes a top-level
  // toggle to switch. Date range defaults to today (local calendar day),
  // using the same bounds `DateRangePicker` emits.
  filter: { num_players: 4, ...todayRange() },
  followToday: true,
  rule: loadRule(),

  setRecords: (records) => set({ records }),
  setLoading: (loading) => set({ loading }),
  prepend: (record) =>
    set((s) => {
      // Defensive dedup: if backend re-emits, swap in place rather than duplicate.
      const filtered = s.records.filter((r) => r.id !== record.id)
      return { records: [record, ...filtered] }
    }),
  remove: (id) =>
    set((s) => ({ records: s.records.filter((r) => r.id !== id) })),
  setFilter: (filter) => set({ filter }),
  setDateRange: (started_after, started_before) =>
    set((s) => ({
      filter: { ...s.filter, started_after, started_before },
      followToday: false,
    })),
  refreshToday: () =>
    set((s) =>
      s.followToday ? { filter: { ...s.filter, ...todayRange() } } : {},
    ),
  setRule: (rule) => {
    saveRule(rule)
    set({ rule })
  },
  resetCustom: () => {
    saveRule(DEFAULT_CUSTOM_RULE)
    set({ rule: DEFAULT_CUSTOM_RULE })
  },
}))
