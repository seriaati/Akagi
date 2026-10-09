//! Thin wrappers around chromiumoxide for the autoplay manager.
//!
//! Centralised so the click + canvas-rect query logic can be unit-mocked
//! and the manager keeps a single dependency on chromiumoxide types.

use crate::autoplay::context::CanvasRect;
use anyhow::{anyhow, Context, Result};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
};
use chromiumoxide::layout::Point;
use chromiumoxide::page::Page;
use rand::Rng;
use std::sync::Mutex;
use std::time::Duration;

/// Where the last press left the cursor, in CSS pixels. CDP cannot be
/// asked where the pointer is, so the path to the next press starts from
/// here. `None` until the first press, which therefore arrives directly.
static LAST_CURSOR: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// Rough mean duration of [`cursor_path`]'s glide between the targets a
/// game presses, for the delay model's click-overhead estimate.
pub const CURSOR_PATH_ESTIMATE_MS: u32 = 250;

/// Interval between the `mouseMoved` events of a glide — about one frame.
const PATH_STEP_MS: f64 = 16.0;

/// `base` plus up to `jitter` ms drawn at random. Never below `base`, so a
/// configured hover or hold stays the floor the client needs.
pub fn jittered_ms(base: u32, jitter: u32) -> u32 {
    base.saturating_add(rand::random_range(0..=jitter))
}

/// The points a glide from `from` to `to` passes through, one per
/// [`PATH_STEP_MS`], ending exactly on `to` (`from` itself is left out).
///
/// The path is a cubic Bézier whose two control points sit off the
/// straight line on the same side, so it bows into an arc rather than
/// wobbling; the bow's size and side are drawn per glide. Progress along
/// it follows the minimum-jerk profile — slow start, fast middle, slow
/// arrival — which is how aimed hand movements are timed. Duration grows
/// with distance after Fitts' law, ±15%.
fn cursor_path(from: (f64, f64), to: (f64, f64), rng: &mut impl Rng) -> Vec<(f64, f64)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let dist = dx.hypot(dy);
    if dist < 2.0 {
        return vec![to];
    }
    let duration_ms = (80.0 + 70.0 * (dist / 40.0 + 1.0).log2()) * rng.random_range(0.85..1.15);
    let steps = ((duration_ms / PATH_STEP_MS).round() as usize).max(2);

    // Unit normal to the straight line; scaling it by `dist` keeps the bow
    // proportional to the move.
    let (nx, ny) = (-dy / dist, dx / dist);
    let side = if rng.random::<bool>() { 1.0 } else { -1.0 };
    let bow1 = side * rng.random_range(0.03..0.2) * dist;
    let bow2 = side * rng.random_range(0.03..0.2) * dist;
    let c1 = (from.0 + dx * 0.3 + nx * bow1, from.1 + dy * 0.3 + ny * bow1);
    let c2 = (from.0 + dx * 0.7 + nx * bow2, from.1 + dy * 0.7 + ny * bow2);

    (1..=steps)
        .map(|i| {
            if i == steps {
                return to;
            }
            let t = i as f64 / steps as f64;
            let s = t * t * t * (10.0 - 15.0 * t + 6.0 * t * t);
            let u = 1.0 - s;
            let b = |p0: f64, p1: f64, p2: f64, p3: f64| {
                u * u * u * p0 + 3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s * p3
            };
            (b(from.0, c1.0, c2.0, to.0), b(from.1, c1.1, c2.1, to.1))
        })
        .collect()
}

/// Bring the cursor to `pt`: along [`cursor_path`] from where the last
/// press left it when `glide` is set, otherwise (or with no last press)
/// in one move.
async fn move_cursor_to(page: &Page, pt: Point, glide: bool) -> Result<()> {
    let from = *LAST_CURSOR.lock().unwrap_or_else(|e| e.into_inner());
    let path = match from {
        Some(from) if glide => cursor_path(from, (pt.x, pt.y), &mut rand::rng()),
        _ => vec![(pt.x, pt.y)],
    };
    let last = path.len() - 1;
    for (i, (x, y)) in path.into_iter().enumerate() {
        page.move_mouse(Point::new(x, y))
            .await
            .context("CDP move_mouse")?;
        if i < last {
            tokio::time::sleep(Duration::from_millis(PATH_STEP_MS as u64)).await;
        }
    }
    Ok(())
}

