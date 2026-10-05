import { useTranslation } from 'react-i18next'
import { TileFrame } from '@/components/TileFrame'
import { useGameStore } from '@/stores/gameStore'
import { useRoundHistoryStore } from '@/stores/roundHistoryStore'
import { fmtScore, kyokuLabel } from '@/lib/format'
import { cn } from '@/lib/utils'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import type { RoundResult } from '@/lib/roundHistory'
import type { Breakpoint } from '@/tiles/defaults'

export function RoundHistoryTile({ bp }: { bp: Breakpoint }) {
  const { t } = useTranslation()
  const { names, rounds } = useRoundHistoryStore((s) => s.history)
  const ourSeat = useGameStore((s) => s.game?.our_seat ?? null)
  const numPlayers = rounds[0]?.startScores.length ?? 0

  // Bridges fill missing names with "" or the seat number — fall back to P1..P4.
  const label = (seat: number) => {
    const name = names[seat]
    return name && name !== String(seat) ? name : `P${seat + 1}`
  }

  const resultText = (r: RoundResult) => {
    if (r.outcome === 'ryukyoku') return t('mahjong.ryukyoku')
    return r.wins
      .map((w) =>
        w.actor === w.target
          ? t('tile.round_history_tsumo', { winner: label(w.actor) })
          : t('tile.round_history_ron', { winner: label(w.actor), loser: label(w.target) }),
      )
      .join(' / ')
  }

  return (
    <TileFrame id="round-history" title={t('tile.round_history')} bp={bp} contentClassName="p-0">
      {rounds.length === 0 ? (
        <span className="text-muted-foreground text-sm p-3 block">{t('tile.round_history_empty')}</span>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="text-[10px] uppercase">{t('mahjong.round')}</TableHead>
              <TableHead className="text-[10px] uppercase">{t('tile.round_history_result')}</TableHead>
              {Array.from({ length: numPlayers }, (_, seat) => (
                <TableHead
                  key={seat}
                  className={cn('text-[10px] text-right max-w-24 truncate', seat === ourSeat && 'text-foreground')}
                  title={label(seat)}
                >
                  {label(seat)}
                  {seat === ourSeat && ` (${t('mahjong.self')})`}
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rounds
              .map((r, i) => (
                <TableRow key={i}>
                  <TableCell className="whitespace-nowrap">
                    <div className="font-medium">{kyokuLabel(r.bakaze, r.kyoku)}</div>
                    {r.honba > 0 && (
                      <div className="text-[10px] text-muted-foreground">
                        {t('tile.round_history_honba', { n: r.honba })}
                      </div>
                    )}
                  </TableCell>
                  <TableCell className="text-xs">{resultText(r)}</TableCell>
                  {r.net.map((delta, seat) => {
                    const won = r.wins.some((w) => w.actor === seat)
                    const dealtIn = r.wins.some((w) => w.target === seat && w.actor !== seat)
                    return (
                      <TableCell key={seat} className="text-right font-mono whitespace-nowrap">
                        <div
                          className={cn(
                            delta > 0 ? 'text-emerald-400' : delta < 0 ? 'text-red-400' : 'text-muted-foreground',
                            (won || dealtIn) && 'font-semibold',
                          )}
                        >
                          {delta > 0 ? '+' : ''}
                          {delta === 0 ? '±0' : fmtScore(delta)}
                        </div>
                        <div className="text-[10px] text-muted-foreground">
                          {fmtScore(r.startScores[seat] + delta)}
                        </div>
                      </TableCell>
                    )
                  })}
                </TableRow>
              ))
              .reverse()}
          </TableBody>
        </Table>
      )}
    </TileFrame>
  )
}
