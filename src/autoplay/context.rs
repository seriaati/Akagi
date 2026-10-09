//! Shared state between the chromium capture backend and the autoplay
//! manager.
//!
//! - `page`: the [`chromiumoxide::page::Page`] handle for the tab where
//!   Majsoul (or another supported platform) is loaded. Written by
//!   `src/capture/chromium/cdp.rs` when it observes a WebSocket whose URL
//!   host matches a known platform. The handle tracks the **tab**, not the
//!   WebSocket: it survives the many short-lived Route-probe / lobby-
//!   reconnect sockets Majsoul opens and closes during a game, and is
//!   cleared only when its owning tab is removed from the page snapshot.
//!   Read by `AutoplayManager` whenever it needs to dispatch input.
//! - `canvas_rect`: cached `getBoundingClientRect()` of the game canvas,
//!   used to translate 16:9-normalised coordinates into CSS pixels.
//!   Filled lazily by the autoplay manager (one `Runtime.evaluate` per
//!   refresh) and invalidated on round transitions.
//!
//! Both fields are populated only when the chromium capture backend is
//! active. The MITM backend leaves the context untouched, so reads return
//! `None` and the manager skips the click.

use chromiumoxide::page::Page;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct AutoplayContext {
    pub page: Arc<RwLock<Option<Page>>>,
    pub canvas_rect: Arc<RwLock<Option<CanvasRect>>>,
    /// Server-granted time budget for the current decision window.
    /// Written by the Majsoul bridge (see `autoplay::budget`), read by
    /// the manager's delay model. Uses a `std::sync::RwLock` (not tokio)
    /// because the writer is the bridge's synchronous `parse()` path.
    pub time_budget: crate::autoplay::budget::SharedTimeBudget,
    /// Counter of the client's own uplink input commands, bumped by the
    /// Majsoul bridge as it parses. The manager takes a ticket before a
    /// click and asks afterwards whether the count moved — the proof that
    /// the click registered (see `autoplay::verify`).
    pub input_watch: crate::autoplay::verify::SharedInputWatch,
    /// Tenhou's hand at tile-index resolution plus its current decision
    /// window, written by the Tenhou bridge (see `autoplay::tenhou_state`).
    /// Read by the Tenhou autoplay planner, which encodes a client frame
    /// rather than synthesising clicks.
    pub tenhou_state: crate::autoplay::tenhou_state::SharedTenhouState,
    /// Frame injection channel for platforms whose client is not a browser
    /// page (Riichi City): the manager sends built wire frames, the MITM
    /// proxy's client→server relay transmits them. The `in_game` gate is
    /// maintained by the Riichi City bridge. See `autoplay::inject`.
    pub inject: crate::autoplay::inject::SharedInjectBus,
}

impl AutoplayContext {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Ceiling on [`CanvasRect::pixel_jittered`]'s offset, in 16:9 grid units.
/// A hand tile is ~0.79 wide, and the result-screen 確認 is the shortest
/// target at a little over 0.4 tall, so 0.2 cannot leave either.
pub const MAX_CLICK_JITTER: f64 = 0.2;

/// CSS-pixel bounding rect for the game canvas, as reported by
/// `Element.getBoundingClientRect()`. `(x, y)` is the top-left of the
/// canvas relative to the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CanvasRect {
    /// Translate a 16:9 normalised point (the coordinate system used by
    /// `LOCATION` tables ported from the Python reference) to CSS pixels.
    pub fn pixel(&self, x_norm: f64, y_norm: f64) -> (f64, f64) {
        (
            self.x + (x_norm / 16.0) * self.width,
            self.y + (y_norm / 9.0) * self.height,
        )
    }

    /// [`Self::pixel`], moved up to `jitter` grid units along each axis
    /// (clamped to [`MAX_CLICK_JITTER`]). Each offset is the sum of two
    /// uniform draws, so it peaks at the centre the way aimed presses do
    /// rather than spreading evenly out to the edge.
    pub fn pixel_jittered(
        &self,
        x_norm: f64,
        y_norm: f64,
        jitter: f64,
        rng: &mut impl Rng,
    ) -> (f64, f64) {
        let r = jitter.clamp(0.0, MAX_CLICK_JITTER);
        let mut offset = || (rng.random::<f64>() + rng.random::<f64>() - 1.0) * r;
        let dx = offset();
        let dy = offset();
        self.pixel(x_norm + dx, y_norm + dy)
    }

    /// Sanity check for a normalised point — clamps off-canvas requests
    /// before we hand them to CDP.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_translation_centre() {
        let rect = CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        assert_eq!(rect.pixel(8.0, 4.5), (800.0, 450.0));
    }

    #[test]
    fn pixel_translation_with_offset() {
        let rect = CanvasRect {
            x: 100.0,
            y: 50.0,
            width: 1280.0,
            height: 720.0,
        };
        let (px, py) = rect.pixel(8.0, 4.5);
        assert!((px - (100.0 + 640.0)).abs() < 1e-9);
        assert!((py - (50.0 + 360.0)).abs() < 1e-9);
    }

    #[test]
    fn jittered_pixel_stays_within_clamped_radius() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        let rect = CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        let mut rng = StdRng::seed_from_u64(7);
        // 100px per grid unit on both axes at this size.
        let max_px = MAX_CLICK_JITTER * 100.0;
        let mut moved = false;
        for _ in 0..1000 {
            let (px, py) = rect.pixel_jittered(8.0, 4.5, 5.0, &mut rng);
            assert!((px - 800.0).abs() <= max_px + 1e-9);
            assert!((py - 450.0).abs() <= max_px + 1e-9);
            moved |= (px, py) != (800.0, 450.0);
        }
        assert!(moved);
    }

    #[test]
    fn zero_jitter_is_exact() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        let rect = CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        let mut rng = StdRng::seed_from_u64(7);
        assert_eq!(rect.pixel_jittered(8.0, 4.5, 0.0, &mut rng), (800.0, 450.0));
    }

    #[test]
    fn contains_inside() {
        let rect = CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        assert!(rect.contains(800.0, 450.0));
    }

    #[test]
    fn contains_outside() {
        let rect = CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0,
        };
        assert!(!rect.contains(-1.0, 0.0));
        assert!(!rect.contains(0.0, 1000.0));
    }
}
