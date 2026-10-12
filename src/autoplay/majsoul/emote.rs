//! Majsoul auto-emote: press an emote on the table's emote panel when
//! something happens to us — a win, a deal-in, our riichi, the first hand
//! of a match, the match's end. Opt-in via `majsoul.emote`.
//!
//! Clicked rather than sent: the client is Unity WebGL and exposes no
//! script API, and a click makes the client send `broadcastInGame` itself,
//! under its own message index, exactly as a player's would. It also keeps
//! the cosmetic unlock's handling of an unowned character's emotes (shown
//! at our seat only) in play, since that rewrites the client's request.
//!
//! Timing follows what players do, measured from captured tables: emotes
//! after a win land 1.5–5.1 s after `ActionHule` (p10–p90, median 3.5 s),
//! and ones after a deal 1.5–8 s after `ActionNewRound`. None arrive after
//! `NotifyGameEndResult` — the result screens have replaced the table by
//! then — so the match-end emote goes out over the last hand's result,
//! which the server announces in the same instant as the end.
//!
//! A win, a deal-in and the match's end are followed by the hand's result
//! screen, which swallows presses: ours landed up to 4.0 s after
//! `ActionHule` and were lost from 4.1 s on. Those emotes start at once
//! and must press their slot by [`PRESS_BY`], or are dropped.
//!
//! An emote is never pressed while a decision window for our seat is open:
//! the bot's clicks own the board then. It waits for the window to close,
//! up to [`WINDOW_WAIT`], and is dropped if it does not. Once started it
//! holds `ctx.input`, so no bot press can land mid-sequence, and it checks
//! between steps whether the window opened after all — if so it closes the
//! panel and gives the board back rather than finishing. The panel's state
//! is read off a screenshot, not assumed, so one left open (by us or by
//! hand) is used as is and closed afterwards.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::page::Page;

use rand::seq::IndexedRandom;
use rand::Rng;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::autoplay::cdp_input::{
    capture_viewport, dispatch_click, evaluate_canvas_rect, jittered_ms, scroll_up,
};
use crate::autoplay::context::{AutoplayContext, CanvasRect};
use crate::autoplay::majsoul::rematch::{coverage, region};
use crate::config::{AppConfig, EmoteConfig, MajsoulAutoplayConfig, Platform};
use crate::event_bus::MjaiBus;
use crate::schema::{GameEndReason, MjaiEvent};

/// The smiley button on the right edge that opens the emote panel,
/// 16:9-normalised. Measured from a live table (canvas 1683×942).
const WHEEL_BUTTON: Option<(f64, f64)> = Some((15.63, 5.04));
/// The panel's slots when scrolled to the top: a 3-wide grid, read left
/// to right, top to bottom. Measured from a live panel (canvas 1689×951). The panel
/// scrolls when a character has more than nine emotes, but where the later
/// rows land after a scroll depends on how many there are, so only the
/// first page is pressed.
const WHEEL_SLOTS: &[(f64, f64)] = &[
    (12.36, 3.65),
    (13.69, 3.65),
    (15.03, 3.65),
    (12.36, 4.97),
    (13.69, 4.97),
    (15.03, 4.97),
    (12.36, 6.29),
    (13.69, 6.29),
    (15.03, 6.29),
];
/// The panel slides in from the right edge and back out; this long covers
/// either.
const PANEL_SLIDE_MS: u32 = 500;
/// The open panel's collapse tab, on its left edge: pressed to close the
/// panel, and sampled (inset from its edges) to tell whether it is open.
/// Measured from a live panel (canvas 1689×951).
const PANEL_TAB: (f64, f64) = (11.3, 4.95);
const PANEL_TAB_SAMPLE: (f64, f64, f64, f64) = (11.2, 4.5, 11.42, 5.4);
/// Share of the sample that must be the tab's navy, and its arrow's
/// orange. Measured: ~0.83 and ~0.13 open, 0 and 0 closed.
const MIN_TAB_NAVY: f64 = 0.5;
const MIN_TAB_ORANGE: f64 = 0.03;
/// The panel reopens where it was last scrolled to, so it is wheeled back
/// to the top before every press. Enough notches for any emote count.
const SCROLL_NOTCHES: u32 = 10;
/// Pause after the wheel, for the list to come to rest.
const SCROLL_SETTLE_MS: u32 = 300;

