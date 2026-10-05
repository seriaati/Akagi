//! The choices the player makes in the game UI while everything is
//! unlocked. The server never hears about them (see the module docs), so
//! they have to live here instead — and survive a restart, or the player's
//! character and outfits would reset on every launch.
//!
//! Every field is "unset" until the player first changes it, and an unset
//! field leaves the server's real value alone. Turning the feature on
//! therefore changes nothing about how the account looks until the player
//! picks something.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tracing::warn;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub main_character: Option<u32>,
    /// Character id → skin id.
    pub skins: BTreeMap<u32, u32>,
    pub character_sort: Option<Vec<u32>>,
    pub hidden_characters: Option<Vec<u32>>,
    pub title: Option<u32>,
    pub loading_images: Option<Vec<u32>>,
    /// Outfit preset index → its slots.
    pub views: BTreeMap<u32, Vec<ViewSlot>>,
    pub view_index: Option<u32>,
}

/// `lq.ViewSlot`. `kind` 0 shows `item_id`; 1 picks one of `item_id_list`
/// at random each game.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSlot {
    pub slot: u32,
    pub item_id: u32,
    pub kind: u32,
    pub item_id_list: Vec<u32>,
}

/// Where `Saved` is kept. `None` keeps it in memory only (tests).
pub struct Store {
    path: Option<PathBuf>,
}

impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        Self { path: None }
    }

    /// A missing or unreadable file starts from scratch rather than failing:
    /// the worst case is the player re-picking their character.
    pub fn load(&self) -> Saved {
        let Some(path) = &self.path else {
            return Saved::default();
        };
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                warn!("ignoring unreadable {}: {e}", path.display());
                Saved::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(e) => {
                warn!("failed to read {}: {e}", path.display());
                Saved::default()
            }
        }
    }

    pub fn save(&self, saved: &Saved) {
        let Some(path) = &self.path else { return };
        if let Err(e) = write(path, saved) {
            warn!("failed to save {}: {e:#}", path.display());
        }
    }
}

fn write(path: &Path, saved: &Saved) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(saved)?)?;
    Ok(())
}
