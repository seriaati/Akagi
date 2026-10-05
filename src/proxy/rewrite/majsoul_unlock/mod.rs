//! Unlocks every Mahjong Soul character and cosmetic — on this machine only.
//!
//! The behaviour follows [MajsoulMax](https://github.com/Avenshy/MajsoulMax);
//! this is an independent implementation, not a port of its code.
//!
//! ## How
//!
//! - **Server → client.** Responses that describe what the account owns
//!   (`fetchCharacterInfo`, `fetchBagInfo`, `fetchTitleList`, the
//!   aggregated `fetchInfo`, …) are rewritten to say it owns everything in
//!   [`catalog.json`](catalog.json) — every character at max bond, every
//!   skin, title, view item, loading image and ending. Places that show the
//!   player's own look (login, profile, rooms, the table) get the choices
//!   from [`state::Saved`].
//! - **Client → server.** A cosmetic change (main character, skin, title,
//!   outfit preset, …) names items the account may not own, so it must not
//!   reach the server. The choice is saved locally and the request is
//!   rewritten in place into a `loginBeat` heartbeat under the *same*
//!   message index. The server answers that with an empty `ResCommon`, which
//!   is exactly the success reply the client was waiting for — so neither
//!   side sees a gap or an unanswered request.
//! - **Notifies.** The server's own character updates would undo the above
//!   mid-session, so `NotifyAccountUpdate.update.character` is stripped. A
//!   skin change gets a synthesized one instead, sent straight to the
//!   client, so the new skin shows without re-entering the screen.
//!
//! ## Limits
//!
//! Display only: other players still see the account's real character and
//! outfit, and nothing here touches gameplay frames. Unlocked emotes are
//! deliberately left out — emotes are sent to the table, so using one the
//! account does not own is visible to everyone.
//!
//! ## Declining
//!
//! Any frame that fails to decode, carries a server error, or belongs to a
//! method this module does not know is forwarded untouched.

mod state;

use crate::bridge::majsoul::parser::{self, POOL};
use anyhow::{ensure, Context, Result};
use prost::Message as _;
use prost_reflect::{DynamicMessage, Value};
use serde::Deserialize;
use state::{Saved, Store, ViewSlot};
use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex as StdMutex},
};
use tracing::debug;

const CATALOG_JSON: &str = include_str!("catalog.json");

#[derive(Deserialize)]
struct Catalog {
    characters: Vec<CatalogCharacter>,
    skins: Vec<u32>,
    titles: Vec<u32>,
    items: Vec<u32>,
    loading_images: Vec<u32>,
    endings: Vec<u32>,
}

#[derive(Deserialize)]
struct CatalogCharacter {
    id: u32,
    init_skin: u32,
}

static CATALOG: LazyLock<Catalog> =
    LazyLock::new(|| serde_json::from_str(CATALOG_JSON).expect("failed to parse catalog.json"));

const LOGIN: &str = ".lq.Lobby.login";
const OAUTH2_LOGIN: &str = ".lq.Lobby.oauth2Login";
const EMAIL_LOGIN: &str = ".lq.Lobby.emailLogin";
const LOGIN_BEAT: &str = ".lq.Lobby.loginBeat";
const FETCH_INFO: &str = ".lq.Lobby.fetchInfo";
const FETCH_CHARACTER_INFO: &str = ".lq.Lobby.fetchCharacterInfo";
const FETCH_BAG_INFO: &str = ".lq.Lobby.fetchBagInfo";
const FETCH_TITLE_LIST: &str = ".lq.Lobby.fetchTitleList";
const FETCH_ALL_COMMON_VIEWS: &str = ".lq.Lobby.fetchAllCommonViews";
const FETCH_ACCOUNT_INFO: &str = ".lq.Lobby.fetchAccountInfo";
const CREATE_ROOM: &str = ".lq.Lobby.createRoom";
const FETCH_ROOM: &str = ".lq.Lobby.fetchRoom";
const JOIN_ROOM: &str = ".lq.Lobby.joinRoom";
const AUTH_GAME: &str = ".lq.FastTest.authGame";
const CHANGE_MAIN_CHARACTER: &str = ".lq.Lobby.changeMainCharacter";
const CHANGE_CHARACTER_SKIN: &str = ".lq.Lobby.changeCharacterSkin";
const UPDATE_CHARACTER_SORT: &str = ".lq.Lobby.updateCharacterSort";
const SET_HIDDEN_CHARACTER: &str = ".lq.Lobby.setHiddenCharacter";
const USE_TITLE: &str = ".lq.Lobby.useTitle";
const SET_LOADING_IMAGE: &str = ".lq.Lobby.setLoadingImage";
const SAVE_COMMON_VIEWS: &str = ".lq.Lobby.saveCommonViews";
const USE_COMMON_VIEW: &str = ".lq.Lobby.useCommonView";
const RECEIVE_CHARACTER_REWARDS: &str = ".lq.Lobby.receiveCharacterRewards";
const ADD_FINISHED_ENDING: &str = ".lq.Lobby.addFinishedEnding";
const SET_RANDOM_CHARACTER: &str = ".lq.Lobby.setRandomCharacter";
const NOTIFY_ACCOUNT_UPDATE: &str = ".lq.NotifyAccountUpdate";
const NOTIFY_ROOM_PLAYER_UPDATE: &str = ".lq.NotifyRoomPlayerUpdate";
const NOTIFY_GAME_FINISH_REWARD: &str = ".lq.NotifyGameFinishRewardV2";

