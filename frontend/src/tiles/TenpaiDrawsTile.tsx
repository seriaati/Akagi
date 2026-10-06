import { useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { TileFrame } from '@/components/TileFrame'
import { Mahgen } from '@/components/Mahgen'
import { useAnalysisStore } from '@/stores/analysisStore'
import { pct } from '@/lib/format'
import { mjaiToMahgen } from '@/lib/tileIdx'
import type { Breakpoint } from '@/tiles/defaults'

export function TenpaiDrawsTile({ bp }: { bp: Breakpoint }) {
  const { t } = useTranslation()
  const result = useAnalysisStore((s) => s.result)
  const d = result?.tenpai_draws ?? null
  const shanten = d?.shanten ?? result?.shanten
  const waitsRef = useRef<HTMLUListElement>(null)

  // Same hand the backend estimates on: the 13-tile hand, or the hand left
  // after the best discard in a 14-tile state.
  const best = result?.hand14?.maintain[0] ?? null
  const after = result?.hand13 ?? best?.result ?? null
  const tenpai = after != null && after.shanten === 0 ? after : null
  const tenpaiDiscard = result?.hand13 == null ? best?.discard ?? null : null

  const draws = (n: number | null) => (n == null ? '—' : t('tile.tenpai_draws_n', { n }))
  const beyondWall = (n: number | null) => n == null || (d != null && n > d.draws_left)

  return (
    <TileFrame
      id="tenpai-draws"
      title={t('tile.tenpai_draws')}
      bp={bp}
      rightSlot={
        shanten != null && (
          <span className="rounded-full border border-border px-2 py-0.5 text-[10px] tracking-wider uppercase">
            {shanten <= 0 ? t('mahjong.tenpai') : t('mahjong.shanten_value', { n: shanten })}
          </span>
        )
      }
      contentClassName="flex flex-col gap-3"
    >
      {tenpai != null ? (
        <>
          {tenpaiDiscard != null && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>{t('tile.tenpai_waits_discard')}</span>
              <Mahgen seq={mjaiToMahgen([tenpaiDiscard])} kind="rec" />
            </div>
          )}
          <ul ref={waitsRef} className="flex flex-wrap gap-2">
            {tenpai.waits.map((w) => (
              <li key={w.tile} className="flex flex-col items-center gap-0.5">
                <Mahgen seq={mjaiToMahgen([w.tile])} kind="rec" containerRef={waitsRef} />
                <span className={`text-xs font-mono ${w.left === 0 ? 'text-muted-foreground' : ''}`}>{w.left}枚</span>
              </li>
            ))}
          </ul>
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
            <dt className="text-muted-foreground">{t('tile.tenpai_waits')}</dt>
            <dd className="font-mono text-right">{tenpai.waits_total}枚</dd>
          </dl>
        </>
      ) : d == null ? (
        <span className="text-muted-foreground text-sm">
          {shanten != null && shanten <= 0 ? t('mahjong.tenpai') : t('tile.tenpai_draws_empty')}
        </span>
      ) : (
        <>
          <div className="flex flex-col">
            {beyondWall(d.median) ? (
              <span className="text-lg font-medium">{t('tile.tenpai_draws_unlikely')}</span>
            ) : (
              <>
                <span className="text-3xl font-mono font-semibold">~{draws(d.median)}</span>
                <span className="text-xs text-muted-foreground">{t('tile.tenpai_draws_median_hint')}</span>
              </>
            )}
          </div>
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
            <dt className="text-muted-foreground">{t('tile.tenpai_draws_p80')}</dt>
            <dd className={`font-mono text-right ${beyondWall(d.p80) ? 'text-muted-foreground' : ''}`}>
              {draws(d.p80)}
            </dd>
            <dt className="text-muted-foreground">{t('tile.tenpai_draws_by_ryukyoku')}</dt>
            <dd className="font-mono text-right">{pct(d.by_ryukyoku)}</dd>
            <dt className="text-muted-foreground">{t('tile.tenpai_draws_left')}</dt>
            <dd className="font-mono text-right">{d.draws_left}</dd>
            <dt className="text-muted-foreground">{t('tile.tenpai_draws_ukeire')}</dt>
            <dd className="font-mono text-right">{d.ukeire}枚</dd>
          </dl>
          {!d.exact && (
            <span className="text-[10px] text-muted-foreground">{t('tile.tenpai_draws_approx')}</span>
          )}
        </>
      )}
    </TileFrame>
  )
}
