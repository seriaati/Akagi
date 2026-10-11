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
//! i.e. over the win screen, and ones after a deal 1.5–8 s after
//! `ActionNewRound`. None arrive after `NotifyGameEndResult` — the result
//! screens have replaced the table by then — so the match-end emote goes
//! out over the last hand's result, which the server announces in the same
//! instant as the end.
//!
//! An emote is never pressed while a decision window for our seat is open:
//! the bot's clicks own the board then. It waits for the window to close,
//! up to [`WINDOW_WAIT`], and is dropped if it does not.

use std::sync::Arc;
use std::time::Duration;

use rand::seq::IndexedRandom;
use rand::Rng;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::autoplay::cdp_input::{dispatch_click, evaluate_canvas_rect, jittered_ms, scroll_up};
use crate::autoplay::context::AutoplayContext;
use crate::config::{AppConfig, EmoteConfig, Platform};
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
/// Pause after opening the panel, before pressing into it: it slides in
/// from the right edge.
const PANEL_OPEN_MS: u32 = 500;
/// The panel reopens where it was last scrolled to, so it is wheeled back
/// to the top before every press. Enough notches for any emote count.
const SCROLL_NOTCHES: u32 = 10;
/// Pause after the wheel, for the list to come to rest.
const SCROLL_SETTLE_MS: u32 = 300;

/// Range of the pause between the trigger and the first press, ms.
const REACTION_MS: std::ops::RangeInclusive<u64> = 1_500..=5_000;
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
                if press(&cfg, &ctx, p.slot).await {
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
    Some(Pending {
        trigger,
        slot,
        due: Instant::now() + Duration::from_millis(rng.random_range(REACTION_MS)),
    })
}

/// Open the panel and press `slot`. Whether both presses went out.
async fn press(cfg: &Arc<RwLock<AppConfig>>, ctx: &AutoplayContext, slot: u32) -> bool {
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
    let Some(page) = ctx.page.read().await.clone() else {
        debug!("emote: no page handle; skipping");
        return false;
    };
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

    if let Err(e) = click(button).await {
        warn!("emote: opening the panel failed: {e:#}");
        return false;
    }
    pause(PANEL_OPEN_MS).await;
    // Over the middle of the grid, so the wheel lands on the list.
    let middle = WHEEL_SLOTS[WHEEL_SLOTS.len() / 2];
    let (x, y) = rect.pixel_jittered(middle.0, middle.1, m.click_jitter, &mut rand::rng());
    if let Err(e) = scroll_up(&page, x, y, SCROLL_NOTCHES, m.cursor_glide).await {
        warn!("emote: scrolling the panel failed: {e:#}");
        return false;
    }
    pause(SCROLL_SETTLE_MS).await;
    if let Err(e) = click(target).await {
        warn!("emote: press failed: {e:#}");
        return false;
    }
    true
}

/// The bridge holds a time budget exactly while a decision window for our
/// seat is open.
async fn wait_for_window_closed(ctx: &AutoplayContext) -> bool {
    let started = Instant::now();
    loop {
        if ctx.time_budget.read().ok().and_then(|g| *g).is_none() {
            return true;
        }
        if started.elapsed() > WINDOW_WAIT {
            return false;
        }
        tokio::time::sleep(WINDOW_POLL).await;
    }
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
        for _ in 0..200 {
            let before = Instant::now();
            let p = schedule(Trigger::Win, &cfg, None, None, &mut rng).unwrap();
            let wait = p.due - before;
            assert!(wait >= Duration::from_millis(*REACTION_MS.start()));
            assert!(wait <= Duration::from_millis(*REACTION_MS.end() + 50));
        }
    }
}
