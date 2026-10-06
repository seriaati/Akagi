//! Majsoul auto-rematch: once a match ends normally, click through the
//! result screens and press 再來一場, which queues the mode just played
//! again (the client sends `.lq.Lobby.startUnifiedMatch` with the same
//! `match_sid`).
//!
//! The end sequence is three screens, each with 確認 in the same spot; only
//! the last also carries 再來一場, to its left. 再來一場 opens a prompt
//! naming the mode, whose own 確認 is what actually queues. Pressing 確認 on that last
//! screen leaves for the lobby, so timing alone cannot drive this — one
//! press too many and the rematch is lost. Instead the watcher reads the
//! two button areas off a screenshot of the canvas and presses what is
//! actually showing:
//!
//! - the prompt's 確認 and 取消 visible → press its 確認. Done once the
//!   client's queue request is seen.
//! - 確認 and 再來一場 both visible → press 再來一場.
//! - only 確認 visible → press it, at most [`MAX_CONFIRMS`] times: the two
//!   screens before the last. A third press could only be the final
//!   screen's 確認.
//! - the reward screen (獲得獎勵, a band across the screen between two
//!   gold rules) → press inside the band to dismiss it. It can come up
//!   before the result screens, and has no button of its own.
//! - neither → still animating; wait.
//!
//! A screen must read the same on two polls in a row, and again after the
//! pre-press pause, before anything is pressed — a button caught mid-fade
//! is not acted on.
//!
//! `majsoul.auto_rematch_limit` caps a run of back-to-back matches: the
//! match the run started from counts as the first, and once the limit is
//! reached the result screens are left alone. A run ends — and the count
//! starts over with the next match — at the limit, when a rematch fails,
//! or when a match ends with auto-rematch off.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chromiumoxide::page::Page;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::autoplay::cdp_input::{capture_viewport, dispatch_click, evaluate_canvas_rect, Frame};
use crate::autoplay::context::{AutoplayContext, CanvasRect};
use crate::config::{AppConfig, Platform};
use crate::event_bus::MjaiBus;
use crate::schema::{GameEndReason, MjaiEvent};

/// A result-screen button: where to press it, and the area sampled to tell
/// whether it is showing (inset from its edges, 16:9-normalised).
struct Button {
    centre: (f64, f64),
    sample: (f64, f64, f64, f64),
    colour: fn([u8; 3]) -> bool,
}

/// Measured from live result screens (canvas 913×513).
const CONFIRM: Button = Button {
    centre: (14.53, 8.21),
    sample: (13.7, 8.0, 15.4, 8.42),
    colour: is_confirm_yellow,
};
const PLAY_AGAIN: Button = Button {
    centre: (12.16, 8.21),
    sample: (11.35, 8.0, 12.95, 8.42),
    colour: is_play_again_blue,
};
/// The 再來一場 prompt's buttons, measured from a live prompt (canvas
/// 910×512). The dialog dims the screen behind it, so the buttons above
/// no longer read while it is up.
const PROMPT_CONFIRM: Button = Button {
    centre: (6.48, 6.61),
    sample: (5.5, 6.5, 7.25, 6.75),
    colour: is_confirm_yellow,
};
const PROMPT_CANCEL: Button = Button {
    centre: (9.44, 6.59),
    sample: (8.5, 6.5, 10.2, 6.75),
    colour: is_play_again_blue,
};

/// The reward screen's band is edged by two 1px gold rules running the
/// full width, at these heights (16:9-normalised, measured from a live
/// reward screen, canvas 909×515). Each is looked for in a strip this tall
/// around it, so a line a pixel or two off still lands inside.
const REWARD_RULES: [f64; 2] = [2.71, 6.06];
const REWARD_RULE_SLACK: f64 = 0.1;
/// Where the reward screen is pressed to dismiss it: inside the band,
/// right of the rewards and below the chest's progress bar.
const REWARD_DISMISS: (f64, f64) = (11.5, 5.6);
/// Share of a row that must be rule gold to count as the rule. Measured:
/// 1.0 on both rules; no other row of the reward screen passes ~0.26.
const MIN_RULE_COVERAGE: f64 = 0.8;