/// Dispatch a single mouse click at `(x, y)` (CSS pixels) as four CDP
/// events, with mandatory hover before press:
///
/// 1. `mouseMoved` to `(x, y)` — a glide of several when `glide` is set
///    (see [`cursor_path`])
/// 2. sleep `hover_delay_ms` (≥100ms — Laya's input system samples hover
///    state before mousedown registers a hit on a tile sprite)
/// 3. `mousePressed`
/// 4. sleep `click_hold_ms`
/// 5. `mouseReleased`
///
/// `chromiumoxide::Page::click` collapses 3+5 into back-to-back frames
/// without the hover delay, which Majsoul drops on the floor for hand
/// tiles. Hand-rolling the sequence is required.
pub async fn dispatch_click(
    page: &Page,
    x: f64,
    y: f64,
    hover_delay_ms: u32,
    click_hold_ms: u32,
    glide: bool,
) -> Result<()> {
    dispatch_click_shaped(page, x, y, hover_delay_ms, click_hold_ms, false, glide).await
}

/// As [`dispatch_click`], but able to vary the *shape* of the press.
///
/// `jiggle` nudges the cursor a pixel mid-press and puts it back. It
/// exists for retries: when a press lands on the right control and the
/// action still does not commit, the position is not what is wrong, so
/// the only thing left to change is how the press is made.
pub async fn dispatch_click_shaped(
    page: &Page,
    x: f64,
    y: f64,
    hover_delay_ms: u32,
    click_hold_ms: u32,
    jiggle: bool,
    glide: bool,
) -> Result<()> {
    let pt = Point::new(x, y);
    move_cursor_to(page, pt, glide).await?;
    *LAST_CURSOR.lock().unwrap_or_else(|e| e.into_inner()) = Some((x, y));
    if hover_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(hover_delay_ms as u64)).await;
    }

    let press = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MousePressed)
        .x(pt.x)
        .y(pt.y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("build mousePressed: {e}"))?;
    page.execute(press).await.context("CDP mousePressed")?;

    if jiggle {
        // Split the hold around the nudge so its total stays what the
        // config says. The move is one CSS pixel — enough for the engine
        // to resample the cursor while the button is down, not enough to
        // leave the control.
        let half = u64::from(click_hold_ms) / 2;
        if half > 0 {
            tokio::time::sleep(Duration::from_millis(half)).await;
        }
        page.move_mouse(Point::new(x + 1.0, y))
            .await
            .context("CDP move_mouse (jiggle out)")?;
        page.move_mouse(pt)
            .await
            .context("CDP move_mouse (jiggle back)")?;
        let rest = u64::from(click_hold_ms) - half;
        if rest > 0 {
            tokio::time::sleep(Duration::from_millis(rest)).await;
        }
    } else if click_hold_ms > 0 {
        tokio::time::sleep(Duration::from_millis(click_hold_ms as u64)).await;
    }

    let release = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseReleased)
        .x(pt.x)
        .y(pt.y)
        .button(MouseButton::Left)
        .click_count(1)
        .build()
        .map_err(|e| anyhow!("build mouseReleased: {e}"))?;
    page.execute(release).await.context("CDP mouseReleased")?;

    Ok(())
}