const MAX_LEVEL: u32 = 5;
/// The outfit slot that holds the avatar frame.
const FRAME_SLOT: u32 = 5;
const SLOT_RANDOM: u32 = 1;

pub enum Verdict {
    Forward,
    Replace(Vec<u8>),
    Drop,
}

pub struct Outcome {
    pub verdict: Verdict,
    /// A frame to send to the game client, after `verdict` is applied.
    pub to_client: Option<Vec<u8>>,
}

impl Outcome {
    fn forward() -> Self {
        Self::of(Verdict::Forward)
    }

    fn of(verdict: Verdict) -> Self {
        Self {
            verdict,
            to_client: None,
        }
    }
}

/// Per-WebSocket state: which method each in-flight request index belongs
/// to, since a response names only the index.
#[derive(Default)]
pub struct FlowState {
    pending: HashMap<u16, Arc<str>>,
}

/// Shared by every flow: the lobby and game sockets are separate
/// connections, but a choice made in one shows up in the other.
pub struct Unlock {
    inner: StdMutex<Inner>,
}

struct Inner {
    store: Store,
    saved: Saved,
    // Learned from traffic, never persisted.
    account_id: u32,
    real_main: Option<u32>,
    real_view_index: Option<u32>,
    real_characters: HashMap<u32, DynamicMessage>,
    contract: String,
}

impl Unlock {
    pub fn load(path: PathBuf) -> Self {
        Self::with_store(Store::at(path))
    }

    fn with_store(store: Store) -> Self {
        LazyLock::force(&CATALOG);
        let saved = store.load();
        Self {
            inner: StdMutex::new(Inner {
                store,
                saved,
                account_id: 0,
                real_main: None,
                real_view_index: None,
                real_characters: HashMap::new(),
                contract: String::new(),
            }),
        }
    }

    /// Decide what to do with one binary liqi frame, in either direction.
    pub fn rewrite(&self, flow: &mut FlowState, buf: &[u8]) -> Outcome {
        let mut inner = self.inner.lock().expect("unlock mutex poisoned");
        let result = match buf.first() {
            Some(1) => inner.notify(buf),
            Some(2) => inner.request(flow, buf),
            Some(3) => inner.response(flow, buf),
            _ => Ok(Outcome::forward()),
        };
        result.unwrap_or_else(|e| {
            debug!("unlock: forwarding frame untouched: {e:#}");
            Outcome::forward()
        })
    }
}

impl Inner {
    fn request(&mut self, flow: &mut FlowState, buf: &[u8]) -> Result<Outcome> {
        ensure!(buf.len() >= 3, "request frame too short");
        let index = u16::from_le_bytes([buf[1], buf[2]]);
        let wrapper = parser::decode_wrapper(&buf[3..])?;
        let method: Arc<str> = Arc::from(wrapper.name.as_str());
        flow.pending.insert(index, method.clone());

        let req = || -> Result<DynamicMessage> {
            let (desc, _) = parser::lookup_method_types(&method)?;
            Ok(DynamicMessage::decode(desc, wrapper.data.as_slice())?)
        };
        let mut to_client = None;
        match &*method {
            LOGIN_BEAT => {
                self.contract = get_str(&req()?, "contract");
                return Ok(Outcome::forward());
            }
            AUTH_GAME => {
                // Covers an Akagi started after the lobby login.
                if self.account_id == 0 {
                    self.account_id = get_u32(&req()?, "account_id");
                }
                return Ok(Outcome::forward());
            }
            USE_COMMON_VIEW => {
                // Only an index, so safe to forward — and forwarding keeps
                // the server's real preset choice in step.
                self.saved.view_index = Some(get_u32(&req()?, "index"));
                self.store.save(&self.saved);
                return Ok(Outcome::forward());
            }
            CHANGE_MAIN_CHARACTER => {
                self.saved.main_character = Some(get_u32(&req()?, "character_id"));
            }
            CHANGE_CHARACTER_SKIN => {
                let req = req()?;
                let character = get_u32(&req, "character_id");
                self.saved.skins.insert(character, get_u32(&req, "skin"));
                to_client = Some(self.character_update(character)?);
            }
            UPDATE_CHARACTER_SORT => {
                self.saved.character_sort = Some(get_u32s(&req()?, "sort"));
            }
            SET_HIDDEN_CHARACTER => {
                self.saved.hidden_characters = Some(get_u32s(&req()?, "chara_list"));
            }
            USE_TITLE => self.saved.title = Some(get_u32(&req()?, "title")),
            SET_LOADING_IMAGE => self.saved.loading_images = Some(get_u32s(&req()?, "images")),
            SAVE_COMMON_VIEWS => {
                let req = req()?;
                let index = get_u32(&req, "save_index");
                let slots = children(&req, "views").map(view_slot_from).collect();
                self.saved.views.insert(index, slots);
                if get_u32(&req, "is_use") == 1 {
                    self.saved.view_index = Some(index);
                }
            }
            // Nothing to remember, but each would ask the server about
            // things the account does not own.
            RECEIVE_CHARACTER_REWARDS | ADD_FINISHED_ENDING | SET_RANDOM_CHARACTER => {}
            _ => return Ok(Outcome::forward()),
        }
        self.store.save(&self.saved);
        debug!("unlock: answering {method} locally");

        let mut beat = new_message("lq.ReqLoginBeat")?;
        beat.set_field_by_name("contract", Value::String(self.contract.clone()));
        let mut out = buf[..3].to_vec();
        out.extend(parser::encode_wrapper(LOGIN_BEAT, beat.encode_to_vec()));
        Ok(Outcome {
            verdict: Verdict::Replace(out),
            to_client,
        })
    }

