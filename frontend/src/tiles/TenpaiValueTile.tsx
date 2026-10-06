import { useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { TileFrame } from '@/components/TileFrame'
import { Mahgen } from '@/components/Mahgen'
import { useAnalysisStore } from '@/stores/analysisStore'
import { fmtScore } from '@/lib/format'
import { mjaiToMahgen } from '@/lib/tileIdx'
import type { Breakpoint } from '@/tiles/defaults'

export function TenpaiValueTile({ bp }: { bp: Breakpoint }) {
  const { t } = useTranslation()
  const result = useAnalysisStore((s) => s.result)
  const waitsRef = useRef<HTMLUListElement>(null)

  // Same hand as the draws-to-tenpai tile: the 13-tile hand, or the hand
  // left after the best discard in a 14-tile state.
  const best = result?.hand14?.maintain[0] ?? null
  const after = result?.hand13 ?? best?.result ?? null
  const tenpai = after != null && after.shanten === 0 ? after : null
  const tenpaiDiscard = result?.hand13 == null ? best?.discard ?? null : null
  // riichi_point is only null for open hands.
  const closed = tenpai?.waits.some((w) => w.score?.riichi_point != null) ?? false

  const points = (n: number | null | undefined) => (n == null ? '—' : fmtScore(n))

  return (
    <TileFrame
      id="tenpai-value"
      title={t('tile.tenpai_value')}
      bp={bp}
      contentClassName="flex flex-col gap-3"
    >
      {tenpai == null ? (
        <span className="text-muted-foreground text-sm">{t('tile.tenpai_value_empty')}</span>
      ) : (
        <>
          {tenpaiDiscard != null && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>{t('tile.tenpai_waits_discard')}</span>
              <Mahgen seq={mjaiToMahgen([tenpaiDiscard])} kind="rec" />
            </div>
          )}
          <ul ref={waitsRef} className="flex flex-col gap-2">
            {tenpai.waits.map((w) => {
              const s = w.score
              return (
                <li key={w.tile} className="flex items-start gap-3">
                  <div className="flex shrink-0 flex-col items-center gap-0.5">
                    <Mahgen seq={mjaiToMahgen([w.tile])} kind="rec" containerRef={waitsRef} />
                    <span className="text-xs font-mono">{w.left}枚</span>
                  </div>
                  <div className="flex min-w-0 flex-1 flex-col gap-1 text-xs">
                    <dl className="grid grid-cols-[auto_1fr] gap-x-4">
                      <dt className="text-muted-foreground">{t('tile.tenpai_value_dama')}</dt>
                      <dd className="font-mono text-right">
                        {s != null && s.dama_point == null ? (
                          <span className="text-muted-foreground">{t('tile.tenpai_value_no_yaku')}</span>
                        ) : (
                          points(s?.dama_point)
                        )}
                      </dd>
                      {closed && (
                        <>
                          <dt className="text-muted-foreground">{t('tile.tenpai_value_riichi')}</dt>
                          <dd className="font-mono text-right">{points(s?.riichi_point)}</dd>
                        </>
                      )}
                    </dl>
                    {s != null && s.yaku_ids.length > 0 && (
                      <span className="text-muted-foreground">
                        {s.yaku_ids
                          .map((id) => t(`mahjong.yaku.${id}`, { defaultValue: `#${id}` }))
                          .join('・')}
                      </span>
                    )}
                  </div>
                </li>
              )
            })}
          </ul>
          <span className="text-[10px] text-muted-foreground">{t('tile.tenpai_value_note')}</span>
        </>
      )}
    </TileFrame>
  )
}
