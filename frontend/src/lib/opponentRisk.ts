import { TILE_LABELS_34, sortMjaiTiles, tileIdx } from '@/lib/tileIdx'

export type RiskEntry = { tile: string; risk: number }

// The `n` most dangerous tile types against one opponent (their likely waits),
// highest first. Zero-risk tiles are never a wait, so they are dropped.
export function topRiskTiles(risk: readonly number[], n: number): RiskEntry[] {
  return risk
    .map((r, i) => ({ tile: TILE_LABELS_34[i], risk: r }))
    .filter((e) => e.risk > 0)
    .sort((a, b) => b.risk - a.risk)
    .slice(0, n)
}

// Tiles in our hand the risk engine rates fully safe (0%) against one opponent,
// one per tile type, in hand order.
export function safeInHand(risk: readonly number[], tehai: readonly string[]): string[] {
  const seen = new Set<number>()
  const out: string[] = []
  for (const tile of sortMjaiTiles([...tehai])) {
    const idx = tileIdx(tile)
    if (idx < 0 || seen.has(idx) || risk[idx] !== 0) continue
    seen.add(idx)
    out.push(tile)
  }
  return out
}
