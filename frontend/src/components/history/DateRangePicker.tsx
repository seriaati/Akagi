// Date-range filter for the History page. Replaces native
// `<input type="date">`, which WebKitGTK renders with today's date as a
// grey placeholder when empty — indistinguishable from a real filter.
//
// Days are local-calendar days: `after` is local midnight of the first
// day (inclusive), `before` is local midnight of the day after the last
// day (exclusive), matching `HistoryFilter`'s RFC3339 bounds.

import { CalendarIcon, XIcon } from 'lucide-react'
import type { DateRange } from 'react-day-picker'
import { enUS, ja, zhCN, zhTW } from 'react-day-picker/locale'
import { useTranslation } from 'react-i18next'

import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { cn } from '@/lib/utils'

const LOCALES = { en: enUS, ja, 'zh-CN': zhCN, 'zh-TW': zhTW }

export function DateRangePicker({
  after,
  before,
  onChange,
}: {
  after?: string
  before?: string
  onChange: (after?: string, before?: string) => void
}) {
  const { t, i18n } = useTranslation()
  const lang = i18n.language
  const locale = LOCALES[lang as keyof typeof LOCALES] ?? enUS

  const from = after ? new Date(after) : undefined
  const to = before ? addDays(new Date(before), -1) : undefined
  const selected: DateRange | undefined =
    from || to ? { from, to } : undefined

  const fmt = (d: Date) => d.toLocaleDateString(lang)
  let label: string | undefined
  if (from && to) {
    label =
      fmt(from) === fmt(to) ? fmt(from) : `${fmt(from)} – ${fmt(to)}`
  } else if (from) {
    label = `${fmt(from)} –`
  } else if (to) {
    label = `– ${fmt(to)}`
  }

  const select = (r: DateRange | undefined) => {
    const start = r?.from
    const end = r?.to ?? r?.from
    onChange(
      start ? startOfDay(start).toISOString() : undefined,
      end ? addDays(startOfDay(end), 1).toISOString() : undefined,
    )
  }

  return (
    <div className="flex gap-1">
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            className={cn(
              'flex-1 justify-start font-normal',
              !label && 'text-muted-foreground',
            )}
          >
            <CalendarIcon />
            {label ?? t('history.filter.any')}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto bg-card p-0" align="start">
          <Calendar
            mode="range"
            selected={selected}
            onSelect={select}
            defaultMonth={from ?? to}
            locale={locale}
          />
        </PopoverContent>
      </Popover>
      {label && (
        <Button
          variant="ghost"
          size="icon"
          aria-label={t('common.clear')}
          onClick={() => onChange(undefined, undefined)}
        >
          <XIcon />
        </Button>
      )}
    </div>
  )
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate())
}

function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n)
}