/// Read the game canvas's `getBoundingClientRect()` via `Runtime.evaluate`.
///
/// Majsoul renders into the first `<canvas>` element on the page; Tenhou
/// uses the same selector when running in browser mode. We grab the
/// first canvas indiscriminately — multi-canvas pages aren't a thing on
/// these platforms.
pub async fn evaluate_canvas_rect(page: &Page) -> Result<CanvasRect> {
    // IIFE so `Runtime.evaluate` returns a single value, not a Promise.
    // `is_likely_js_function` in chromiumoxide picks the right CDP call
    // based on whether the expression looks like a function — we wrap
    // in `(()=>{...})()` to ensure plain-expression evaluation.
    let expr = "(()=>{const c=document.getElementsByTagName('canvas')[0];\
                if(!c)return null;\
                const r=c.getBoundingClientRect();\
                return {x:r.x,y:r.y,width:r.width,height:r.height};})()";
    let result = page
        .evaluate(expr)
        .await
        .context("CDP evaluate canvas rect")?;
    let value = result
        .value()
        .ok_or_else(|| anyhow!("canvas rect: no value returned"))?;
    if value.is_null() {
        return Err(anyhow!("canvas rect: page has no <canvas> element"));
    }
    let rect: CanvasRect = serde_json::from_value(value.clone())
        .context("canvas rect: deserialise from page value")?;
    Ok(rect)
}

/// One screenshot of the whole viewport, decoded to RGB.
pub struct Frame {
    width: usize,
    height: usize,
    /// Image pixels per CSS pixel.
    scale: f64,
    pixels: Vec<[u8; 3]>,
}

impl Frame {
    /// The pixels of a region given in CSS pixels, row by row, clamped to
    /// the image.
    pub fn region(&self, x: f64, y: f64, width: f64, height: f64) -> Vec<Vec<[u8; 3]>> {
        let px = |v: f64, max: usize| ((v * self.scale).round().max(0.0) as usize).min(max);
        let (left, right) = (px(x, self.width), px(x + width, self.width));
        let (top, bottom) = (px(y, self.height), px(y + height, self.height));
        (top..bottom)
            .map(|row| self.pixels[row * self.width + left..row * self.width + right].to_vec())
            .collect()
    }
}

/// Capture the whole viewport.
///
/// Never with a `clip`: Chrome serves a clipped capture by briefly
/// emulating a viewport the size of the clip, offset to it, so a visible
/// window flashes that region at its top-left on every capture. A
/// full-viewport capture needs no emulation and leaves the window alone.
///
/// The raw `Page.captureScreenshot` command rather than `Page::screenshot`:
/// the wrapper activates the tab first, which would pull the game to the
/// front on every poll.
pub async fn capture_viewport(page: &Page) -> Result<Frame> {
    use base64::Engine as _;
    use chromiumoxide::cdp::browser_protocol::page::{
        CaptureScreenshotFormat, CaptureScreenshotParams, GetLayoutMetricsParams,
    };
    let metrics = page
        .execute(GetLayoutMetricsParams::default())
        .await
        .context("CDP getLayoutMetrics")?;
    let css_width = metrics.result.css_visual_viewport.client_width;
    let params = CaptureScreenshotParams::builder()
        .format(CaptureScreenshotFormat::Png)
        .optimize_for_speed(true)
        .build();
    let shot = page
        .execute(params)
        .await
        .context("CDP captureScreenshot")?;
    let data: &str = shot.result.data.as_ref();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("screenshot: base64")?;
    let mut decoder = png::Decoder::new(bytes.as_slice());
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().context("screenshot: png header")?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("screenshot: png frame")?;
    let channels = info.color_type.samples();
    if channels < 3 {
        return Err(anyhow!(
            "screenshot: unexpected colour type {:?}",
            info.color_type
        ));
    }
    if css_width <= 0.0 {
        return Err(anyhow!("screenshot: viewport has no width"));
    }
    Ok(Frame {
        width: info.width as usize,
        height: info.height as usize,
        scale: f64::from(info.width) / css_width,
        pixels: buf[..info.buffer_size()]
            .chunks_exact(channels)
            .map(|p| [p[0], p[1], p[2]])
            .collect(),
    })
}