    fn response(&mut self, flow: &mut FlowState, buf: &[u8]) -> Result<Outcome> {
        ensure!(buf.len() >= 3, "response frame too short");
        let index = u16::from_le_bytes([buf[1], buf[2]]);
        let Some(method) = flow.pending.remove(&index) else {
            return Ok(Outcome::forward());
        };
        if !matches!(
            &*method,
            LOGIN
                | OAUTH2_LOGIN
                | EMAIL_LOGIN
                | FETCH_INFO
                | FETCH_CHARACTER_INFO
                | FETCH_BAG_INFO
                | FETCH_TITLE_LIST
                | FETCH_ALL_COMMON_VIEWS
                | FETCH_ACCOUNT_INFO
                | CREATE_ROOM
                | FETCH_ROOM
                | JOIN_ROOM
                | AUTH_GAME
                | SET_HIDDEN_CHARACTER
        ) {
            return Ok(Outcome::forward());
        }
        let wrapper = parser::decode_wrapper(&buf[3..])?;
        ensure!(wrapper.name.is_empty(), "response wrapper has a name");
        let (_, desc) = parser::lookup_method_types(&method)?;
        let mut msg = DynamicMessage::decode(desc, wrapper.data.as_slice())?;
        if has_error(&msg) {
            return Ok(Outcome::forward());
        }

        match &*method {
            LOGIN | OAUTH2_LOGIN | EMAIL_LOGIN => {
                self.account_id = get_u32(&msg, "account_id");
                if let Some(account) = child_mut(&mut msg, "account") {
                    self.patch_account(account)?;
                }
            }
            FETCH_INFO => {
                if let Some(info) = child_mut(&mut msg, "character_info") {
                    self.patch_character_info(info)?;
                }
                if let Some(bag) = child_mut(&mut msg, "bag_info").and_then(|b| child_mut(b, "bag"))
                {
                    patch_bag(bag)?;
                }
                if let Some(views) = child_mut(&mut msg, "all_common_views") {
                    self.patch_views(views)?;
                }
                if let Some(titles) = child_mut(&mut msg, "title_list") {
                    patch_titles(titles);
                }
            }
            FETCH_CHARACTER_INFO => self.patch_character_info(&mut msg)?,
            FETCH_BAG_INFO => {
                if let Some(bag) = child_mut(&mut msg, "bag") {
                    patch_bag(bag)?;
                }
            }
            FETCH_TITLE_LIST => patch_titles(&mut msg),
            FETCH_ALL_COMMON_VIEWS => self.patch_views(&mut msg)?,
            FETCH_ACCOUNT_INFO => {
                if let Some(account) = child_mut(&mut msg, "account") {
                    if self.is_me(account) {
                        self.patch_account(account)?;
                    }
                }
            }
            CREATE_ROOM | FETCH_ROOM | JOIN_ROOM => {
                if let Some(room) = child_mut(&mut msg, "room") {
                    self.patch_players(room, "persons")?;
                }
            }
            AUTH_GAME => self.patch_players(&mut msg, "players")?,
            SET_HIDDEN_CHARACTER => {
                // The server answered the heartbeat, so the list it would
                // have echoed back is missing.
                if let Some(hidden) = &self.saved.hidden_characters {
                    msg.set_field_by_name("hidden_characters", u32s(hidden.iter().copied()));
                }
            }
            _ => unreachable!("filtered above"),
        }

        let mut out = buf[..3].to_vec();
        out.extend(parser::encode_wrapper("", msg.encode_to_vec()));
        Ok(Outcome::of(Verdict::Replace(out)))
    }

