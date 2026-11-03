//! The settings, kept per character as JSON in the plugin's storage
//! (mudsort's `Properties/Settings.settings`, which Decal kept per Windows
//! user; the layout of the file follows Mosswart Massacre's
//! `context.rs`, `load_settings` and `save_settings`).
//!
//! Loaded at login, written on every change. A missing or unreadable file
//! reads as the defaults; nothing is written until something changes.

use openac_plugin::Host;
use openac_plugin::storage::{StoragePlace, StorageRoot};
use serde::{Deserialize, Serialize};

use crate::sort_string::{self, SortString};

/// The key of the settings file in the character's storage.
pub const SETTINGS_KEY: &str = "settings.json";

/// mudsort's three saved sort strings (`SavedSortString1` to `3`), as it
/// shipped them. They are kept as written, so they decode as in mudsort:
/// `MA` in the first matches no key in today's code table (as in mudsort's
/// default, `DIVERGENCES.md` MS-1) and is skipped, leaving `OC,EP,AL-,MX-`.
pub const MUDSORT_SAVED: [&str; 3] = ["OC,EP,MA,AL-,MX-", "AL-,MX-", "BR-"];

/// A named sort string.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preset {
    /// What the player called it.
    pub name: String,
    /// The sort string.
    pub sort_string: String,
}

impl Preset {
    /// A preset.
    #[must_use]
    pub fn new(name: &str, sort_string: &str) -> Self {
        Self {
            name: name.to_owned(),
            sort_string: sort_string.to_owned(),
        }
    }
}

/// The presets a new character starts with: "Everything" (the default sort
/// string) and mudsort's three saved strings as "Saved 1" to "Saved 3".
#[must_use]
pub fn default_presets() -> Vec<Preset> {
    let mut presets = vec![Preset::new("Everything", sort_string::DEFAULT)];
    for (index, text) in MUDSORT_SAVED.iter().enumerate() {
        presets.push(Preset::new(&format!("Saved {}", index + 1), text));
    }
    presets
}

/// One character's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The sort string (`DefaultSortString`, the Sort tab's field).
    pub sort_string: String,
    /// Only object classes whose name starts with this, ignoring case;
    /// empty for every class (mudsort's object class filter).
    pub class_filter: String,
    /// Only items whose name contains this, ignoring case; empty for every
    /// item. New: mudsort had none.
    pub name_filter: String,
    /// The slot in the destination every item is put at (mudsort's insert
    /// position).
    pub start_slot: i32,
    /// Reverse the whole list (`ReverseSortList`).
    pub reverse: bool,
    /// Queue the whole inventory for identification at login
    /// (`IdentifyOnLogin`).
    pub identify_on_login: bool,
    /// Send `/tell <self>, done sorting!` when a sort ends
    /// (`ThinkWhenDone`).
    pub announce_when_done: bool,
    /// Named sort strings (mudsort had three unnamed slots).
    pub presets: Vec<Preset>,
}

impl Default for Settings {
    /// mudsort's defaults (`Settings.settings`), with the default sort
    /// string of `DIVERGENCES.md` MS-1.
    fn default() -> Self {
        Self {
            sort_string: sort_string::DEFAULT.to_owned(),
            class_filter: String::new(),
            name_filter: String::new(),
            start_slot: 0,
            reverse: false,
            identify_on_login: true,
            announce_when_done: true,
            presets: default_presets(),
        }
    }
}

impl Settings {
    /// The settings a file holds; a member it does not have reads as its
    /// default.
    ///
    /// # Errors
    /// The text is not a settings object.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    /// The file's text.
    #[must_use]
    pub fn to_json(&self) -> String {
        // A struct of strings, numbers and lists always serializes.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The sort string, read.
    #[must_use]
    pub fn keys(&self) -> SortString {
        sort_string::parse(&self.sort_string)
    }

    /// The class filter, `None` when empty.
    #[must_use]
    pub fn class_filter(&self) -> Option<&str> {
        (!self.class_filter.is_empty()).then_some(self.class_filter.as_str())
    }

    /// The name filter, `None` when empty.
    #[must_use]
    pub fn name_filter(&self) -> Option<&str> {
        (!self.name_filter.is_empty()).then_some(self.name_filter.as_str())
    }
}

/// Where the logged-in character's settings live.
#[derive(Debug, Clone, Default)]
pub struct SettingsStore {
    place: Option<StoragePlace>,
}

impl SettingsStore {
    /// Reads `character`'s settings; the defaults when there are none or
    /// they cannot be read (logged).
    pub fn load(&mut self, host: &mut Host<'_>, character: &str) -> Settings {
        let place = StoragePlace::character(StorageRoot::PluginData, character);
        let settings = match host.storage_read_text(&place, SETTINGS_KEY) {
            Ok(Some(text)) if !text.trim().is_empty() => match Settings::from_json(&text) {
                Ok(settings) => settings,
                Err(error) => {
                    host.warn(&format!(
                        "Settings for {character} could not be read, using the defaults: {error}"
                    ));
                    Settings::default()
                }
            },
            Ok(_) => Settings::default(),
            Err(error) => {
                host.warn(&format!(
                    "Settings for {character} could not be read, using the defaults: {error}"
                ));
                Settings::default()
            }
        };
        self.place = Some(place);
        settings
    }

    /// Writes the settings (nothing before a login named the character).
    pub fn save(&self, host: &mut Host<'_>, settings: &Settings) {
        let Some(place) = &self.place else {
            return;
        };
        if let Err(error) = host.storage_write_text(place, SETTINGS_KEY, &settings.to_json()) {
            host.warn(&format!("Settings could not be saved: {error}"));
        }
    }

    /// Forgets the character (at logoff).
    pub fn close(&mut self) {
        self.place = None;
    }
}
