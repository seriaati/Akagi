//! Discord Rich Presence: the current game on the user's Discord profile.
//!
//! Two halves, because the IPC client is blocking:
//!
//! - an async task folds the `MjaiBus` into a [`Game`] view and sends the
//!   resulting [`Presence`] (or `None` between games) whenever it changes;
//! - a dedicated thread owns the `DiscordIpcClient` and makes Discord match
//!   the latest one. It wakes every [`RETRY`] on its own as well, which is
//!   what retries a Discord that wasn't running and picks up `[discord]`
//!   config edits without any hook in `update_config`.
//!
//! "No presence" is done by closing the connection rather than sending a
//! clear: Discord drops an application's activity when its IPC socket
//! closes, and that holds even if Discord restarted in between.

use crate::config::{AppConfig, Platform};
use crate::schema::MjaiEvent;
use discord_rich_presence::activity::{Activity, Timestamps};
use discord_rich_presence::error::Error as DiscordError;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// How often the worker re-checks config and retries a failed connection.
/// Discord itself only applies one activity update per 15s.
const RETRY: Duration = Duration::from_secs(15);

/// Language the presence is written in: the app's UI language, which the
/// frontend reports via `set_ui_language`. Unknown tags fall back to English.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    ZhTw,
    ZhCn,
    Ja,
}

impl Lang {
    /// From an i18next language tag (`SUPPORTED_LANGS` in the frontend).
    pub fn from_tag(tag: &str) -> Self {
        match tag {
            "zh-TW" => Lang::ZhTw,
            "zh-CN" => Lang::ZhCn,
            "ja" => Lang::Ja,
            _ => Lang::En,
        }
    }
}

/// What Discord should show. Compared against the last one sent so a
/// `dahai` that changes nothing visible costs nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    /// "Mahjong Soul" — replaces the application's name in "Playing …".
    pub name: String,
    /// "4P East-South"
    pub details: String,
    /// "East 2 · 1 honba · 2nd · 28,400"
    pub state: String,
    /// `start_game` time, Unix ms — Discord renders it as "elapsed".
    pub start_ms: i64,
}

/// The slice of a game the presence needs. `None` outside a game.
#[derive(Debug)]
struct Game {
    start_ms: i64,
    /// Our seat; `None` on a neutral stream, which drops the placement.
    seat: Option<u8>,
    num_players: u8,
    /// Majsoul `game_config.mode.mode`; other platforms don't report one.
    match_mode: Option<u8>,
    round: Option<Round>,
}

#[derive(Debug)]
struct Round {
    bakaze: String,
    kyoku: u8,
    honba: u8,
    scores: Vec<i32>,
}

fn apply(game: &mut Option<Game>, event: &MjaiEvent, now_ms: i64) {
    match event {
        MjaiEvent::StartGame {
            id,
            num_players,
            game_meta,
            ..
        } => {
            *game = Some(Game {
                start_ms: now_ms,
                seat: *id,
                num_players: *num_players,
                match_mode: game_meta.as_ref().and_then(|m| m.match_mode),
                round: None,
            });
        }
        MjaiEvent::StartKyoku {
            bakaze,
            kyoku,
            honba,
            scores,
            ..
        } => {
            if let Some(g) = game {
                g.round = Some(Round {
                    bakaze: bakaze.clone(),
                    kyoku: *kyoku,
                    honba: *honba,
                    scores: scores.clone(),
                });
            }
        }
        MjaiEvent::EndGame { .. } => *game = None,
        _ => {}
    }
}

impl Game {
    fn presence(&self, platform: Platform, lang: Lang) -> Presence {
        let name = match (platform, lang) {
            (Platform::Majsoul, Lang::En) => "Mahjong Soul",
            (Platform::Majsoul, _) => "雀魂",
            (Platform::Tenhou, Lang::En) => "Tenhou",
            (Platform::Tenhou, Lang::ZhCn) => "天凤",
            (Platform::Tenhou, _) => "天鳳",
            (Platform::RiichiCity, Lang::En) => "Riichi City",
            (Platform::RiichiCity, _) => "麻雀一番街",
        };
        let players = match (lang, self.num_players) {
            (Lang::En, n) => format!("{n}P"),
            (_, 3) => "三人".to_string(),
            (_, 4) => "四人".to_string(),
            (_, n) => format!("{n}人"),
        };
        let length = match (self.match_mode, lang) {
            (Some(1 | 11), Lang::En) => " East",
            (Some(1 | 11), Lang::ZhCn) => "东风",
            (Some(1 | 11), _) => "東風",
            (Some(2 | 12), Lang::En) => " East-South",
            (Some(2 | 12), Lang::ZhTw) => "半莊",
            (Some(2 | 12), Lang::ZhCn) => "半庄",
            (Some(2 | 12), Lang::Ja) => "半荘",
            _ => "",
        };
        let details = format!("{players}{length}");

        let mut parts = Vec::new();
        if let Some(r) = &self.round {
            let wind = wind(&r.bakaze, lang);
            parts.push(match lang {
                Lang::En => format!("{wind} {}", r.kyoku),
                _ => format!("{wind}{}局", r.kyoku),
            });
            if r.honba > 0 {
                parts.push(match lang {
                    Lang::En => format!("{} honba", r.honba),
                    Lang::ZhCn => format!("{}本场", r.honba),
                    _ => format!("{}本場", r.honba),
                });
            }
            let seat = self.seat.map(usize::from);
            if let Some(seat) = seat.filter(|&s| s < r.scores.len()) {
                let rank = rank(&r.scores, seat);
                parts.push(match lang {
                    Lang::En => ordinal(rank).to_string(),
                    _ => format!("{rank}位"),
                });
                parts.push(thousands(r.scores[seat]));
            }
        }
        let state = if parts.is_empty() {
            match lang {
                Lang::En => "Starting",
                Lang::ZhTw => "開局中",
                Lang::ZhCn => "开局中",
                Lang::Ja => "対局開始",
            }
            .to_string()
        } else {
            parts.join(" · ")
        };

        Presence {
            name: name.to_string(),
            details,
            state,
            start_ms: self.start_ms,
        }
    }
}