    fn notify(&mut self, buf: &[u8]) -> Result<Outcome> {
        let wrapper = parser::decode_wrapper(&buf[1..])?;
        if !matches!(
            wrapper.name.as_str(),
            NOTIFY_ACCOUNT_UPDATE | NOTIFY_ROOM_PLAYER_UPDATE | NOTIFY_GAME_FINISH_REWARD
        ) {
            return Ok(Outcome::forward());
        }
        let desc = parser::lookup_notify_type(&wrapper.name)?;
        let mut msg = DynamicMessage::decode(desc, wrapper.data.as_slice())?;

        match wrapper.name.as_str() {
            NOTIFY_ACCOUNT_UPDATE => {
                let Some(update) = child_mut(&mut msg, "update") else {
                    return Ok(Outcome::forward());
                };
                if !update.has_field_by_name("character") {
                    return Ok(Outcome::forward());
                }
                update.clear_field_by_name("character");
                // The rest of an update (currency, tasks, …) is real and
                // still has to arrive.
                if update.fields().next().is_none() && update.unknown_fields().next().is_none() {
                    return Ok(Outcome::of(Verdict::Drop));
                }
            }
            NOTIFY_ROOM_PLAYER_UPDATE => self.patch_players(&mut msg, "player_list")?,
            NOTIFY_GAME_FINISH_REWARD => {
                let Some(main) = child_mut(&mut msg, "main_character") else {
                    return Ok(Outcome::forward());
                };
                main.set_field_by_name("level", Value::U32(MAX_LEVEL));
                main.set_field_by_name("exp", Value::U32(0));
                main.set_field_by_name("add", Value::U32(0));
            }
            _ => unreachable!("filtered above"),
        }

        let mut out = vec![1];
        out.extend(parser::encode_wrapper(&wrapper.name, msg.encode_to_vec()));
        Ok(Outcome::of(Verdict::Replace(out)))
    }

    fn main_character(&self) -> Option<u32> {
        self.saved.main_character.or(self.real_main)
    }

    fn skin_of(&self, character: u32) -> u32 {
        if let Some(&skin) = self.saved.skins.get(&character) {
            return skin;
        }
        if let Some(real) = self.real_characters.get(&character) {
            let skin = get_u32(real, "skin");
            if skin != 0 {
                return skin;
            }
        }
        CATALOG
            .characters
            .iter()
            .find(|c| c.id == character)
            .map(|c| c.init_skin)
            .unwrap_or_else(|| default_skin(character))
    }

    fn is_me(&self, account: &DynamicMessage) -> bool {
        self.account_id != 0 && get_u32(account, "account_id") == self.account_id
    }

    /// The character at max bond, built on the real one when the account
    /// owns it so fields this module does not manage stay true.
    fn perfect_character(&self, character: u32) -> Result<DynamicMessage> {
        let mut c = match self.real_characters.get(&character) {
            Some(real) => real.clone(),
            None => new_message("lq.Character")?,
        };
        c.set_field_by_name("charid", Value::U32(character));
        c.set_field_by_name("level", Value::U32(MAX_LEVEL));
        c.set_field_by_name("exp", Value::U32(0));
        c.set_field_by_name("is_upgraded", Value::Bool(true));
        c.set_field_by_name("rewarded_level", u32s(1..=MAX_LEVEL));
        c.set_field_by_name("skin", Value::U32(self.skin_of(character)));
        Ok(c)
    }

    /// The current outfit preset with random slots resolved, if the player
    /// has saved one through this module.
    fn current_slots(&self) -> Option<Vec<ViewSlot>> {
        let index = self.saved.view_index.or(self.real_view_index)?;
        let slots = self.saved.views.get(&index)?;
        Some(
            slots
                .iter()
                .map(|s| {
                    let mut s = s.clone();
                    if s.kind == SLOT_RANDOM && !s.item_id_list.is_empty() {
                        s.item_id = s.item_id_list[rand::random_range(0..s.item_id_list.len())];
                    }
                    s
                })
                .collect(),
        )
    }

    /// `NotifyAccountUpdate` carrying one character, for a skin change.
    fn character_update(&self, character: u32) -> Result<Vec<u8>> {
        let mut char_update = new_message("lq.AccountUpdate.CharacterUpdate")?;
        char_update.set_field_by_name(
            "characters",
            Value::List(vec![Value::Message(self.perfect_character(character)?)]),
        );
        let mut update = new_message("lq.AccountUpdate")?;
        update.set_field_by_name("character", Value::Message(char_update));
        let mut notify = new_message("lq.NotifyAccountUpdate")?;
        notify.set_field_by_name("update", Value::Message(update));
        let mut out = vec![1];
        out.extend(parser::encode_wrapper(
            NOTIFY_ACCOUNT_UPDATE,
            notify.encode_to_vec(),
        ));
        Ok(out)
    }

    /// `lq.Account`: the player's own profile.
    fn patch_account(&self, account: &mut DynamicMessage) -> Result<()> {
        if let Some(main) = self.main_character() {
            account.set_field_by_name("avatar_id", Value::U32(self.skin_of(main)));
        }
        if let Some(title) = self.saved.title {
            account.set_field_by_name("title", Value::U32(title));
        }
        if let Some(images) = &self.saved.loading_images {
            account.set_field_by_name("loading_image", u32s(images.iter().copied()));
        }
        if let Some(frame) = self.current_slots().and_then(|s| frame_of(&s)) {
            account.set_field_by_name("avatar_frame", Value::U32(frame));
        }
        Ok(())
    }

