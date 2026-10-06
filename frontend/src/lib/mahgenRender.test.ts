import { describe, expect, it, vi } from 'vitest'

// Stand-in for the library: same element contract (open shadow root holding an
// <img>, re-render on `data-seq`), with renders the test settles by hand.
const pending = vi.hoisted(() => [] as { seq: string; resolve: (src: string) => void }[])
vi.mock('mahgen', () => {
  class MahgenElement extends HTMLElement {
    static get observedAttributes() {
      return ['data-seq']
    }
    constructor() {
      super()
      this.attachShadow({ mode: 'open' }).appendChild(document.createElement('img'))
    }
    attributeChangedCallback() {
      ;(this as unknown as { genImage: () => void }).genImage()
    }
  }
  customElements.define('mah-gen', MahgenElement)
  return {
    MahgenElement,
    ParseError: class extends Error {},
    Mahgen: {
      render: vi.fn(
        (seq: string) => new Promise<string>((resolve) => pending.push({ seq, resolve })),
      ),
    },
  }
})

const { Mahgen } = await import('mahgen')
await import('./mahgenRender')
const render = vi.mocked(Mahgen.render)

function tile(seq: string): HTMLElement {
  const el = document.createElement('mah-gen')
  el.setAttribute('data-seq', seq)
  document.body.appendChild(el)
  return el
}

const src = (el: HTMLElement) => el.shadowRoot!.querySelector('img')!.getAttribute('src')
const flush = () => new Promise((r) => setTimeout(r, 0))

async function settleAll() {
  await flush()
  while (pending.length) {
    const { seq, resolve } = pending.shift()!
    resolve(`data:${seq}`)
    await flush()
  }
}

describe('mahgen render queue', () => {
  it('renders a sequence shown by many tiles once', async () => {
    render.mockClear()
    const tiles = Array.from({ length: 20 }, () => tile('1m'))
    await settleAll()
    expect(render).toHaveBeenCalledTimes(1)
    for (const t of tiles) expect(src(t)).toBe('data:1m')
  })

  it('serves a rendered sequence from cache', async () => {
    render.mockClear()
    const t = tile('1m')
    expect(render).not.toHaveBeenCalled()
    expect(src(t)).toBe('data:1m')
  })

  it('never runs more than two renders at once', async () => {
    render.mockClear()
    const tiles = Array.from({ length: 10 }, (_, i) => tile(`${i + 1}p`))
    await flush()
    expect(pending).toHaveLength(2)
    await settleAll()
    expect(render).toHaveBeenCalledTimes(10)
    tiles.forEach((t, i) => expect(src(t)).toBe(`data:${i + 1}p`))
  })

  it('skips a queued render nobody wants any more', async () => {
    render.mockClear()
    tile('1s')
    tile('2s')
    const changed = tile('3s')
    const removed = tile('4s')
    changed.setAttribute('data-seq', '5s')
    removed.remove()
    await settleAll()
    expect(render.mock.calls.map(([seq]) => seq)).toEqual(['1s', '2s', '5s'])
    expect(src(changed)).toBe('data:5s')
  })
})