fn wind(bakaze: &str, lang: Lang) -> &str {
    let i = match bakaze {
        "E" => 0,
        "S" => 1,
        "W" => 2,
        "N" => 3,
        other => return other,
    };
    let names = match lang {
        Lang::En => ["East", "South", "West", "North"],
        Lang::ZhCn => ["东", "南", "西", "北"],
        Lang::ZhTw | Lang::Ja => ["東", "南", "西", "北"],
    };
    names[i]
}

/// 1-based placement of `seat`. Ties go to the lower seat — seat 0 is the
/// starting dealer in mjai, which is the tie-break Majsoul and Tenhou use.
fn rank(scores: &[i32], seat: usize) -> usize {
    let mine = scores[seat];
    1 + scores
        .iter()
        .enumerate()
        .filter(|&(s, &p)| p > mine || (p == mine && s < seat))
        .count()
}

fn ordinal(n: usize) -> &'static str {
    match n {
        1 => "1st",
        2 => "2nd",
        3 => "3rd",
        _ => "4th",
    }
}

fn thousands(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

/// Start the presence task and its IPC thread. Does nothing visible until
/// `[discord]` is enabled with a client ID and a game starts. A `lang`
/// change mid-game shows up with the next event.
pub fn spawn(
    config: Arc<RwLock<AppConfig>>,
    lang: Arc<RwLock<Lang>>,
    mut events: broadcast::Receiver<MjaiEvent>,
) {
    let (tx, rx) = mpsc::channel();
    let worker_cfg = config.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("discord-presence".into())
        .spawn(move || worker(worker_cfg, rx))
    {
        warn!("discord: could not start presence thread: {e}");
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut game = None;
        let mut sent: Option<Presence> = None;
        loop {
            let event = match events.recv().await {
                Ok(e) => e,
                // A missed event at worst leaves a stale round until the next one.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            apply(&mut game, &event, chrono::Utc::now().timestamp_millis());
            let platform = config.read().await.platform.kind;
            let lang = *lang.read().await;
            let next = game.as_ref().map(|g| g.presence(platform, lang));
            if next != sent {
                if tx.send(next.clone()).is_err() {
                    return;
                }
                sent = next;
            }
        }
    });
}

struct Connection {
    client_id: String,
    client: DiscordIpcClient,
    shown: Option<Presence>,
}

fn worker(config: Arc<RwLock<AppConfig>>, rx: mpsc::Receiver<Option<Presence>>) {
    let mut want: Option<Presence> = None;
    let mut conn: Option<Connection> = None;
    loop {
        match rx.recv_timeout(RETRY) {
            Ok(p) => want = rx.try_iter().last().unwrap_or(p),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let client_id = config
            .blocking_read()
            .discord
            .active_client_id()
            .map(str::to_owned);

        let (Some(client_id), Some(p)) = (client_id, &want) else {
            disconnect(&mut conn);
            continue;
        };
        if conn.as_ref().is_some_and(|c| c.client_id != client_id) {
            disconnect(&mut conn);
        }
        if conn.as_ref().is_some_and(|c| c.shown.as_ref() == Some(p)) {
            continue;
        }
        if conn.is_none() {
            let mut client = DiscordIpcClient::new(&client_id);
            if let Err(e) = client.connect() {
                // Discord not running is the normal case; retried every RETRY.
                debug!("discord: connect failed: {e}");
                continue;
            }
            info!("discord: connected");
            conn = Some(Connection {
                client_id,
                client,
                shown: None,
            });
        }
        let c = conn.as_mut().expect("connected above");
        match show(&mut c.client, p) {
            Ok(()) => c.shown = Some(p.clone()),
            Err(e) => {
                debug!("discord: set_activity failed: {e}");
                disconnect(&mut conn);
            }
        }
    }
    disconnect(&mut conn);
}

fn show(client: &mut DiscordIpcClient, p: &Presence) -> Result<(), DiscordError> {
    client.set_activity(
        Activity::new()
            .name(p.name.as_str())
            .details(p.details.as_str())
            .state(p.state.as_str())
            .timestamps(Timestamps::new().start(p.start_ms)),
    )?;
    // Read the reply: it is where Discord reports a rejected payload, and
    // leaving replies unread would fill the socket over a long session.
    let (_, reply) = client.recv()?;
    if reply["evt"] == "ERROR" {
        warn!("discord: activity rejected: {}", reply["data"]);
    }
    Ok(())
}

fn disconnect(conn: &mut Option<Connection>) {
    if let Some(mut c) = conn.take() {
        let _ = c.client.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::GameMeta;

    fn start_game(seat: Option<u8>, num_players: u8, match_mode: Option<u8>) -> MjaiEvent {
        MjaiEvent::StartGame {
            names: vec![],
            kyoku_first: None,
            aka_flag: None,
            id: seat,
            num_players,
            game_meta: Some(GameMeta {
                game_id: None,
                match_mode,
                match_info: None,
            }),
        }
    }

    fn start_kyoku(bakaze: &str, kyoku: u8, honba: u8, scores: Vec<i32>) -> MjaiEvent {
        MjaiEvent::StartKyoku {
            bakaze: bakaze.into(),
            dora_marker: "1m".into(),
            kyoku,
            honba,
            kyotaku: 0,
            oya: kyoku - 1,
            num_players: scores.len() as u8,
            tehais: vec![],
            scores,
        }
    }

    fn presence(events: &[MjaiEvent], platform: Platform) -> Option<Presence> {
        presence_in(events, platform, Lang::En)
    }

    fn presence_in(events: &[MjaiEvent], platform: Platform, lang: Lang) -> Option<Presence> {
        let mut game = None;
        for e in events {
            apply(&mut game, e, 1_000);
        }
        game.map(|g| g.presence(platform, lang))
    }

    #[test]
    fn nothing_outside_a_game() {
        assert_eq!(presence(&[], Platform::Majsoul), None);
        let ended = [start_game(Some(0), 4, Some(2)), MjaiEvent::end_game()];
        assert_eq!(presence(&ended, Platform::Majsoul), None);
    }

    #[test]
    fn before_the_first_kyoku() {
        let events = [start_game(Some(0), 4, Some(2))];
        let p = presence(&events, Platform::Majsoul).unwrap();
        assert_eq!(p.name, "Mahjong Soul");
        assert_eq!(p.details, "4P East-South");
        assert_eq!(p.state, "Starting");
        assert_eq!(p.start_ms, 1_000);
    }

    #[test]
    fn mid_game() {
        let events = [
            start_game(Some(1), 4, Some(2)),
            start_kyoku("S", 2, 1, vec![31_000, 28_400, 28_400, 12_200]),
        ];
        let p = presence(&events, Platform::Majsoul).unwrap();
        assert_eq!(p.state, "South 2 · 1 honba · 2nd · 28,400");
    }

    #[test]
    fn sanma_without_a_match_mode_or_seat() {
        let events = [
            start_game(None, 3, None),
            start_kyoku("E", 1, 0, vec![35_000, 35_000, 35_000]),
        ];
        let p = presence(&events, Platform::Tenhou).unwrap();
        assert_eq!(p.name, "Tenhou");
        assert_eq!(p.details, "3P");
        assert_eq!(p.state, "East 1");
    }

    #[test]
    fn follows_the_ui_language() {
        let events = [
            start_game(Some(1), 4, Some(2)),
            start_kyoku("S", 2, 1, vec![31_000, 28_400, 28_400, 12_200]),
        ];
        let p = presence_in(&events, Platform::Majsoul, Lang::ZhTw).unwrap();
        assert_eq!(p.name, "雀魂");
        assert_eq!(p.details, "四人半莊");
        assert_eq!(p.state, "南2局 · 1本場 · 2位 · 28,400");

        let p = presence_in(&events[..1], Platform::Tenhou, Lang::ZhCn).unwrap();
        assert_eq!(p.name, "天凤");
        assert_eq!(p.state, "开局中");
    }

    #[test]
    fn language_tags() {
        assert_eq!(Lang::from_tag("zh-TW"), Lang::ZhTw);
        assert_eq!(Lang::from_tag("zh-CN"), Lang::ZhCn);
        assert_eq!(Lang::from_tag("ja"), Lang::Ja);
        assert_eq!(Lang::from_tag("en"), Lang::En);
        assert_eq!(Lang::from_tag("fr"), Lang::En);
    }

    #[test]
    fn ties_go_to_the_lower_seat() {
        let scores = [25_000, 25_000, 30_000, 20_000];
        assert_eq!(rank(&scores, 2), 1);
        assert_eq!(rank(&scores, 0), 2);
        assert_eq!(rank(&scores, 1), 3);
        assert_eq!(rank(&scores, 3), 4);
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(800), "800");
        assert_eq!(thousands(28_400), "28,400");
        assert_eq!(thousands(-1_200), "-1,200");
        assert_eq!(thousands(1_000_000), "1,000,000");
    }
}