/// Share of a sample area that must match the button's colour. Measured:
/// ~0.88 for 確認, ~0.75 for 再來一場, and ~0.84 / ~0.81 for the prompt's
/// 確認 / 取消 when shown; the result-screen art
/// behind them reaches ~0.13 at most.
const MIN_COVERAGE: f64 = 0.5;
/// 確認 presses allowed per match end — one per screen before the last.
const MAX_CONFIRMS: u32 = 2;
/// 再來一場 presses allowed before giving up on a client that ignores them.
const MAX_PLAY_AGAIN: u32 = 3;
/// Prompt 確認 presses allowed, likewise.
const MAX_PROMPT_CONFIRMS: u32 = 3;
/// Reward screen presses allowed; more than one reward can be shown.
const MAX_REWARD_DISMISSALS: u32 = 5;
const POLL: Duration = Duration::from_secs(1);
/// The end screens take well under this even when read slowly by hand.
const GIVE_UP_AFTER: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Neither button showing: animating, or not a result screen.
    Busy,
    /// 確認 alone: a screen before the last.
    Confirm,
    /// 確認 with 再來一場 beside it: the last screen.
    PlayAgain,
    /// The prompt 再來一場 opens, 確認 and 取消 side by side.
    Prompt,
    /// 獲得獎勵: rewards shown in a band across the screen.
    Reward,
}

fn is_confirm_yellow([r, g, b]: [u8; 3]) -> bool {
    r > 170 && g > 120 && i16::from(r) - i16::from(b) > 90
}

fn is_play_again_blue([r, _, b]: [u8; 3]) -> bool {
    b > 110 && i16::from(b) - i16::from(r) > 50
}

/// The reward band's rules, ~(166, 157, 105).
fn is_rule_gold([r, g, b]: [u8; 3]) -> bool {
    r > 140 && g > 110 && b < 140 && i16::from(r) - i16::from(b) > 40
}

fn coverage(pixels: &[[u8; 3]], colour: fn([u8; 3]) -> bool) -> f64 {
    if pixels.is_empty() {
        return 0.0;
    }
    pixels.iter().filter(|p| colour(**p)).count() as f64 / pixels.len() as f64
}

fn classify(
    confirm: f64,
    play_again: f64,
    prompt_confirm: f64,
    prompt_cancel: f64,
    reward_rules: [f64; 2],
) -> Screen {
    if prompt_confirm >= MIN_COVERAGE && prompt_cancel >= MIN_COVERAGE {
        return Screen::Prompt;
    }
    if reward_rules.iter().all(|&c| c >= MIN_RULE_COVERAGE) {
        return Screen::Reward;
    }
    match (confirm >= MIN_COVERAGE, play_again >= MIN_COVERAGE) {
        (true, true) => Screen::PlayAgain,
        (true, false) => Screen::Confirm,
        _ => Screen::Busy,
    }
}

async fn enabled(cfg: &Arc<RwLock<AppConfig>>) -> bool {
    let guard = cfg.read().await;
    guard.autoplay.enabled
        && guard.autoplay.majsoul.auto_rematch
        && guard.platform.kind == Platform::Majsoul
}

