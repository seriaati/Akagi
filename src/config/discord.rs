use serde::{Deserialize, Serialize};

/// Discord Rich Presence: shows the current game on the user's Discord
/// profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    /// Off by default — a presence is broadcast to every Discord friend, so it
    /// is something the user opts into, not something they discover.
    pub enabled: bool,
    /// Application (client) ID from the Discord Developer Portal. Discord
    /// shows the application's name as "Playing <name>", so the user picks
    /// what their friends see by picking the application. Empty ⇒ no
    /// presence, even when `enabled`.
    pub client_id: String,
}

impl DiscordConfig {
    /// The client ID to connect with, or `None` when the presence should be
    /// off. The ID is a Discord snowflake: anything that isn't all digits
    /// would only fail the handshake, so it counts as unset.
    pub fn active_client_id(&self) -> Option<&str> {
        let id = self.client_id.trim();
        let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit());
        (self.enabled && valid).then_some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_gets_presence_off() {
        let cfg: crate::config::AppConfig = toml::from_str("[bot]\nenabled = true\n").unwrap();
        assert_eq!(cfg.discord, DiscordConfig::default());
        assert_eq!(cfg.discord.active_client_id(), None);
    }

    #[test]
    fn active_client_id_needs_enabled_and_a_numeric_id() {
        let mut c = DiscordConfig {
            enabled: true,
            client_id: " 123456789012345678 ".into(),
        };
        assert_eq!(c.active_client_id(), Some("123456789012345678"));

        c.client_id = "not-an-id".into();
        assert_eq!(c.active_client_id(), None);

        c.client_id = String::new();
        assert_eq!(c.active_client_id(), None);

        c.client_id = "123".into();
        c.enabled = false;
        assert_eq!(c.active_client_id(), None);
    }

    #[test]
    fn round_trips_through_toml() {
        let mut cfg = crate::config::AppConfig::default();
        cfg.discord.enabled = true;
        cfg.discord.client_id = "123456789012345678".into();

        let body = toml::to_string_pretty(&cfg).unwrap();
        assert!(body.contains("[discord]"), "expected [discord] in:\n{body}");

        let back: crate::config::AppConfig = toml::from_str(&body).unwrap();
        assert_eq!(back.discord, cfg.discord);
    }
}