/// Range of the pause between the trigger and the first press, ms.
const REACTION_MS: std::ops::RangeInclusive<u64> = 1_500..=5_000;
/// The same, before a result screen: the sequence itself takes most of
/// the time players take, so it starts almost at once.
const QUICK_REACTION_MS: std::ops::RangeInclusive<u64> = 200..=700;
/// Before a result screen, the slot's press must start this soon after
/// the trigger: it lands ~0.55 s later, and presses landing past 4.0 s
/// were swallowed.
const PRESS_BY: Duration = Duration::from_millis(3_400);
/// From pressing the panel's button to pressing a slot, at the default
/// timings: the press, the slide, a screenshot, the wheel and its settle.
/// A sequence that cannot fit before [`PRESS_BY`] is not started.
const OPEN_TO_PRESS: Duration = Duration::from_millis(2_300);
/// How long a due emote may wait for our decision window to close.
const WINDOW_WAIT: Duration = Duration::from_secs(10);
const WINDOW_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trigger {
    Win,
    DealIn,
    Riichi,
    MatchStart,
    MatchEnd,
}

impl Trigger {
    /// Whether the hand's result screen follows within seconds.
    fn before_result_screen(self) -> bool {
        matches!(self, Self::Win | Self::DealIn | Self::MatchEnd)
    }

    fn slots(self, cfg: &EmoteConfig) -> &[u32] {
        match self {
            Self::Win => &cfg.on_win,
            Self::DealIn => &cfg.on_deal_in,
            Self::Riichi => &cfg.on_riichi,
            Self::MatchStart => &cfg.on_match_start,
            Self::MatchEnd => &cfg.on_match_end,
        }
    }
}

/// Per-game view of the event stream: which events are triggers for us.
#[derive(Default)]
struct Game {
    seat: Option<u8>,
    /// Set by `start_game`, cleared by the first `start_kyoku` after it.
    awaiting_first_hand: bool,
}

impl Game {
    fn observe(&mut self, ev: &MjaiEvent) -> Option<Trigger> {
        match ev {
            MjaiEvent::StartGame { id, .. } => {
                *self = Self {
                    seat: *id,
                    awaiting_first_hand: true,
                };
                None
            }
            MjaiEvent::StartKyoku { .. } if self.awaiting_first_hand => {
                self.awaiting_first_hand = false;
                Some(Trigger::MatchStart)
            }
            MjaiEvent::ReachAccepted { actor } if Some(*actor) == self.seat => {
                Some(Trigger::Riichi)
            }
            MjaiEvent::Hora { actor, .. } if Some(*actor) == self.seat => Some(Trigger::Win),
            MjaiEvent::Hora { target, .. } if Some(*target) == self.seat => Some(Trigger::DealIn),
            MjaiEvent::EndGame {
                reason: GameEndReason::Confirmed,
                ..
            } => Some(Trigger::MatchEnd),
            _ => None,
        }
    }
}

struct Pending {
    trigger: Trigger,
    slot: u32,
    due: Instant,
    /// When the slot's press must start by, if a result screen is coming.
    press_by: Option<Instant>,
}

async fn emote_config(cfg: &Arc<RwLock<AppConfig>>) -> Option<EmoteConfig> {
    let guard = cfg.read().await;
    (guard.autoplay.enabled
        && guard.autoplay.majsoul.emote.enabled
        && guard.platform.kind == Platform::Majsoul)
        .then(|| guard.autoplay.majsoul.emote.clone())
}

