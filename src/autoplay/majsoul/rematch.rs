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

use crate::autoplay::cdp_input::{capture_rgb, dispatch_click, evaluate_canvas_rect};
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
}

fn is_confirm_yellow([r, g, b]: [u8; 3]) -> bool {
    r > 170 && g > 120 && i16::from(r) - i16::from(b) > 90
}

fn is_play_again_blue([r, _, b]: [u8; 3]) -> bool {
    b > 110 && i16::from(b) - i16::from(r) > 50
}

fn coverage(pixels: &[[u8; 3]], colour: fn([u8; 3]) -> bool) -> f64 {
    if pixels.is_empty() {
        return 0.0;
    }
    pixels.iter().filter(|p| colour(**p)).count() as f64 / pixels.len() as f64
}

fn classify(confirm: f64, play_again: f64, prompt_confirm: f64, prompt_cancel: f64) -> Screen {
    if prompt_confirm >= MIN_COVERAGE && prompt_cancel >= MIN_COVERAGE {
        return Screen::Prompt;
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
    let (mut confirms, mut play_agains, mut prompts) = (0u32, 0u32, 0u32);
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
                "auto-rematch: no rematch after {}s (確認 pressed {confirms}x, 再來一場 {play_agains}x, prompt 確認 {prompts}x); giving up",
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
        let (button, label) = match screen {
            Screen::Prompt if prompts < MAX_PROMPT_CONFIRMS => (&PROMPT_CONFIRM, "prompt 確認"),
            Screen::PlayAgain if play_agains < MAX_PLAY_AGAIN => (&PLAY_AGAIN, "再來一場"),
            Screen::Confirm if confirms < MAX_CONFIRMS => (&CONFIRM, "確認"),
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
        let (x, y) = rect.pixel(button.centre.0, button.centre.1);
        info!("auto-rematch: pressing {label}");
        if let Err(e) = dispatch_click(&page, x, y, hover, hold).await {
            warn!("auto-rematch: {label} press failed: {e:#}");
            continue;
        }
        match screen {
            Screen::Prompt => prompts += 1,
            Screen::PlayAgain => play_agains += 1,
            _ => confirms += 1,
        }
        // The press should change the screen; whatever shows next has to
        // prove itself stable again.
        last = None;
    }
}

async fn read_screen(page: &Page) -> anyhow::Result<(Screen, CanvasRect)> {
    let rect = evaluate_canvas_rect(page).await?;
    let confirm = sample(page, &rect, &CONFIRM).await?;
    let play_again = sample(page, &rect, &PLAY_AGAIN).await?;
    let prompt_confirm = sample(page, &rect, &PROMPT_CONFIRM).await?;
    let prompt_cancel = sample(page, &rect, &PROMPT_CANCEL).await?;
    Ok((
        classify(confirm, play_again, prompt_confirm, prompt_cancel),
        rect,
    ))
}

async fn sample(page: &Page, rect: &CanvasRect, button: &Button) -> anyhow::Result<f64> {
    let (x0, y0, x1, y1) = button.sample;
    let (left, top) = rect.pixel(x0, y0);
    let (right, bottom) = rect.pixel(x1, y1);
    let pixels = capture_rgb(page, left, top, right - left, bottom - top).await?;
    Ok(coverage(&pixels, button.colour))
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
        assert_eq!(classify(0.88, 0.0, 0.0, 0.0), Screen::Confirm);
        assert_eq!(classify(0.88, 0.75, 0.0, 0.0), Screen::PlayAgain);
        assert_eq!(classify(0.13, 0.0, 0.0, 0.0), Screen::Busy);
        assert_eq!(classify(0.0, 0.75, 0.0, 0.0), Screen::Busy);
        assert_eq!(classify(0.0, 0.0, 0.84, 0.81), Screen::Prompt);
        assert_eq!(classify(0.0, 0.0, 0.84, 0.0), Screen::Busy);
        assert_eq!(classify(0.0, 0.0, 0.0, 0.81), Screen::Busy);
    }
}
