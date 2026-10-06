//! How many own draws until tenpai.
//!
//! Each draw from the `unseen` pool (live wall + opponents' hands) hits one
//! of the K shanten-progressing tiles with probability `K / (unseen - n)`,
//! `n` = draws so far. At 1-shanten this is the exact hypergeometric
//! draw-without-replacement. At 2+ shanten each later step uses the
//! post-progress ukeire (`avg_next_shanten_waits`) as K, so the result is an
//! approximation. Calls and the round ending early are not modelled.

use serde::Serialize;

use super::result::Hand13Result;

#[derive(Debug, Clone, Serialize)]
pub struct TenpaiDraws {
    pub shanten: i8,
    /// Ukeire of the current step (`Hand13Result.waits_total`).
    pub ukeire: u32,
    /// Own draws left before the live wall runs out.
    pub draws_left: u8,
    /// Own draws until P(tenpai) reaches 50% / 80%. `None` if it never does.
    pub median: Option<u32>,
    pub p80: Option<u32>,
    /// P(tenpai within `draws_left` draws), percent.
    pub by_ryukyoku: f64,
    /// True at 1-shanten (exact), false for the multi-step approximation.
    pub exact: bool,
}

/// `None` when the hand is already tenpai (or agari) or nothing is unseen.
pub fn estimate(hand: &Hand13Result, unseen: u32, draws_left: u8) -> Option<TenpaiDraws> {
    if hand.shanten <= 0 || unseen == 0 {
        return None;
    }
    let steps = hand.shanten as usize;
    let ks: Vec<f64> = (0..steps)
        .map(|i| {
            if i == 0 {
                hand.waits_total as f64
            } else {
                hand.avg_next_shanten_waits
            }
        })
        .collect();

    // dist[j] = P(j steps completed so far).
    let mut dist = vec![0.0; steps + 1];
    dist[0] = 1.0;
    let (mut median, mut p80, mut by_ryukyoku) = (None, None, 0.0);
    for n in 0..unseen {
        let left = (unseen - n) as f64;
        // Walk steps high → low so one draw advances at most one step.
        for j in (0..steps).rev() {
            let moved = dist[j] * (ks[j] / left).min(1.0);
            dist[j] -= moved;
            dist[j + 1] += moved;
        }
        let done = dist[steps];
        let draws = n + 1;
        if draws == draws_left as u32 {
            by_ryukyoku = done * 100.0;
        }
        if median.is_none() && done >= 0.5 {
            median = Some(draws);
        }
        if p80.is_none() && done >= 0.8 {
            p80 = Some(draws);
        }
    }

    Some(TenpaiDraws {
        shanten: hand.shanten,
        ukeire: hand.waits_total,
        draws_left,
        median,
        p80,
        by_ryukyoku,
        exact: steps == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze_13;
    use crate::analysis::hand::PlayerInfo34Builder;

    fn comb(n: u32, k: u32) -> f64 {
        (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
    }

    fn hand(shanten: i8, ukeire: u32, next: f64) -> Hand13Result {
        let mut h = analyze_13(
            &PlayerInfo34Builder::new()
                .add_many(&[
                    "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m", "1p", "2p", "3p", "1s",
                ])
                .build(),
        );
        h.shanten = shanten;
        h.waits_total = ukeire;
        h.avg_next_shanten_waits = next;
        h
    }

    #[test]
    fn one_shanten_matches_hypergeometric() {
        // 16 outs in 90 unseen, 12 draws → 1 - C(74,12)/C(90,12) ≈ 92.0%.
        let r = estimate(&hand(1, 16, 0.0), 90, 12).unwrap();
        let want = 100.0 * (1.0 - comb(74, 12) / comb(90, 12));
        assert!((r.by_ryukyoku - want).abs() < 1e-9);
        assert!(r.exact);
        assert_eq!(r.median, Some(4));
        assert_eq!(r.p80, Some(8));
    }

    #[test]
    fn two_shanten_is_slower_than_one() {
        let one = estimate(&hand(1, 16, 0.0), 90, 12).unwrap();
        let two = estimate(&hand(2, 16, 16.0), 90, 12).unwrap();
        assert!(!two.exact);
        assert!(two.by_ryukyoku < one.by_ryukyoku);
        assert!(two.median.unwrap() > one.median.unwrap());
    }

    #[test]
    fn tenpai_and_dead_hands() {
        assert!(estimate(&hand(0, 8, 0.0), 90, 12).is_none());
        let dead = estimate(&hand(1, 0, 0.0), 90, 12).unwrap();
        assert_eq!(dead.median, None);
        assert_eq!(dead.by_ryukyoku, 0.0);
    }
}
