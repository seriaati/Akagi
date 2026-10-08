import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'

import { DateRangePicker } from './DateRangePicker'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('DateRangePicker', () => {
  beforeEach(() => {
    // Non-UTC zone so local-day vs UTC-day bugs show up.
    vi.stubEnv('TZ', 'Asia/Tokyo')
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(new Date('2026-10-08T12:00:00+09:00'))
  })
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllEnvs()
  })

  it('shows "any" with no clear button when unset', () => {
    render(<DateRangePicker onChange={() => {}} />)
    expect(screen.getByText('history.filter.any')).toBeTruthy()
    expect(screen.queryByLabelText('common.clear')).toBeNull()
  })

  it('selecting one day covers that whole local day', () => {
    const onChange = vi.fn()
    render(<DateRangePicker onChange={onChange} />)
    fireEvent.click(screen.getByText('history.filter.any'))
    fireEvent.click(document.querySelector('[data-day="10/8/2026"]')!)
    expect(onChange).toHaveBeenCalledWith(
      '2026-10-07T15:00:00.000Z',
      '2026-10-08T15:00:00.000Z',
    )
  })

  it('labels a set day and clears it', () => {
    const onChange = vi.fn()
    render(
      <DateRangePicker
        after="2026-10-07T15:00:00.000Z"
        before="2026-10-08T15:00:00.000Z"
        onChange={onChange}
      />,
    )
    expect(screen.getByText('10/8/2026')).toBeTruthy()
    fireEvent.click(screen.getByLabelText('common.clear'))
    expect(onChange).toHaveBeenCalledWith(undefined, undefined)
  })
})