// ============================================================================
// Tenhou actuation
// ============================================================================
//
// Tenhou's client owns its board state: when you discard, its own handler
// updates the board *and* sends the frame, and its receive path then
// deliberately ignores the server's echo of that discard
// (`1==U.a && "D"==c.tag || Nb.cb(c)`) because it has already applied it.
// Writing the frame onto the socket behind the client's back therefore
// freezes the board — the local apply never happened and the echo is
// skipped. Everything here exists to drive the client's *own* input path
// instead, so its state machine stays in step.
//
// Two routes, because the client has two:
//
// - Buttons (chi/pon/kan/riichi/ron/tsumo/kyuushu/kita/pass) are real DOM
//   elements carrying `class="s7" name="c22-<slot>"`, routed by a
//   body-level click listener into the client's own handler. A dispatched
//   click is indistinguishable from the user's.
// - The discard is a canvas hit-test with no DOM element, so it needs a
//   pixel position — see [`probe_hand_geometry`].

/// Slots in the client's action menu. The client builds this menu itself
/// from the server's `t` bitmask, and the slot number *is* the meaning.
pub mod menu {
    pub const TSUMO_AGARI: u8 = 0;
    pub const RON: u8 = 1;
    pub const RIICHI: u8 = 2;
    pub const KYUUSHU: u8 = 3;
    pub const PASS: u8 = 4;
    /// 5..=9 — kita (sanma), one per distinct North the client offers.
    pub const KITA_FIRST: u8 = 5;
    /// 10..=12 — ankan / kakan candidates.
    pub const KAN_FIRST: u8 = 10;
    pub const DAIMINKAN: u8 = 13;
    /// The pon, spending the red five if the hand holds one. Always drawn
    /// when a pon is on offer — the client's builder writes it for all three
    /// shapes of holding (two plain copies, red + plain, or the red of a
    /// three-copy set).
    pub const PON: u8 = 15;
    /// The pon that *keeps* the red five out of the meld, drawn only when
    /// there is a choice to make: exactly one red copy and two plain ones.
    /// It holds the two plain copies.
    pub const PON_KEEP_RED: u8 = 14;
    /// 16..=21 — chi, in pairs running called-tile-lowest, -middle,
    /// -highest. The odd slot of each pair spends no red five; the even one
    /// below it does. Unlike pon, either can be drawn without the other:
    /// which exist is decided per shape by which copies the hand holds.
    pub const CHI_FIRST: u8 = 16;
}