/// Long-lived: watches the mjai stream and presses at most one emote per
/// trigger.
pub async fn emote_watcher(cfg: Arc<RwLock<AppConfig>>, ctx: Arc<AutoplayContext>, bus: MjaiBus) {
    let mut rx = bus.subscribe();
    let mut game = Game::default();
    let mut pending: Option<Pending> = None;
    let mut last_sent: Option<Instant> = None;
    loop {
        let due = pending.as_ref().map(|p| p.due);
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(ev) => {
                    match &ev {
                        // The table this emote was meant for is gone.
                        MjaiEvent::StartGame { .. }
                        | MjaiEvent::EndGame { reason: GameEndReason::Terminated, .. } => {
                            pending = None;
                        }
                        _ => {}
                    }
                    let Some(trigger) = game.observe(&ev) else { continue };
                    let Some(emote) = emote_config(&cfg).await else { continue };
                    let mut rng = rand::rng();
                    if let Some(p) = schedule(trigger, &emote, pending.as_ref(), last_sent, &mut rng) {
                        debug!("emote: {trigger:?} → slot {} in {:?}", p.slot, p.due - Instant::now());
                        pending = Some(p);
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return,
            },
            _ = tokio::time::sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                let Some(p) = pending.take() else { continue };
                if emote_config(&cfg).await.is_none() {
                    continue;
                }
                if press(&cfg, &ctx, p.slot, p.press_by).await {
                    info!("emote: sent slot {} for {:?}", p.slot, p.trigger);
                    last_sent = Some(Instant::now());
                }
            }
        }
    }
}

/// Whether `trigger` gets an emote, and which. The match's end replaces an
/// emote still waiting from the same last hand; anything else leaves a
/// waiting one alone.
fn schedule(
    trigger: Trigger,
    cfg: &EmoteConfig,
    pending: Option<&Pending>,
    last_sent: Option<Instant>,
    rng: &mut impl Rng,
) -> Option<Pending> {
    let &slot = trigger.slots(cfg).choose(rng)?;
    if pending.is_some() && trigger != Trigger::MatchEnd {
        return None;
    }
    let gap = Duration::from_secs(cfg.min_gap_secs.into());
    if last_sent.is_some_and(|at| at.elapsed() < gap) {
        return None;
    }
    if !rng.random_bool(cfg.chance.clamp(0.0, 1.0)) {
        return None;
    }
    let now = Instant::now();
    let (reaction, press_by) = if trigger.before_result_screen() {
        (QUICK_REACTION_MS, Some(now + PRESS_BY))
    } else {
        (REACTION_MS, None)
    };
    Some(Pending {
        trigger,
        slot,
        due: now + Duration::from_millis(rng.random_range(reaction)),
        press_by,
    })
}

