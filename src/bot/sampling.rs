//! Play temperature: under autoplay, sometimes play one of a bot's other
//! top choices instead of its first (`autoplay.play_temperature`).
//!
//! Works off the HUD card any bot may attach (`meta.show.items`, ranked,
//! each with a `prob`), so it covers the built-in bot and mjai bots such as
//! Mortal alike. Replayed on recorded games, both put a median ~0.84 on
//! their top move, so one temperature means much the same for either: at
//! 1.0 about one decision in five changes, at 0.5 about one in nine.
//!
//! Only swaps whose result is exact and always legal are made:
//!
//! - our turn (a discard, ankan or kakan was chosen) → another discard the
//!   card names by tile;
//! - a call window (pon, chi or daiminkan was chosen) → passing.
//!
//! A win, riichi, ryukyoku or kita is always played as chosen. Riichi in
//! particular: the bot manager has already fed a stateful bot the reach to
//! learn its discard, so not declaring it would leave the bot out of step.

use crate::schema::MjaiEvent;
use rand::Rng;
use serde_json::Value;

/// Rows of the card considered, the chosen move included.
const TOP_N: usize = 3;
/// A row less likely than this is never played: the tail of a bot's
/// ranking holds the moves no player would make.
const MIN_PROB: f64 = 0.05;

/// The move to play instead of `action`, or `None` to play it as chosen.
///
/// Each eligible row, the chosen one included, is weighted
/// `prob^(1/temperature)`. `drawn` is the tile we just drew, if this is our
/// turn after a draw; a discard of it is a tsumogiri.
pub fn resample(
    action: &MjaiEvent,
    meta: Option<&Value>,
    seat: u8,
    drawn: Option<&str>,
    temperature: f64,
    rng: &mut impl Rng,
) -> Option<MjaiEvent> {
    if !(temperature > 0.0 && temperature.is_finite()) {
        return None;
    }
    let items = meta?.pointer("/show/items")?.as_array()?;
    let rows: Vec<Row> = items.iter().take(TOP_N).map(Row::parse).collect();
    let top = rows.first()?;
    let top_prob = top.prob?;
    let own_turn = match action {
        MjaiEvent::Dahai { pai, .. } => {
            if top.label != "Discard" || top.pais != [pai.as_str()] {
                return None;
            }
            true
        }
        MjaiEvent::Ankan { .. } | MjaiEvent::Kakan { .. } if top.is_kan() => true,
        MjaiEvent::Pon { .. } if top.label == "Pon" => false,
        MjaiEvent::Chi { .. } if top.label == "Chi" => false,
        MjaiEvent::Daiminkan { .. } if top.is_kan() => false,
        _ => return None,
    };

    let mut options: Vec<(MjaiEvent, f64)> = Vec::new();
    for row in &rows[1..] {
        let Some(prob) = row.prob.filter(|&p| p >= MIN_PROB) else {
            continue;
        };
        let alt = match (own_turn, row.label, row.pais.as_slice()) {
            (true, "Discard", &[pai]) if !matches!(action, MjaiEvent::Dahai { pai: p, .. } if p == pai) => {
                MjaiEvent::Dahai {
                    actor: seat,
                    pai: pai.to_string(),
                    tsumogiri: drawn == Some(pai),
                }
            }
            (false, "Pass", _) => MjaiEvent::None,
            _ => continue,
        };
        options.push((alt, prob));
    }
    if options.is_empty() {
        return None;
    }

    let weight = |p: f64| p.powf(1.0 / temperature);
    let total = weight(top_prob) + options.iter().map(|(_, p)| weight(*p)).sum::<f64>();
    let mut pick = rng.random::<f64>() * total - weight(top_prob);
    if pick < 0.0 || !total.is_finite() {
        return None;
    }
    for (alt, p) in options {
        pick -= weight(p);
        if pick < 0.0 {
            return Some(alt);
        }
    }
    None
}

/// One ranked row of a `meta.show` card.
struct Row<'a> {
    label: &'a str,
    pais: Vec<&'a str>,
    prob: Option<f64>,
}

impl<'a> Row<'a> {
    fn parse(item: &'a Value) -> Self {
        Self {
            label: item.get("label").and_then(Value::as_str).unwrap_or(""),
            pais: item
                .get("pais")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default(),
            prob: item.get("prob").and_then(Value::as_f64),
        }
    }