/// CSS selector for one action button.
pub fn action_button_selector(slot: u8) -> String {
    format!(r#"button.s7[name="c22-{slot}"]"#)
}

/// Which action buttons the client is currently showing, in document order.
///
/// The client rebuilds this set on every decision window, so it is the
/// authoritative list of what may be pressed right now — better than
/// re-deriving the menu from the `t` bitmask and hoping the two agree.
///
/// Left in the client's own order rather than sorted: it renders highest slot
/// first, and seeing that in a log is what tells you a selector list was
/// resolved the wrong way round.
pub async fn list_action_buttons(page: &Page) -> Result<Vec<u8>> {
    let expr = "(()=>Array.from(document.querySelectorAll('button.s7[name^=\"c22-\"]'))\
                .map(b=>parseInt(b.getAttribute('name').slice(4),10))\
                .filter(n=>!isNaN(n)))()";
    let result = page
        .evaluate(expr)
        .await
        .context("CDP evaluate action button list")?;
    let value = result
        .value()
        .cloned()
        .unwrap_or(serde_json::Value::Array(vec![]));
    Ok(serde_json::from_value::<Vec<u8>>(value).unwrap_or_default())
}

/// Dispatch a real click on the first of `selectors` the page actually has.
///
/// The list is a *preference* order, and it has to be honoured as one: the
/// client appends its buttons in **descending** slot number
/// (`Object.keys(k).sort((a,n)=>n-a)`), so a single comma-joined selector
/// would resolve through `querySelector`'s document order and hand back the
/// highest-numbered match — the opposite of what a caller listing
/// "this one, or that one" means.
///
/// Returns `Ok(false)` when none match — the window closed while we were
/// thinking, or the client never offered any of them. Callers report that as
/// a skipped action rather than pressing something else.
pub async fn click_dom(page: &Page, selectors: &[String]) -> Result<bool> {
    let literal = serde_json::to_string(selectors).context("encode selectors as JS literal")?;
    let expr = format!(
        "(()=>{{for(const s of {literal}){{const e=document.querySelector(s);\
          if(e){{e.click();return true;}}}}return false;}})()"
    );
    let result = page
        .evaluate(expr)
        .await
        .context("CDP evaluate DOM click")?;
    Ok(result.value().and_then(|v| v.as_bool()).unwrap_or(false))
}

/// Is the client taking input yet?
///
/// The client's turn clock and its highlight box are raised by the same call
/// (`Ub.O`), at the end of the animation for whatever opened the window — so
/// the box appearing *is* "animation finished, clock started". Frame arrival
/// is not: the server can deliver several seats' actions at once and the
/// client will spend seconds animating them.
pub async fn turn_clock_running(page: &Page) -> Result<bool> {
    let expr = "(()=>{for(const d of document.querySelectorAll('div')){\
        const s=d.style; if(s.position!=='fixed'||s.display==='none')continue;\
        const c=d.firstElementChild;\
        if(!c||!c.classList||!c.classList.contains('ts2'))continue;\
        const r=d.getBoundingClientRect();\
        if(r.width>0&&r.height>0)return true;}\
      return false;})()";
    let result = page
        .evaluate(expr)
        .await
        .context("CDP evaluate turn clock probe")?;
    Ok(result.value().and_then(|v| v.as_bool()).unwrap_or(false))
}

/// Discard `tile_index` through the client's own handler.
///
/// `Ok(false)` means the client script was never instrumented, so the handler
/// is not reachable — reported as a skipped action rather than fumbled into a
/// click at a guessed position.
pub async fn discard_tile(page: &Page, tile_index: u32) -> Result<bool> {
    let expr = crate::autoplay::tenhou::inject::discard_expression(tile_index);
    let result = page.evaluate(expr).await.context("CDP evaluate discard")?;
    Ok(result.value().and_then(|v| v.as_str()) == Some("ok"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_names_the_menu_slot() {
        assert_eq!(
            action_button_selector(menu::PASS),
            r#"button.s7[name="c22-4"]"#
        );
        assert_eq!(
            action_button_selector(menu::RIICHI),
            r#"button.s7[name="c22-2"]"#
        );
    }

    /// A selector containing quotes must not break out of the JS literal.
    #[test]
    fn selector_is_escaped_into_the_expression() {
        let literal = serde_json::to_string(r#"button.s7[name="c22-4"]"#).unwrap();
        assert!(literal.contains(r#"\"c22-4\""#));
    }

    #[test]
    fn jittered_ms_never_drops_below_base() {
        for _ in 0..1000 {
            let ms = jittered_ms(200, 60);
            assert!((200..=260).contains(&ms));
        }
        assert_eq!(jittered_ms(200, 0), 200);
    }

    #[test]
    fn cursor_path_ends_on_target_and_stays_near_the_line() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        let mut rng = StdRng::seed_from_u64(7);
        let (from, to) = ((100.0, 800.0), (900.0, 300.0));
        let dist = (800.0f64).hypot(500.0);
        for _ in 0..200 {
            let path = cursor_path(from, to, &mut rng);
            assert!(path.len() >= 2);
            assert_eq!(*path.last().unwrap(), to);
            // Control points sit at most 0.2·dist off the line, and a
            // Bézier stays inside their hull.
            for &(x, y) in &path {
                let off = ((x - from.0) * -500.0 - (y - from.1) * 800.0).abs() / dist;
                assert!(off <= 0.2 * dist + 1e-6);
            }
        }
    }

    #[test]
    fn cursor_path_skips_a_glide_when_already_there() {
        let mut rng = rand::rng();
        assert_eq!(
            cursor_path((5.0, 5.0), (6.0, 5.0), &mut rng),
            vec![(6.0, 5.0)]
        );
    }
}