/// Open the panel and press `slot`, starting that press by `press_by`.
/// Whether the emote went out.
///
/// Holds `ctx.input` throughout, so the bot's presses cannot interleave
/// with ours; in return it gives the board back as soon as our decision
/// window opens. Whatever happens, the panel is left closed: an open one
/// covers the right end of the action-button and candidate rows, and hides
/// the button that opens it (whose spot is then a slot). A result screen
/// can hide it from that check, so `ctx.emote_panel_dirty` hands a second
/// look to the bot's next press.
async fn press(
    cfg: &Arc<RwLock<AppConfig>>,
    ctx: &AutoplayContext,
    slot: u32,
    press_by: Option<Instant>,
) -> bool {
    let (Some(button), Some(&target)) = (WHEEL_BUTTON, WHEEL_SLOTS.get(slot as usize)) else {
        warn!(
            "emote: slot {slot} is past the panel's first page (0–{}); skipping",
            WHEEL_SLOTS.len() - 1
        );
        return false;
    };
    if !wait_for_window_closed(ctx).await {
        debug!("emote: our decision window stayed open; dropping it");
        return false;
    }
    // Out of its lock before `input` is taken — never the other way round.
    let Some(page) = ctx.page.read().await.clone() else {
        debug!("emote: no page handle; skipping");
        return false;
    };
    let _input = ctx.input.lock().await;
    // The window may have opened while the bot held the lock.
    if window_open(ctx) {
        debug!("emote: our decision window opened; dropping it");
        return false;
    }
    let rect = match evaluate_canvas_rect(&page).await {
        Ok(rect) => rect,
        Err(e) => {
            debug!("emote: canvas rect query failed: {e:#}");
            return false;
        }
    };
    let m = cfg.read().await.autoplay.majsoul.clone();
    let pause = |base: u32| {
        let ms = jittered_ms(base, m.click_timing_jitter_ms);
        tokio::time::sleep(Duration::from_millis(ms.into()))
    };
    let click = |pos: (f64, f64)| {
        let (x, y) = rect.pixel_jittered(pos.0, pos.1, m.click_jitter, &mut rand::rng());
        let hover = jittered_ms(m.hover_delay_ms, m.click_timing_jitter_ms);
        let hold = jittered_ms(m.click_hold_ms, m.click_timing_jitter_ms);
        dispatch_click(&page, x, y, hover, hold, m.cursor_glide)
    };
    let is_open = || async {
        panel_open(&page, &rect)
            .await
            .inspect_err(|e| debug!("emote: screen read failed: {e:#}"))
            .ok()
    };

    let late = |by: Option<Instant>| by.is_some_and(|by| Instant::now() > by);

    let sent = async {
        match is_open().await {
            Some(true) => {
                debug!("emote: panel already open");
                ctx.emote_panel_dirty.store(true, Ordering::Relaxed);
            }
            Some(false) => {
                if late(press_by.map(|by| by - OPEN_TO_PRESS)) {
                    debug!("emote: too late to beat the result screen; dropping it");
                    return false;
                }
                ctx.emote_panel_dirty.store(true, Ordering::Relaxed);
                if let Err(e) = click(button).await {
                    warn!("emote: opening the panel failed: {e:#}");
                    return false;
                }
                pause(PANEL_SLIDE_MS).await;
                if is_open().await != Some(true) {
                    debug!("emote: the panel did not open; skipping");
                    return false;
                }
            }
            // Pressing blind could land on the board.
            None => return false,
        }
        if window_open(ctx) {
            debug!("emote: our decision window opened; giving the board back");
            return false;
        }
        // Over the middle of the grid, so the wheel lands on the list.
        let middle = WHEEL_SLOTS[WHEEL_SLOTS.len() / 2];
        let (x, y) = rect.pixel_jittered(middle.0, middle.1, m.click_jitter, &mut rand::rng());
        if let Err(e) = scroll_up(&page, x, y, SCROLL_NOTCHES, m.cursor_glide).await {
            warn!("emote: scrolling the panel failed: {e:#}");
            return false;
        }
        pause(SCROLL_SETTLE_MS).await;
        if window_open(ctx) {
            debug!("emote: our decision window opened; giving the board back");
            return false;
        }
        if late(press_by) {
            debug!("emote: the result screen is due; closing the panel instead");
            return false;
        }
        if let Err(e) = click(target).await {
            warn!("emote: press failed: {e:#}");
            return false;
        }
        // Picking an emote closes the panel; let it slide out.
        pause(PANEL_SLIDE_MS).await;
        true
    }
    .await;

    if !close_panel(&page, &rect, &m).await {
        warn!("emote: could not close the emote panel");
    }
    sent
}

/// Close the emote panel if it is showing. Whether it is known to be
/// closed now. Behind a result screen the panel reads as closed, so only a
/// read taken over the table settles it.
pub async fn close_panel(page: &Page, rect: &CanvasRect, m: &MajsoulAutoplayConfig) -> bool {
    let is_open = || async {
        panel_open(page, rect)
            .await
            .inspect_err(|e| debug!("emote: screen read failed: {e:#}"))
            .ok()
    };
    match is_open().await {
        Some(false) => return true,
        Some(true) => {}
        None => return false,
    }
    let (x, y) = rect.pixel_jittered(PANEL_TAB.0, PANEL_TAB.1, m.click_jitter, &mut rand::rng());
    let hover = jittered_ms(m.hover_delay_ms, m.click_timing_jitter_ms);
    let hold = jittered_ms(m.click_hold_ms, m.click_timing_jitter_ms);
    if let Err(e) = dispatch_click(page, x, y, hover, hold, m.cursor_glide).await {
        debug!("emote: closing the panel failed: {e:#}");
        return false;
    }
    let slide = jittered_ms(PANEL_SLIDE_MS, m.click_timing_jitter_ms);
    tokio::time::sleep(Duration::from_millis(slide.into())).await;
    is_open().await == Some(false)
}

/// The bridge holds a time budget exactly while a decision window for our
/// seat is open.
fn window_open(ctx: &AutoplayContext) -> bool {
    ctx.time_budget.read().ok().and_then(|g| *g).is_some()
}