    /// Mortal labels every kan "Kan"; the built-in bot tells them apart.
    fn is_kan(&self) -> bool {
        matches!(self.label, "Kan" | "Ankan" | "Kakan")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use serde_json::json;

    fn discard(pai: &str) -> MjaiEvent {
        MjaiEvent::Dahai {
            actor: 1,
            pai: pai.into(),
            tsumogiri: false,
        }
    }

    fn card(rows: &[(&str, &[&str], f64)]) -> Value {
        let items: Vec<Value> = rows
            .iter()
            .map(|(label, pais, prob)| json!({ "label": label, "pais": pais, "prob": prob }))
            .collect();
        json!({ "show": { "title": "t", "items": items } })
    }

    /// How often each outcome comes up over `n` draws at `t`.
    fn tally(
        action: &MjaiEvent,
        meta: &Value,
        drawn: Option<&str>,
        t: f64,
        n: usize,
    ) -> Vec<(Option<MjaiEvent>, usize)> {
        let mut rng = StdRng::seed_from_u64(7);
        let mut out: Vec<(Option<MjaiEvent>, usize)> = Vec::new();
        for _ in 0..n {
            let got = resample(action, Some(meta), 1, drawn, t, &mut rng);
            match out.iter_mut().find(|(e, _)| *e == got) {
                Some((_, c)) => *c += 1,
                None => out.push((got, 1)),
            }
        }
        out
    }

    fn count(t: &[(Option<MjaiEvent>, usize)], e: &Option<MjaiEvent>) -> usize {
        t.iter().find(|(x, _)| x == e).map_or(0, |(_, c)| *c)
    }

    #[test]
    fn zero_temperature_never_swaps() {
        let meta = card(&[("Discard", &["1m"], 0.5), ("Discard", &["9p"], 0.5)]);
        let t = tally(&discard("1m"), &meta, None, 0.0, 200);
        assert_eq!(count(&t, &None), 200);
    }

    /// At 1.0 the draw follows the card's own odds over the eligible rows.
    #[test]
    fn unit_temperature_follows_the_card() {
        let meta = card(&[
            ("Discard", &["1m"], 0.6),
            ("Discard", &["9p"], 0.3),
            ("Discard", &["E"], 0.1),
        ]);
        let t = tally(&discard("1m"), &meta, None, 1.0, 10_000);
        let share = |e| count(&t, &e) as f64 / 10_000.0;
        assert!((share(None) - 0.6).abs() < 0.02);
        assert!((share(Some(discard("9p"))) - 0.3).abs() < 0.02);
        assert!((share(Some(discard("E"))) - 0.1).abs() < 0.02);
    }

    #[test]
    fn lower_temperature_swaps_less() {
        let meta = card(&[("Discard", &["1m"], 0.6), ("Discard", &["9p"], 0.4)]);
        let swaps = |t| 10_000 - count(&tally(&discard("1m"), &meta, None, t, 10_000), &None);
        assert!(swaps(0.25) < swaps(1.0) / 2);
    }

    #[test]
    fn rows_under_the_floor_and_past_the_top_three_are_never_played() {
        let meta = card(&[
            ("Discard", &["1m"], 0.5),
            ("Discard", &["9p"], 0.04),
            ("Discard", &["E"], 0.3),
            ("Discard", &["S"], 0.16),
        ]);
        let t = tally(&discard("1m"), &meta, None, 2.0, 2_000);
        assert_eq!(count(&t, &Some(discard("9p"))), 0);
        assert_eq!(count(&t, &Some(discard("S"))), 0);
        assert!(count(&t, &Some(discard("E"))) > 0);
    }

    /// Wins and riichi are always played as chosen, and a card whose top row
    /// is not the chosen move (a bot that overrode its own ranking) is not
    /// trusted.
    #[test]
    fn wins_riichi_and_mismatched_cards_are_left_alone() {
        let mut rng = StdRng::seed_from_u64(7);
        let hora = MjaiEvent::Hora {
            actor: 1,
            target: 0,
            deltas: None,
            ura_markers: None,
        };
        let hora_card = card(&[("Hora", &[], 0.5), ("Pass", &[], 0.5)]);
        let reach = MjaiEvent::Reach {
            actor: 1,
            pai: Some("1m".into()),
        };
        let reach_card = card(&[("Riichi", &["1m"], 0.5), ("Discard", &["9p"], 0.5)]);
        let other_top = card(&[("Discard", &["2m"], 0.5), ("Discard", &["9p"], 0.5)]);
        for _ in 0..200 {
            assert_eq!(
                resample(&hora, Some(&hora_card), 1, None, 3.0, &mut rng),
                None
            );
            assert_eq!(
                resample(&reach, Some(&reach_card), 1, None, 3.0, &mut rng),
                None
            );
            assert_eq!(
                resample(&discard("1m"), Some(&other_top), 1, None, 3.0, &mut rng),
                None
            );
            assert_eq!(resample(&discard("1m"), None, 1, None, 3.0, &mut rng), None);
        }
    }

    #[test]
    fn a_call_can_become_a_pass_but_a_discard_cannot() {
        let pon = MjaiEvent::Pon {
            actor: 1,
            target: 0,
            pai: "5m".into(),
            consumed: ["5m".into(), "5m".into()],
        };
        let pon_card = card(&[("Pon", &["5m", "5m", "5m"], 0.5), ("Pass", &[], 0.5)]);
        let t = tally(&pon, &pon_card, None, 1.0, 1_000);
        assert!(count(&t, &Some(MjaiEvent::None)) > 0);

        let own_turn = card(&[("Discard", &["1m"], 0.5), ("Pass", &[], 0.5)]);
        assert_eq!(
            count(&tally(&discard("1m"), &own_turn, None, 3.0, 500), &None),
            500
        );
        // A pass can't become a call: the card doesn't say which tiles.
        let pass_card = card(&[("Pass", &[], 0.5), ("Pon", &[], 0.5)]);
        assert_eq!(
            count(&tally(&MjaiEvent::None, &pass_card, None, 3.0, 500), &None),
            500
        );
    }

    #[test]
    fn discarding_the_drawn_tile_is_a_tsumogiri() {
        let meta = card(&[("Discard", &["1m"], 0.5), ("Discard", &["9p"], 0.5)]);
        let t = tally(&discard("1m"), &meta, Some("9p"), 1.0, 1_000);
        let tsumogiri = Some(MjaiEvent::Dahai {
            actor: 1,
            pai: "9p".into(),
            tsumogiri: true,
        });
        assert!(count(&t, &tsumogiri) > 0);
        assert_eq!(count(&t, &Some(discard("9p"))), 0);
    }
}