    /// Our own entry in a list of `lq.PlayerGameView`.
    fn patch_players(&self, msg: &mut DynamicMessage, field: &str) -> Result<()> {
        if self.account_id == 0 {
            return Ok(());
        }
        let Some(list) = msg
            .get_field_by_name_mut(field)
            .and_then(Value::as_list_mut)
        else {
            return Ok(());
        };
        for player in list.iter_mut().filter_map(Value::as_message_mut) {
            if !self.is_me(player) {
                continue;
            }
            if let Some(main) = self.main_character() {
                player.set_field_by_name("avatar_id", Value::U32(self.skin_of(main)));
                player
                    .set_field_by_name("character", Value::Message(self.perfect_character(main)?));
            }
            if let Some(title) = self.saved.title {
                player.set_field_by_name("title", Value::U32(title));
            }
            if let Some(slots) = self.current_slots() {
                if let Some(frame) = frame_of(&slots) {
                    player.set_field_by_name("avatar_frame", Value::U32(frame));
                }
                let views = slots
                    .iter()
                    .map(|s| view_slot_to(s).map(Value::Message))
                    .collect::<Result<_>>()?;
                player.set_field_by_name("views", Value::List(views));
            }
        }
        Ok(())
    }

    /// `lq.ResCharacterInfo`.
    fn patch_character_info(&mut self, info: &mut DynamicMessage) -> Result<()> {
        let real_main = get_u32(info, "main_character_id");
        if real_main != 0 {
            self.real_main = Some(real_main);
        }
        self.real_characters = children(info, "characters")
            .map(|c| (get_u32(c, "charid"), c.clone()))
            .collect();

        // Union with what the server sent, in case the catalog is older
        // than the client.
        let ids: BTreeSet<u32> = CATALOG
            .characters
            .iter()
            .map(|c| c.id)
            .chain(self.real_characters.keys().copied())
            .collect();
        let characters = ids
            .into_iter()
            .map(|id| self.perfect_character(id).map(Value::Message))
            .collect::<Result<_>>()?;
        info.set_field_by_name("characters", Value::List(characters));
        let skins = union(&CATALOG.skins, &get_u32s(info, "skins"));
        info.set_field_by_name("skins", u32s(skins));
        if let Some(main) = self.main_character() {
            info.set_field_by_name("main_character_id", Value::U32(main));
        }
        if let Some(sort) = &self.saved.character_sort {
            info.set_field_by_name("character_sort", u32s(sort.iter().copied()));
        }
        if let Some(hidden) = &self.saved.hidden_characters {
            info.set_field_by_name("hidden_characters", u32s(hidden.iter().copied()));
        }
        for field in ["finished_endings", "rewarded_endings"] {
            let endings = union(&CATALOG.endings, &get_u32s(info, field));
            info.set_field_by_name(field, u32s(endings));
        }
        Ok(())
    }

    /// `lq.ResAllcommonViews`: saved presets replace the server's, the rest
    /// stay as they are.
    fn patch_views(&mut self, views: &mut DynamicMessage) -> Result<()> {
        self.real_view_index = Some(get_u32(views, "use"));
        let list = views
            .get_field_by_name_mut("views")
            .and_then(Value::as_list_mut)
            .context("ResAllcommonViews.views is not a list")?;
        for (&index, slots) in &self.saved.views {
            let values = Value::List(
                slots
                    .iter()
                    .map(|s| view_slot_to(s).map(Value::Message))
                    .collect::<Result<_>>()?,
            );
            let existing = list
                .iter_mut()
                .filter_map(Value::as_message_mut)
                .find(|v| get_u32(v, "index") == index);
            match existing {
                Some(view) => view.set_field_by_name("values", values),
                None => {
                    let mut view = new_message("lq.ResAllcommonViews.Views")?;
                    view.set_field_by_name("index", Value::U32(index));
                    view.set_field_by_name("values", values);
                    list.push(Value::Message(view));
                }
            }
        }
        if let Some(index) = self.saved.view_index {
            views.set_field_by_name("use", Value::U32(index));
        }
        Ok(())
    }
}

/// `lq.Bag`: every view item and loading image, on top of the real items.
fn patch_bag(bag: &mut DynamicMessage) -> Result<()> {
    let owned: BTreeSet<u32> = children(bag, "items")
        .map(|i| get_u32(i, "item_id"))
        .collect();
    let mut extra = Vec::new();
    for &id in CATALOG.items.iter().chain(&CATALOG.loading_images) {
        if owned.contains(&id) {
            continue;
        }
        let mut item = new_message("lq.Item")?;
        item.set_field_by_name("item_id", Value::U32(id));
        item.set_field_by_name("stack", Value::U32(1));
        extra.push(Value::Message(item));
    }
    if let Some(items) = bag
        .get_field_by_name_mut("items")
        .and_then(Value::as_list_mut)
    {
        items.extend(extra);
    }
    Ok(())
}

/// `lq.ResTitleList`.
fn patch_titles(titles: &mut DynamicMessage) {
    let all = union(&CATALOG.titles, &get_u32s(titles, "title_list"));
    titles.set_field_by_name("title_list", u32s(all));
}

fn frame_of(slots: &[ViewSlot]) -> Option<u32> {
    slots
        .iter()
        .find(|s| s.slot == FRAME_SLOT && s.item_id != 0)
        .map(|s| s.item_id)
}

