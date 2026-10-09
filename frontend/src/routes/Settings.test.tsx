import { describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'

import { Settings } from './Settings'
import { useConfigStore } from '@/stores/configStore'
import type { AppConfig } from '@/types'

// Regression cover for the auto-save flip-flop: `update_config` emits
// `overlay-config` mid-call, which patched the store while it still held the
// pre-save config. The draft synced from that, reverted the edit, and
// auto-saved the old value back — looping forever (MITM <-> hybrid, each
// swap restarting capture and killing the Chromium window).

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
  initReactI18next: { type: '3rdParty', init: () => {} },
}))

const invoke = vi.fn()
vi.mock('@/lib/tauri', () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
  HAS_TAURI: false,
  listen: () => Promise.resolve(() => {}),
}))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.0.0') }))

function makeConfig(): AppConfig {
  return {
    general: { first_run_completed: true, developer_mode: false },
    logging: { dir: '/old/logs', level: 'info', all_level: 'warn' },
    platform: { kind: 'Majsoul' },
    proxy: { enabled: true, addr: '127.0.0.1:23410', ca_dir: '', block_telemetry: true, unlock_cosmetics: false },
    bot: {
      enabled: true,
      active_4p: 'akagi-native',
      active_3p: 'akagi-native3p',
      auto_sync: false,
      dir: '',
      api: {
        enabled: false,
        base_url: '',
        key: '',
        model_4p: '',
        model_3p: '',
        proxy_enabled: false,
        proxy: '',
        react_timeout_ms: 3000,
      },
    },
    capture: {
      mode: 'mitm',
      chromium: {
        executable: '',
        user_data_dir: '',
        start_url: 'https://game.maj-soul.com/1/',
        cft_channel: 'stable',
        force_cft: false,
        extra_args: [],
      },
    },
    autoplay: {
      enabled: false,
      majsoul: {
        pre_click_delay_min_ms: 0,
        pre_click_delay_max_ms: 0,
        inter_click_delay_ms: 0,
        hover_delay_ms: 0,
        click_hold_ms: 0,
        click_jitter: 0,
        click_timing_jitter_ms: 0,
        verify_input_ms: 0,
        click_retries: 0,
        reload_after_failures: 0,
        dealer_first_discard_extra_delay_ms: 0,
        auto_rematch: false,
        auto_rematch_limit: 0,
      },
      delay: {
        mode: 'legacy',
        min_delay_ms: 0,
        min_button_delay_ms: 0,
        distribution: 'uniform',
        lognormal: {},
        bank_on_long_thought: false,
        riichi_extra_ms: 0,
        kan_extra_ms: 0,
        safety_margin_ms: 0,
        bank_use_fraction: 0,
        bank_max_single_ms: 0,
        no_budget_cap_ms: 0,
      },
    },
    overlay: { enabled: true, top_n: 3, opacity: 1, always_on_top: true },
    network: { github_mirror_mode: 'auto', github_custom_mirror: '' },
    discord: { enabled: false, client_id: '' },
  }
}

describe('Settings auto-save', () => {
  it('does not revert an edit when the backend patches the store mid-save', async () => {
    useConfigStore.getState().setConfig(makeConfig())
    invoke.mockImplementation(async (cmd: string, args?: { newConfig: AppConfig }) => {
      if (cmd !== 'update_config') return null
      // What `overlay::reconcile` does via the bridge's `overlay-config`
      // listener, before the command resolves.
      useConfigStore.getState().setOverlay(args!.newConfig.overlay)
      return null
    })

    render(
      <MemoryRouter>
        <Settings />
      </MemoryRouter>,
    )
    fireEvent.change(screen.getByDisplayValue('/old/logs'), { target: { value: '/new/logs' } })

    const saves = () => invoke.mock.calls.filter(([cmd]) => cmd === 'update_config')
    await waitFor(() => expect(saves()).toHaveLength(1), { timeout: 2000 })
    // Give a reverted draft time to trip the 600ms debounce again.
    await act(() => new Promise((r) => setTimeout(r, 1500)))

    expect(saves()).toHaveLength(1)
    expect(saves()[0][1].newConfig.logging.dir).toBe('/new/logs')
    expect(useConfigStore.getState().config!.logging.dir).toBe('/new/logs')
    expect(screen.getByDisplayValue('/new/logs')).toBeTruthy()
  })
})