/// Long-lived: one click-through per normally-ended match. A terminated
/// match (disconnect, abandoned) never rematches.
pub async fn rematch_watcher(cfg: Arc<RwLock<AppConfig>>, ctx: Arc<AutoplayContext>, bus: MjaiBus) {
    let mut rx = bus.subscribe();
    // Matches finished in the current run of rematches.
    let mut played = 0u32;
    loop {
        match rx.recv().await {
            Ok(MjaiEvent::EndGame {
                reason: GameEndReason::Confirmed,
                ..
            }) => {
                if !enabled(&cfg).await {
                    played = 0;
                    continue;
                }
                played += 1;
                let limit = cfg.read().await.autoplay.majsoul.auto_rematch_limit;
                if limit_reached(played, limit) {
                    info!("auto-rematch: {played} of {limit} matches played, not queueing again");
                    played = 0;
                    continue;
                }
                if !click_through(&cfg, &ctx).await {
                    played = 0;
                }
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// `0` is no limit.
fn limit_reached(played: u32, limit: u32) -> bool {
    limit > 0 && played >= limit
}

/// Whether the client ended up queued for another match.
async fn click_through(cfg: &Arc<RwLock<AppConfig>>, ctx: &AutoplayContext) -> bool {
    info!("auto-rematch: match over, watching the result screens");
    let queued_before = ctx.input_watch.match_requests();
    let started = Instant::now();
    let (mut confirms, mut play_agains, mut prompts, mut rewards) = (0u32, 0u32, 0u32, 0u32);
    let mut last: Option<Screen> = None;
    loop {
        tokio::time::sleep(POLL).await;
        // Also catches the user pressing 再來一場 themselves.
        if ctx.input_watch.match_requests() != queued_before {
            info!("auto-rematch: queued for the next match");
            return true;
        }
        if !enabled(cfg).await {
            info!("auto-rematch: switched off, leaving the result screens alone");
            return false;
        }
        if started.elapsed() > GIVE_UP_AFTER {
            warn!(
                "auto-rematch: no rematch after {}s (確認 pressed {confirms}x, 再來一場 {play_agains}x, prompt 確認 {prompts}x, rewards {rewards}x); giving up",
                GIVE_UP_AFTER.as_secs()
            );
            return false;
        }
        let Some(page) = ctx.page.read().await.clone() else {
            continue;
        };
        let screen = match read_screen(&page).await {
            Ok((screen, _)) => screen,
            Err(e) => {
                debug!("auto-rematch: screen read failed: {e:#}");
                last = None;
                continue;
            }
        };
        if last.replace(screen) != Some(screen) {
            continue;
        }
        let (target, label) = match screen {
            Screen::Prompt if prompts < MAX_PROMPT_CONFIRMS => {
                (PROMPT_CONFIRM.centre, "prompt 確認")
            }
            Screen::Reward if rewards < MAX_REWARD_DISMISSALS => {
                (REWARD_DISMISS, "the reward screen")
            }
            Screen::PlayAgain if play_agains < MAX_PLAY_AGAIN => (PLAY_AGAIN.centre, "再來一場"),
            Screen::Confirm if confirms < MAX_CONFIRMS => (CONFIRM.centre, "確認"),
            _ => continue,
        };

        // A reading beat before pressing, then make sure the screen is
        // still the one we decided on.
        let pause = 800 + rand::random::<u64>() % 1_700;
        tokio::time::sleep(Duration::from_millis(pause)).await;
        let rect = match read_screen(&page).await {
            Ok((now, rect)) if now == screen => rect,
            _ => {
                last = None;
                continue;
            }
        };

        let (hover, hold) = {
            let guard = cfg.read().await;
            (
                guard.autoplay.majsoul.hover_delay_ms,
                guard.autoplay.majsoul.click_hold_ms,
            )
        };
        let (x, y) = rect.pixel(target.0, target.1);
        info!("auto-rematch: pressing {label}");
        if let Err(e) = dispatch_click(&page, x, y, hover, hold).await {
            warn!("auto-rematch: {label} press failed: {e:#}");
            continue;
        }
        match screen {
            Screen::Prompt => prompts += 1,
            Screen::PlayAgain => play_agains += 1,
            Screen::Reward => rewards += 1,
            _ => confirms += 1,
        }
        // The press should change the screen; whatever shows next has to
        // prove itself stable again.
        last = None;
    }
}

/// One capture per read, every area sampled from it.
async fn read_screen(page: &Page) -> anyhow::Result<(Screen, CanvasRect)> {
    let rect = evaluate_canvas_rect(page).await?;
    let frame = capture_viewport(page).await?;
    let sample = |button: &Button| {
        let pixels: Vec<_> = region(&frame, &rect, button.sample).concat();
        coverage(&pixels, button.colour)
    };
    let rule = |y: f64| {
        region(
            &frame,
            &rect,
            (1.0, y - REWARD_RULE_SLACK, 15.0, y + REWARD_RULE_SLACK),
        )
        .iter()
        .map(|row| coverage(row, is_rule_gold))
        .fold(0.0, f64::max)
    };
    Ok((
        classify(
            sample(&CONFIRM),
            sample(&PLAY_AGAIN),
            sample(&PROMPT_CONFIRM),
            sample(&PROMPT_CANCEL),
            REWARD_RULES.map(rule),
        ),
        rect,
    ))
}

/// A 16:9-normalised area of the canvas, row by row.
fn region(
    frame: &Frame,
    rect: &CanvasRect,
    (x0, y0, x1, y1): (f64, f64, f64, f64),
) -> Vec<Vec<[u8; 3]>> {
    let (left, top) = rect.pixel(x0, y0);
    let (right, bottom) = rect.pixel(x1, y1);
    frame.region(left, top, right - left, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Colours sampled from live result screens: the button bodies (light
    /// and shaded ends of each gradient) match, and what surrounds them —
    /// the end-screen art, tiles, board, and each other — does not.
    #[test]
    fn button_colours_match_only_their_own_button() {
        let yellow = [[255, 228, 123], [231, 178, 74], [242, 207, 107]];
        let blue = [[57, 85, 154], [41, 69, 117], [64, 85, 123]];
        let background = [[255, 251, 251], [242, 194, 186], [67, 67, 66], [35, 28, 44]];
        for p in yellow {
            assert!(is_confirm_yellow(p), "{p:?}");
            assert!(!is_play_again_blue(p), "{p:?}");
        }
        for p in blue {
            assert!(is_play_again_blue(p), "{p:?}");
            assert!(!is_confirm_yellow(p), "{p:?}");
        }
        for p in background {
            assert!(!is_confirm_yellow(p) && !is_play_again_blue(p), "{p:?}");
        }
    }

    #[test]
    fn coverage_is_the_matching_share() {
        let px = [[255, 228, 123], [255, 228, 123], [35, 28, 44], [35, 28, 44]];
        assert_eq!(coverage(&px, is_confirm_yellow), 0.5);
        assert_eq!(coverage(&[], is_confirm_yellow), 0.0);
    }

    #[test]
    fn limit_counts_the_starting_match() {
        // Limit 3: matches 1 and 2 rematch, the third is the last.
        assert!(!limit_reached(1, 3));
        assert!(!limit_reached(2, 3));
        assert!(limit_reached(3, 3));
        // 0 = no limit.
        assert!(!limit_reached(1_000, 0));
    }

    /// 再來一場 counts only alongside 確認: the last screen shows both, so
    /// blue on its own is not a result screen. The prompt likewise needs
    /// both its buttons, and dims the result-screen buttons behind it.
    #[test]
    fn screens_classify_from_measured_coverage() {
        const NO_RULES: [f64; 2] = [0.0, 0.0];
        assert_eq!(classify(0.88, 0.0, 0.0, 0.0, NO_RULES), Screen::Confirm);
        assert_eq!(classify(0.88, 0.75, 0.0, 0.0, NO_RULES), Screen::PlayAgain);
        assert_eq!(classify(0.13, 0.0, 0.0, 0.0, NO_RULES), Screen::Busy);
        assert_eq!(classify(0.0, 0.75, 0.0, 0.0, NO_RULES), Screen::Busy);
        assert_eq!(classify(0.0, 0.0, 0.84, 0.81, NO_RULES), Screen::Prompt);
        assert_eq!(classify(0.0, 0.0, 0.84, 0.0, NO_RULES), Screen::Busy);
        assert_eq!(classify(0.0, 0.0, 0.0, 0.81, NO_RULES), Screen::Busy);
    }

    /// The reward screen needs both rules; the brightest other row of a
    /// live reward screen reads ~0.26.
    #[test]
    fn reward_screen_needs_both_rules() {
        assert_eq!(classify(0.0, 0.0, 0.0, 0.0, [1.0, 1.0]), Screen::Reward);
        assert_eq!(classify(0.0, 0.0, 0.0, 0.0, [1.0, 0.26]), Screen::Busy);
        assert_eq!(classify(0.0, 0.0, 0.0, 0.0, [0.26, 1.0]), Screen::Busy);
    }

    /// Rule colours sampled from a live reward screen, and the rows
    /// either side of them.
    #[test]
    fn rule_gold_matches_only_the_rules() {
        for p in [[165, 157, 105], [167, 158, 104], [165, 153, 106]] {
            assert!(is_rule_gold(p), "{p:?}");
        }
        for p in [
            [63, 66, 69],
            [69, 69, 57],
            [233, 208, 182],
            [44, 49, 62],
            [15, 22, 35],
        ] {
            assert!(!is_rule_gold(p), "{p:?}");
        }
    }
}