/// A preset slot as saved: a fixed slot keeps only its item, a random one
/// only its pool.
fn view_slot_from(m: &DynamicMessage) -> ViewSlot {
    let kind = get_u32(m, "type");
    let random = kind == SLOT_RANDOM;
    ViewSlot {
        slot: get_u32(m, "slot"),
        item_id: if random { 0 } else { get_u32(m, "item_id") },
        kind,
        item_id_list: if random {
            get_u32s(m, "item_id_list")
        } else {
            Vec::new()
        },
    }
}

fn view_slot_to(s: &ViewSlot) -> Result<DynamicMessage> {
    let mut m = new_message("lq.ViewSlot")?;
    m.set_field_by_name("slot", Value::U32(s.slot));
    m.set_field_by_name("item_id", Value::U32(s.item_id));
    m.set_field_by_name("type", Value::U32(s.kind));
    m.set_field_by_name("item_id_list", u32s(s.item_id_list.iter().copied()));
    Ok(m)
}

/// The client's own rule for a character's first skin, used only for a
/// character newer than the catalog: `200034` → `403401`,
/// `20000101` → `40010101`.
fn default_skin(character: u32) -> u32 {
    let id = character.to_string();
    id.get(4..)
        .and_then(|tail| format!("40{tail}01").parse().ok())
        .unwrap_or(0)
}

fn new_message(name: &str) -> Result<DynamicMessage> {
    let desc = POOL
        .get_message_by_name(name)
        .with_context(|| format!("unknown message {name}"))?;
    Ok(DynamicMessage::new(desc))
}

fn has_error(msg: &DynamicMessage) -> bool {
    msg.get_field_by_name("error")
        .and_then(|e| e.as_message().map(|e| get_u32(e, "code")))
        .unwrap_or(0)
        != 0
}

fn get_u32(m: &DynamicMessage, field: &str) -> u32 {
    m.get_field_by_name(field)
        .and_then(|v| v.as_u32())
        .unwrap_or(0)
}

fn get_u32s(m: &DynamicMessage, field: &str) -> Vec<u32> {
    m.get_field_by_name(field)
        .and_then(|v| {
            v.as_list()
                .map(|l| l.iter().filter_map(Value::as_u32).collect())
        })
        .unwrap_or_default()
}

fn get_str(m: &DynamicMessage, field: &str) -> String {
    m.get_field_by_name(field)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// A set sub-message, or `None` when the field is absent.
fn child_mut<'a>(m: &'a mut DynamicMessage, field: &str) -> Option<&'a mut DynamicMessage> {
    if !m.has_field_by_name(field) {
        return None;
    }
    m.get_field_by_name_mut(field)?.as_message_mut()
}

fn children<'a>(m: &'a DynamicMessage, field: &str) -> impl Iterator<Item = &'a DynamicMessage> {
    // An unset field comes back as an owned default, i.e. an empty list.
    let list = match m.get_field_by_name(field) {
        Some(Cow::Borrowed(v)) => v.as_list().unwrap_or_default(),
        _ => &[],
    };
    list.iter().filter_map(Value::as_message)
}

fn u32s(values: impl IntoIterator<Item = u32>) -> Value {
    Value::List(values.into_iter().map(Value::U32).collect())
}

