import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { TileFrame } from '@/components/TileFrame'
import { Mahgen } from '@/components/Mahgen'
import { useAnalysisStore } from '@/stores/analysisStore'
import { useGameStore } from '@/stores/gameStore'
import { useRoundHistoryStore } from '@/stores/roundHistoryStore'
import { pct, relativeKind } from '@/lib/format'
import { playerName } from '@/lib/roundHistory'
import { mjaiToMahgen } from '@/lib/tileIdx'
import { safeInHand, topRiskTiles } from '@/lib/opponentRisk'
import { riskColor } from '@/lib/riskColor'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import type { OpponentRisk } from '@/types'
import type { Breakpoint } from '@/tiles/defaults'

// Stable empty fallback — selector must not allocate a new array per render.
// Zustand v5 uses Object.is on the selector output and would loop forever.
const NO_OPPONENTS: readonly OpponentRisk[] = []
const NO_TILES: readonly string[] = []

// How many likely-wait tiles to show per opponent.
const TOP_N = 5

export function OpponentsTile({ bp }: { bp: Breakpoint }) {
  const { t } = useTranslation()
  const opponents = useAnalysisStore((s) => s.result?.opponents ?? NO_OPPONENTS)
  const ourSeat = useGameStore((s) => s.game?.our_seat ?? null)
  const tehai = useGameStore((s) =>
    s.game && s.game.our_seat != null ? s.game.players[s.game.our_seat]?.tehai ?? NO_TILES : NO_TILES,
  )
  const numPlayers = useGameStore((s) => s.game?.num_players ?? 4)
  const names = useRoundHistoryStore((s) => s.history.names)

  const label = (seat: number) => {
    const name = playerName(names, seat) ?? t('tile.player_n', { n: seat + 1 })
    return ourSeat == null ? name : `${name} (${t(`mahjong.${relativeKind(seat, ourSeat, numPlayers)}`)})`
  }

  return (
    <TileFrame id="opponents" title={t('tile.opponents')} bp={bp} contentClassName="p-0">
      {opponents.length === 0 ? (
        <span className="text-muted-foreground text-sm p-3 block">{t('tile.opponents_empty')}</span>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="text-[10px] uppercase">{t('tile.opponents_player')}</TableHead>
              <TableHead className="text-[10px] uppercase">{t('tile.opponents_tenpai')}</TableHead>
              <TableHead className="text-[10px] uppercase">{t('tile.opponents_riichi')}</TableHead>
              <TableHead className="text-[10px] uppercase">{t('tile.opponents_waits')}</TableHead>
              <TableHead className="text-[10px] uppercase">{t('tile.opponents_safe')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {opponents.map((o) => (
              <OpponentRow
                key={o.seat}
                o={o}
                label={label(o.seat)}
                tehai={tehai}
              />
            ))}
          </TableBody>
        </Table>
      )}
    </TileFrame>
  )
}

function OpponentRow({ o, label, tehai }: { o: OpponentRisk; label: string; tehai: readonly string[] }) {
  const waits = useMemo(() => topRiskTiles(o.risk, TOP_N), [o.risk])
  const safe = useMemo(() => safeInHand(o.risk, tehai), [o.risk, tehai])
  return (
    <TableRow>
      <TableCell className="max-w-40 truncate" title={label}>{label}</TableCell>
      <TableCell className="font-mono">{pct(o.tenpai_rate)}</TableCell>
      <TableCell>{o.is_riichi ? '●' : '—'}</TableCell>
      <TableCell>
        {waits.length === 0 ? '—' : (
          <div className="flex gap-1">
            {waits.map((w) => (
              <div key={w.tile} className="flex flex-col items-center gap-0.5">
                <Mahgen seq={mjaiToMahgen([w.tile])} kind="opp-risk" className="leading-[0]" />
                <span
                  className="min-w-[2.4em] rounded px-1 py-0.5 text-center text-[10px] font-mono tabular-nums leading-none text-white"
                  style={{ backgroundColor: riskColor(w.risk).chip }}
                >
                  {w.risk.toFixed(1)}
                </span>
              </div>
            ))}
          </div>
        )}
      </TableCell>
      <TableCell>
        {safe.length === 0 ? '—' : <Mahgen seq={mjaiToMahgen(safe)} kind="opp-risk" className="leading-[0]" />}
      </TableCell>
    </TableRow>
  )
}
