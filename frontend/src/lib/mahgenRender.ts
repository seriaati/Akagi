// Throttled, cached rendering for <mah-gen>.
//
// mahgen's own element renders on every `data-seq` change by spawning a fresh
// Web Worker that loads its ~565 KB image library and composes the PNG in plain
// JS — no cache, no concurrency limit. The dashboard re-renders dozens of tiles
// per game event, so once spawns outpace completions the workers pile up, each
// runs slower, and the webview ends up pegging every core with hundreds of them.
// This replaces the element's `genImage` with a shared queue: results are cached
// by sequence, identical in-flight renders are shared, at most
// RENDER_CONCURRENCY workers run at once, and a queued render nobody still
// wants is skipped.
import { Mahgen, MahgenElement, ParseError } from 'mahgen'

const RENDER_CONCURRENCY = 2
const RENDER_CACHE_MAX = 300
// mahgen never settles a render whose worker fails, so don't let one hold a
// slot forever.
const RENDER_TIMEOUT_MS = 10_000

const cache = new Map<string, string>()
// Elements waiting on each queued or in-flight key.
const waiters = new Map<string, Set<HTMLElement>>()
const queue: string[] = []
let inFlight = 0

// Mode prefix + sequence: river mode lays out the same sequence differently.
function keyOf(el: HTMLElement): string | null {
  const seq = el.getAttribute('data-seq')
  if (seq === null) return null
  return (el.hasAttribute('data-river-mode') ? 'r|' : 'h|') + seq
}

function imgOf(el: HTMLElement): HTMLImageElement | null {
  return el.shadowRoot?.querySelector('img') ?? null
}

function genImage(this: HTMLElement): void {
  const key = keyOf(this)
  if (key === null) return
  const img = imgOf(this)
  if (!img) return
  if (key.length === 2) {
    // Empty sequence: nothing to draw, and not worth a worker.
    img.src = ''
    return
  }
  const hit = cache.get(key)
  if (hit !== undefined) {
    cache.delete(key)
    cache.set(key, hit)
    img.src = hit
    return
  }
  let set = waiters.get(key)
  if (!set) {
    set = new Set()
    waiters.set(key, set)
    queue.push(key)
  }
  set.add(this)
  // Deferred: the element usually gets its sequence before it is attached.
  queueMicrotask(pump)
}

function pump(): void {
  while (inFlight < RENDER_CONCURRENCY && queue.length > 0) {
    const key = queue.shift()!
    const set = waiters.get(key)!
    for (const el of set) if (!el.isConnected || keyOf(el) !== key) set.delete(el)
    if (set.size === 0) {
      waiters.delete(key)
      continue
    }
    inFlight++
    render(key.slice(2), key[0] === 'r')
      .then((src) => {
        cache.set(key, src)
        if (cache.size > RENDER_CACHE_MAX) cache.delete(cache.keys().next().value!)
        for (const el of set) {
          const img = imgOf(el)
          if (img && keyOf(el) === key) img.src = src
        }
      })
      .catch((err: unknown) => {
        if (err instanceof ParseError) {
          console.error(`[Mahgen] invalid sequence "${key.slice(2)}": ${err.message}.`)
        }
        for (const el of set) {
          const img = imgOf(el)
          if (!img || keyOf(el) !== key) continue
          img.src = ''
          img.alt = el.hasAttribute('data-show-err') && err instanceof Error ? err.message : ''
        }
      })
      .finally(() => {
        waiters.delete(key)
        inFlight--
        pump()
      })
  }
}

function render(seq: string, river: boolean): Promise<string> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('mahgen render timed out')), RENDER_TIMEOUT_MS)
    Mahgen.render(seq, river).then(resolve, reject).finally(() => clearTimeout(timer))
  })
}

;(MahgenElement.prototype as unknown as { genImage: () => void }).genImage = genImage