fn union(a: &[u32], b: &[u32]) -> Vec<u32> {
    a.iter()
        .chain(b)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn unlock() -> Unlock {
        Unlock::with_store(Store::memory())
    }

    fn message(name: &str, value: serde_json::Value) -> DynamicMessage {
        let desc = POOL.get_message_by_name(name).expect("known message");
        DynamicMessage::deserialize(desc, value).expect("valid fixture")
    }

    fn request(index: u16, method: &str, value: serde_json::Value) -> Vec<u8> {
        let (desc, _) = parser::lookup_method_types(method).unwrap();
        let mut out = vec![2];
        out.extend(index.to_le_bytes());
        out.extend(parser::encode_wrapper(
            method,
            message(desc.full_name(), value).encode_to_vec(),
        ));
        out
    }

    fn response(index: u16, method: &str, value: serde_json::Value) -> Vec<u8> {
        let (_, desc) = parser::lookup_method_types(method).unwrap();
        let mut out = vec![3];
        out.extend(index.to_le_bytes());
        out.extend(parser::encode_wrapper(
            "",
            message(desc.full_name(), value).encode_to_vec(),
        ));
        out
    }

    fn notify(name: &str, value: serde_json::Value) -> Vec<u8> {
        let mut out = vec![1];
        out.extend(parser::encode_wrapper(
            name,
            message(name.trim_start_matches('.'), value).encode_to_vec(),
        ));
        out
    }

    fn replaced(outcome: Outcome) -> Vec<u8> {
        match outcome.verdict {
            Verdict::Replace(out) => out,
            Verdict::Forward => panic!("expected a rewrite, got forward"),
            Verdict::Drop => panic!("expected a rewrite, got drop"),
        }
    }

    /// Send `req` up, then `resp` down under the same index; the decoded
    /// rewritten response.
    fn exchange(
        unlock: &Unlock,
        flow: &mut FlowState,
        index: u16,
        method: &str,
        resp: serde_json::Value,
    ) -> DynamicMessage {
        unlock.rewrite(flow, &request(index, method, json!({})));
        let out = replaced(unlock.rewrite(flow, &response(index, method, resp)));
        let (_, desc) = parser::lookup_method_types(method).unwrap();
        DynamicMessage::decode(
            desc,
            parser::decode_wrapper(&out[3..]).unwrap().data.as_slice(),
        )
        .unwrap()
    }

    fn login(unlock: &Unlock, flow: &mut FlowState) -> DynamicMessage {
        exchange(
            unlock,
            flow,
            1,
            OAUTH2_LOGIN,
            json!({"account_id": 7, "account": {"account_id": 7, "avatar_id": 400101}}),
        )
    }

    fn characters_by_id(info: &DynamicMessage) -> HashMap<u32, DynamicMessage> {
        children(info, "characters")
            .map(|c| (get_u32(c, "charid"), c.clone()))
            .collect()
    }

    #[test]
    fn catalog_loads() {
        assert!(CATALOG
            .characters
            .iter()
            .any(|c| c.id == 200001 && c.init_skin == 400101));
        assert!(!CATALOG.skins.is_empty() && !CATALOG.items.is_empty());
    }

    #[test]
    fn character_info_lists_every_character_at_max_bond() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        let info = exchange(
            &unlock,
            &mut flow,
            3,
            FETCH_CHARACTER_INFO,
            json!({
                "characters": [{"charid": 200001, "level": 2, "skin": 400102, "extra_emoji": [13]}],
                "skins": [400102],
                "main_character_id": 200001,
            }),
        );
        let chars = characters_by_id(&info);
        assert_eq!(chars.len(), CATALOG.characters.len());
        for c in chars.values() {
            assert_eq!(get_u32(c, "level"), MAX_LEVEL);
            assert_eq!(get_u32s(c, "rewarded_level"), vec![1, 2, 3, 4, 5]);
        }
        // An owned character keeps its real skin and fields we don't manage.
        let owned = &chars[&200001];
        assert_eq!(get_u32(owned, "skin"), 400102);
        assert_eq!(get_u32s(owned, "extra_emoji"), vec![13]);
        // An unowned one gets its initial skin.
        assert_eq!(get_u32(&chars[&200002], "skin"), 400201);
        assert_eq!(get_u32s(&info, "skins").len(), CATALOG.skins.len());
        assert_eq!(get_u32(&info, "main_character_id"), 200001);
    }

    #[test]
    fn cosmetic_request_becomes_a_heartbeat_under_the_same_index() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        unlock.rewrite(
            &mut flow,
            &request(9, LOGIN_BEAT, json!({"contract": "abc"})),
        );

        let out = replaced(unlock.rewrite(
            &mut flow,
            &request(10, CHANGE_MAIN_CHARACTER, json!({"character_id": 200002})),
        ));
        assert_eq!(out[..3], [2, 10, 0]);
        let wrapper = parser::decode_wrapper(&out[3..]).unwrap();
        assert_eq!(wrapper.name, LOGIN_BEAT);
        let beat = DynamicMessage::decode(
            POOL.get_message_by_name("lq.ReqLoginBeat").unwrap(),
            wrapper.data.as_slice(),
        )
        .unwrap();
        assert_eq!(get_str(&beat, "contract"), "abc");

        // The server's empty ResCommon reaches the client as-is.
        let reply = response(10, LOGIN_BEAT, json!({}));
        assert!(matches!(
            unlock.rewrite(&mut flow, &reply).verdict,
            Verdict::Forward
        ));
    }

    #[test]
    fn choices_show_up_on_later_logins() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        unlock.rewrite(
            &mut flow,
            &request(1, CHANGE_MAIN_CHARACTER, json!({"character_id": 200003})),
        );
        unlock.rewrite(
            &mut flow,
            &request(
                2,
                CHANGE_CHARACTER_SKIN,
                json!({"character_id": 200003, "skin": 400303}),
            ),
        );
        unlock.rewrite(&mut flow, &request(3, USE_TITLE, json!({"title": 600002})));

        let res = login(&unlock, &mut flow);
        let account = res.get_field_by_name("account").unwrap();
        let account = account.as_message().unwrap();
        assert_eq!(get_u32(account, "avatar_id"), 400303);
        assert_eq!(get_u32(account, "title"), 600002);

        let info = exchange(&unlock, &mut flow, 4, FETCH_CHARACTER_INFO, json!({}));
        assert_eq!(get_u32(&info, "main_character_id"), 200003);
        assert_eq!(get_u32(&characters_by_id(&info)[&200003], "skin"), 400303);
    }

    #[test]
    fn skin_change_is_shown_to_the_client_at_once() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        let outcome = unlock.rewrite(
            &mut flow,
            &request(
                5,
                CHANGE_CHARACTER_SKIN,
                json!({"character_id": 200001, "skin": 400105}),
            ),
        );
        let frame = outcome.to_client.expect("a synthesized notify");
        assert_eq!(frame[0], 1);
        let wrapper = parser::decode_wrapper(&frame[1..]).unwrap();
        assert_eq!(wrapper.name, NOTIFY_ACCOUNT_UPDATE);
        let notify = DynamicMessage::decode(
            parser::lookup_notify_type(&wrapper.name).unwrap(),
            wrapper.data.as_slice(),
        )
        .unwrap();
        let update = notify.get_field_by_name("update").unwrap();
        let character = update
            .as_message()
            .unwrap()
            .get_field_by_name("character")
            .unwrap();
        let chars: Vec<_> = children(character.as_message().unwrap(), "characters")
            .map(|c| (get_u32(c, "charid"), get_u32(c, "skin")))
            .collect();
        assert_eq!(chars, vec![(200001, 400105)]);
    }

    #[test]
    fn server_character_updates_are_stripped() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        let only_character = notify(
            NOTIFY_ACCOUNT_UPDATE,
            json!({"update": {"character": {"skins": [400102]}}}),
        );
        assert!(matches!(
            unlock.rewrite(&mut flow, &only_character).verdict,
            Verdict::Drop
        ));

        let mixed = notify(
            NOTIFY_ACCOUNT_UPDATE,
            json!({"update": {"character": {"skins": [400102]}, "numerical": [{"id": 100002, "final": 5}]}}),
        );
        let out = replaced(unlock.rewrite(&mut flow, &mixed));
        let wrapper = parser::decode_wrapper(&out[1..]).unwrap();
        let msg = DynamicMessage::decode(
            parser::lookup_notify_type(&wrapper.name).unwrap(),
            wrapper.data.as_slice(),
        )
        .unwrap();
        let update = msg.get_field_by_name("update").unwrap();
        let update = update.as_message().unwrap();
        assert!(!update.has_field_by_name("character"));
        assert!(update.has_field_by_name("numerical"));
    }

    #[test]
    fn only_our_own_seat_is_dressed_at_the_table() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        login(&unlock, &mut flow);
        unlock.rewrite(
            &mut flow,
            &request(
                2,
                SAVE_COMMON_VIEWS,
                json!({
                    "save_index": 1,
                    "is_use": 1,
                    "views": [
                        {"slot": 5, "item_id": 305501, "type": 0, "item_id_list": [1, 2]},
                        {"slot": 1, "item_id": 9, "type": 1, "item_id_list": [305001, 305002]},
                    ],
                }),
            ),
        );
        unlock.rewrite(
            &mut flow,
            &request(3, CHANGE_MAIN_CHARACTER, json!({"character_id": 200002})),
        );

        let game = exchange(
            &unlock,
            &mut flow,
            4,
            AUTH_GAME,
            json!({"players": [
                {"account_id": 7, "avatar_id": 400101, "character": {"charid": 200001}},
                {"account_id": 8, "avatar_id": 400101, "character": {"charid": 200001}},
            ]}),
        );
        let players: Vec<_> = children(&game, "players").collect();
        let me = players[0];
        assert_eq!(get_u32(me, "avatar_id"), 400201);
        assert_eq!(get_u32(me, "avatar_frame"), 305501);
        let views: Vec<_> = children(me, "views").collect();
        assert_eq!(get_u32s(views[0], "item_id_list"), Vec::<u32>::new());
        assert!([305001, 305002].contains(&get_u32(views[1], "item_id")));

        let other = players[1];
        assert_eq!(get_u32(other, "avatar_id"), 400101);
        assert!(!other.has_field_by_name("views"));
    }

    #[test]
    fn errors_and_unknown_traffic_pass_through() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        unlock.rewrite(&mut flow, &request(1, FETCH_CHARACTER_INFO, json!({})));
        let failed = response(1, FETCH_CHARACTER_INFO, json!({"error": {"code": 1004}}));
        assert!(matches!(
            unlock.rewrite(&mut flow, &failed).verdict,
            Verdict::Forward
        ));

        let other = request(2, ".lq.Lobby.fetchFriendList", json!({}));
        assert!(matches!(
            unlock.rewrite(&mut flow, &other).verdict,
            Verdict::Forward
        ));
        assert!(matches!(
            unlock.rewrite(&mut flow, &[2, 0]).verdict,
            Verdict::Forward
        ));
        assert!(matches!(
            unlock.rewrite(&mut flow, &[3, 9, 9, 0xff]).verdict,
            Verdict::Forward
        ));
    }

    #[test]
    fn bag_gains_every_view_item_once() {
        let unlock = unlock();
        let mut flow = FlowState::default();
        let owned = CATALOG.items[0];
        let res = exchange(
            &unlock,
            &mut flow,
            1,
            FETCH_BAG_INFO,
            json!({"bag": {"items": [{"item_id": owned, "stack": 1}, {"item_id": 100001, "stack": 3}]}}),
        );
        let bag = res.get_field_by_name("bag").unwrap();
        let ids: Vec<u32> = children(bag.as_message().unwrap(), "items")
            .map(|i| get_u32(i, "item_id"))
            .collect();
        assert_eq!(ids.iter().filter(|&&i| i == owned).count(), 1);
        assert!(ids.contains(&100001));
        assert!(CATALOG.items.iter().all(|i| ids.contains(i)));
    }

    #[test]
    fn default_skin_follows_the_client_rule() {
        assert_eq!(default_skin(200034), 403401);
        assert_eq!(default_skin(20000101), 40010101);
    }
}