async fn wait_for_window_closed(ctx: &AutoplayContext) -> bool {
    let started = Instant::now();
    loop {
        if !window_open(ctx) {
            return true;
        }
        if started.elapsed() > WINDOW_WAIT {
            return false;
        }
        tokio::time::sleep(WINDOW_POLL).await;
    }
}

/// Whether the emote panel is showing, read off its collapse tab: a navy
/// strip with an orange arrow while open, the table while closed.
async fn panel_open(page: &Page, rect: &CanvasRect) -> anyhow::Result<bool> {
    let frame = capture_viewport(page).await?;
    let pixels = region(&frame, rect, PANEL_TAB_SAMPLE).concat();
    Ok(tab_showing(&pixels))
}

fn tab_showing(pixels: &[[u8; 3]]) -> bool {
    coverage(pixels, is_tab_navy) >= MIN_TAB_NAVY
        && coverage(pixels, is_tab_orange) >= MIN_TAB_ORANGE
}

/// The tab's body, ~(22, 43, 77).
fn is_tab_navy([r, g, b]: [u8; 3]) -> bool {
    let (r, g, b) = (i16::from(r), i16::from(g), i16::from(b));
    b > 55 && b - r > 35 && b - g > 15 && r < 60
}

/// The tab's arrow, ~(222, 146, 55).
fn is_tab_orange([r, g, b]: [u8; 3]) -> bool {
    r > 190 && (110..175).contains(&g) && b < 90
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn start_game(seat: u8) -> MjaiEvent {
        serde_json::from_value(
            serde_json::json!({"type": "start_game", "names": ["a", "b", "c", "d"], "id": seat}),
        )
        .unwrap()
    }

    fn start_kyoku() -> MjaiEvent {
        serde_json::from_value(serde_json::json!({
            "type": "start_kyoku", "bakaze": "E", "dora_marker": "1m", "kyoku": 1,
            "honba": 0, "kyotaku": 0, "oya": 0, "scores": [25000, 25000, 25000, 25000],
            "tehais": vec![vec!["?"; 13]; 4],
        }))
        .unwrap()
    }

    fn hora(actor: u8, target: u8) -> MjaiEvent {
        MjaiEvent::Hora {
            actor,
            target,
            deltas: None,
            ura_markers: None,
        }
    }

    fn cfg_all(slot: u32) -> EmoteConfig {
        EmoteConfig {
            enabled: true,
            chance: 1.0,
            min_gap_secs: 0,
            on_win: vec![slot],
            on_deal_in: vec![slot],
            on_riichi: vec![slot],
            on_match_start: vec![slot],
            on_match_end: vec![slot],
        }
    }

    /// Pixels sampled from the tab area of live screenshots.
    #[test]
    fn tab_reads_open_only_with_navy_and_arrow() {
        let navy = [22, 43, 77];
        let orange = [222, 146, 55];
        let table = [[70, 56, 66], [89, 60, 66], [68, 53, 65]];
        assert!(is_tab_navy(navy) && is_tab_orange(orange));
        for p in table {
            assert!(!is_tab_navy(p) && !is_tab_orange(p));
        }
        let mut open = vec![navy; 85];
        open.extend([orange; 13]);
        open.extend([[43, 54, 74]; 2]);
        assert!(tab_showing(&open));
        assert!(!tab_showing(&[table[0]; 100]));
        // Navy alone (a blue table cloth) is not the tab.
        assert!(!tab_showing(&[navy; 100]));
    }

    #[test]
    fn events_map_to_triggers_from_our_seat() {
        let mut game = Game::default();
        assert_eq!(game.observe(&start_game(1)), None);
        assert_eq!(game.observe(&start_kyoku()), Some(Trigger::MatchStart));
        // Only the first hand of the match.
        assert_eq!(game.observe(&start_kyoku()), None);
        assert_eq!(
            game.observe(&MjaiEvent::ReachAccepted { actor: 1 }),
            Some(Trigger::Riichi)
        );
        assert_eq!(game.observe(&MjaiEvent::ReachAccepted { actor: 2 }), None);
        assert_eq!(game.observe(&hora(1, 3)), Some(Trigger::Win));
        assert_eq!(game.observe(&hora(1, 1)), Some(Trigger::Win));
        assert_eq!(game.observe(&hora(2, 1)), Some(Trigger::DealIn));
        // Someone else's tsumo, or a ron on someone else.
        assert_eq!(game.observe(&hora(2, 2)), None);
        assert_eq!(game.observe(&hora(2, 3)), None);
        assert_eq!(
            game.observe(&MjaiEvent::confirmed_game(None, None)),
            Some(Trigger::MatchEnd)
        );
        assert_eq!(game.observe(&MjaiEvent::terminated_game()), None);
    }

    #[test]
    fn no_seat_no_personal_triggers() {
        let mut game = Game::default();
        assert_eq!(game.observe(&hora(0, 1)), None);
        assert_eq!(game.observe(&MjaiEvent::ReachAccepted { actor: 0 }), None);
    }

    #[test]
    fn empty_list_leaves_the_trigger_off() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut cfg = cfg_all(3);
        cfg.on_riichi.clear();
        assert!(schedule(Trigger::Riichi, &cfg, None, None, &mut rng).is_none());
        let p = schedule(Trigger::Win, &cfg, None, None, &mut rng).unwrap();
        assert_eq!(p.slot, 3);
    }

    /// Emotes before a result screen must leave room for the whole
    /// sequence before its deadline; others have no deadline.
    #[tokio::test]
    async fn result_screen_triggers_start_in_time_for_their_deadline() {
        let mut rng = StdRng::seed_from_u64(1);
        let cfg = cfg_all(0);
        for trigger in [Trigger::Win, Trigger::DealIn, Trigger::MatchEnd] {
            for _ in 0..50 {
                let p = schedule(trigger, &cfg, None, None, &mut rng).unwrap();
                let by = p.press_by.expect("a deadline");
                assert!(p.due + OPEN_TO_PRESS < by);
            }
        }
        for trigger in [Trigger::Riichi, Trigger::MatchStart] {
            let p = schedule(trigger, &cfg, None, None, &mut rng).unwrap();
            assert!(p.press_by.is_none());
        }
    }

    #[test]
    fn chance_zero_never_sends() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut cfg = cfg_all(0);
        cfg.chance = 0.0;
        for _ in 0..100 {
            assert!(schedule(Trigger::Win, &cfg, None, None, &mut rng).is_none());
        }
    }

    #[tokio::test]
    async fn gap_blocks_a_trigger_right_after_an_emote() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut cfg = cfg_all(0);
        cfg.min_gap_secs = 20;
        let just_now = Some(Instant::now());
        assert!(schedule(Trigger::Win, &cfg, None, just_now, &mut rng).is_none());
        cfg.min_gap_secs = 0;
        assert!(schedule(Trigger::Win, &cfg, None, just_now, &mut rng).is_some());
    }

    #[tokio::test]
    async fn match_end_replaces_a_waiting_emote_and_nothing_else_does() {
        let mut rng = StdRng::seed_from_u64(1);
        let cfg = cfg_all(0);
        let waiting = schedule(Trigger::Win, &cfg, None, None, &mut rng).unwrap();
        assert!(schedule(Trigger::DealIn, &cfg, Some(&waiting), None, &mut rng).is_none());
        let end = schedule(Trigger::MatchEnd, &cfg, Some(&waiting), None, &mut rng).unwrap();
        assert_eq!(end.trigger, Trigger::MatchEnd);
    }

    #[tokio::test]
    async fn reaction_lands_inside_the_measured_range() {
        let mut rng = StdRng::seed_from_u64(1);
        let cfg = cfg_all(0);
        for (trigger, range) in [
            (Trigger::Riichi, REACTION_MS),
            (Trigger::Win, QUICK_REACTION_MS),
        ] {
            for _ in 0..200 {
                let before = Instant::now();
                let p = schedule(trigger, &cfg, None, None, &mut rng).unwrap();
                let wait = p.due - before;
                assert!(wait >= Duration::from_millis(*range.start()));
                assert!(wait <= Duration::from_millis(*range.end() + 50));
            }
        }
    }
}
