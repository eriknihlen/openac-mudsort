//! The plugin: mudsort's `PluginCore` (login and logoff, the `/ms`
//! commands, the source and destination) over the run, the identification
//! queue and the settings; and the state the window binds to.

use openac_plugin::items::InventoryItem;
use openac_plugin::ui::{UiAction, UiEvent};
use openac_plugin::world::{ObjectChangeKind, ObjectClass};
use openac_plugin::{CommandCall, Event, Host, Plugin, StartInfo};

use crate::chat;
use crate::codes;
use crate::commands::{self, Command, Target};
use crate::identify::{Identifier, Via};
use crate::inspect::{self, InspectorEntry};
use crate::keys::DecalItem;
use crate::run::{self, Place, PreviewRow, Progress, RunConfig, Sorter, valid_container};
use crate::settings::{Settings, SettingsStore};
use crate::window::Window;

/// The verb.
pub const VERB: &str = "ms";

/// The character in the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    /// Its id (its main pack).
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// A container the window offers as source or destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerChoice {
    /// What it is.
    pub place: Place,
    /// Its name.
    pub name: String,
    /// Items in it (packs not counted).
    pub items: usize,
    /// Items it holds at most, 0 when the client was not told.
    pub capacity: i32,
}

/// The plugin as the client loads it.
#[derive(Debug, Default)]
pub struct MudSort {
    settings: Settings,
    store: SettingsStore,
    sorter: Sorter,
    identifier: Identifier,
    me: Option<Me>,
    source: u32,
    dest: u32,
    /// The window (none before `enable`).
    window: Option<Window>,
}

impl Plugin for MudSort {
    fn enable(&mut self, host: &mut Host<'_>, _info: &StartInfo) -> Result<(), String> {
        host.register_command(VERB, commands::DESCRIPTION)?;
        self.window = Some(Window::open(host)?);
        // Enabled while in the world (a reload): pick the character up
        // without the login's chat line and identification.
        self.login(host, false);
        self.refresh_window(host);
        Ok(())
    }

    fn tick(&mut self, host: &mut Host<'_>) {
        if self.me.is_some() {
            self.identifier.tick(host);
            self.sorter.tick(host, &self.identifier);
        }
        let phase = self.sorter.phase();
        if self.window.as_ref().is_some_and(|w| w.needs_refresh(phase)) {
            self.refresh_window(host);
        }
    }

    fn on_event(&mut self, host: &mut Host<'_>, event: Event) {
        if let Event::Ui(UiEvent::Action(action)) = &event {
            self.window_action(host, action);
            return;
        }
        if let Some(window) = self.window.as_mut() {
            window.on_event(host, &event);
        }
        match event {
            Event::Command(call) if call.verb.eq_ignore_ascii_case(VERB) => {
                self.command(host, &call);
            }
            Event::LoginComplete => self.login(host, true),
            Event::Logoff => self.logoff(host),
            Event::ObjectChanged(change) => match change.kind {
                ObjectChangeKind::IdentReceived => self.identifier.received(change.object_id),
                ObjectChangeKind::Moved | ObjectChangeKind::Released => {
                    self.sorter.object_moved(host, change.object_id);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl MudSort {
    /// `CharacterFilter_LoginComplete`: says so, stops any run, loads the
    /// character's settings, queues the whole inventory for identification
    /// when asked, and makes the character both source and destination.
    fn login(&mut self, host: &mut Host<'_>, announce: bool) {
        let Some(character) = host.character() else {
            return;
        };
        if announce {
            chat::say(host, "Plugin online.");
        }
        self.sorter.cancel(&mut self.identifier);
        self.settings = self.store.load(host, &character.name);
        if announce && self.settings.identify_on_login {
            for item in host.owned_items() {
                self.identifier.push_back(item.object_id, Via::World);
            }
        }
        self.source = character.id;
        self.dest = character.id;
        self.me = Some(Me {
            id: character.id,
            name: character.name,
        });
    }

    /// `CharacterFilter_Logoff`.
    fn logoff(&mut self, host: &mut Host<'_>) {
        self.sorter.cancel(&mut self.identifier);
        self.identifier.clear();
        self.store.close();
        self.me = None;
        chat::say(host, "Plugin offline.");
    }

    fn save(&self, host: &mut Host<'_>) {
        self.store.save(host, &self.settings);
    }

    // ---- The window. ----

    /// The window.
    #[must_use]
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    /// Pushes what the window shows now.
    pub fn refresh_window(&mut self, host: &mut Host<'_>) {
        if let Some(mut window) = self.window.take() {
            window.refresh(self, host);
            self.window = Some(window);
        }
    }

    /// One of the window's controls was used.
    fn window_action(&mut self, host: &mut Host<'_>, action: &UiAction) {
        if let Some(mut window) = self.window.take() {
            if window.panel_id() == action.panel {
                window.act(self, host, action);
            }
            self.window = Some(window);
        }
    }

    // ---- What the window binds to. ----

    /// The character in the world.
    #[must_use]
    pub fn me(&self) -> Option<&Me> {
        self.me.as_ref()
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Changes the settings and saves them.
    pub fn update_settings(&mut self, host: &mut Host<'_>, change: impl FnOnce(&mut Settings)) {
        change(&mut self.settings);
        self.save(host);
    }

    /// The source's id, 0 when the last one set was invalid (mudsort's
    /// field then read `invalid!`).
    #[must_use]
    pub fn source(&self) -> u32 {
        self.source
    }

    /// The destination's id, 0 when invalid.
    #[must_use]
    pub fn dest(&self) -> u32 {
        self.dest
    }

    fn checked(&self, host: &mut Host<'_>, id: u32) -> u32 {
        let me = self.me.as_ref().map_or(0, |me| me.id);
        if valid_container(host, id, me) { id } else { 0 }
    }

    /// `setSourceContainer`: `id` when it holds items, else 0 (invalid).
    pub fn set_source(&mut self, host: &mut Host<'_>, id: u32) -> u32 {
        self.source = self.checked(host, id);
        self.source
    }

    /// `setDestContainer`: `id` when it holds items, else 0 (invalid).
    pub fn set_dest(&mut self, host: &mut Host<'_>, id: u32) -> u32 {
        self.dest = self.checked(host, id);
        self.dest
    }

    /// Swaps source and destination.
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.source, &mut self.dest);
    }

    /// The selected object's id, 0 for none.
    fn selection(host: &mut Host<'_>) -> u32 {
        host.selection().selected.unwrap_or(0)
    }

    /// What a run started now would be given.
    #[must_use]
    pub fn run_config(&self) -> RunConfig {
        RunConfig {
            source: self.source,
            dest: self.dest,
            keys: self.settings.keys().keys,
            class_filter: self.settings.class_filter().map(str::to_owned),
            name_filter: self.settings.name_filter().map(str::to_owned),
            start_slot: self.settings.start_slot,
            reverse: self.settings.reverse,
            announce_to: self
                .settings
                .announce_when_done
                .then(|| self.me.as_ref().map(|me| me.name.clone()))
                .flatten(),
        }
    }

    /// Starts a sort (`activate`).
    ///
    /// # Errors
    /// Not in the world, or the source or destination is not one a sort can
    /// use.
    pub fn start(&mut self, host: &mut Host<'_>) -> Result<usize, String> {
        let me = self.me.as_ref().ok_or("not in the world")?.id;
        let config = self.run_config();
        self.sorter.start(host, config, me, &mut self.identifier)
    }

    /// Stops a sort (`cancel`); true when one was under way.
    pub fn stop(&mut self) -> bool {
        self.sorter.cancel(&mut self.identifier)
    }

    /// The run.
    #[must_use]
    pub fn sorter(&self) -> &Sorter {
        &self.sorter
    }

    /// How far the run is.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.sorter.progress(&self.identifier)
    }

    /// The run's rows, in the order shown; empty before the first run.
    #[must_use]
    pub fn rows(&self) -> Vec<PreviewRow> {
        self.sorter.rows()
    }

    /// The names of the keys the rows show values for: the run's while it
    /// has one, else the settings'.
    #[must_use]
    pub fn columns(&self) -> Vec<&'static str> {
        let keys = match self.sorter.config() {
            Some(config) if self.sorter.phase() != run::Phase::Idle => config.keys.clone(),
            _ => self.settings.keys().keys,
        };
        keys.iter()
            .take(run::SHOWN_KEYS)
            .map(|key| key.flag.display_name())
            .collect()
    }

    /// The rows a sort started now would make, without starting it.
    ///
    /// # Errors
    /// As [`Self::start`].
    pub fn preview(&self, host: &mut Host<'_>) -> Result<Vec<PreviewRow>, String> {
        let me = self.me.as_ref().ok_or("not in the world")?.id;
        run::preview(host, &self.run_config(), me)
    }

    /// The containers a sort can use now: the main pack, each carried pack
    /// in pack-slot order, and the open chest.
    #[must_use]
    pub fn containers(&self, host: &mut Host<'_>) -> Vec<ContainerChoice> {
        let Some(me) = &self.me else {
            return Vec::new();
        };
        let owned = host.owned_items();
        let count = |container: u32, items: &[InventoryItem]| {
            items
                .iter()
                .filter(|item| {
                    item.container_id == container && item.object_class != ObjectClass::Container
                })
                .count()
        };
        let mut choices = vec![ContainerChoice {
            place: Place::MainPack(me.id),
            name: me.name.clone(),
            items: count(me.id, &owned),
            capacity: host.object(me.id).map_or(0, |object| object.items_capacity),
        }];
        let mut packs: Vec<&InventoryItem> = owned
            .iter()
            .filter(|item| {
                item.container_id == me.id
                    && item.equipped_location == 0
                    && item.object_class == ObjectClass::Container
            })
            .collect();
        packs.sort_by_key(|pack| pack.container_slot);
        for pack in packs {
            choices.push(ContainerChoice {
                place: Place::SidePack(pack.object_id),
                name: pack.name.clone(),
                items: count(pack.object_id, &owned),
                capacity: pack.items_capacity,
            });
        }
        let chest = host.loot_state().current_container_id;
        if chest != 0 {
            let contents = host.loot_contents();
            let object = host.object(chest);
            choices.push(ContainerChoice {
                place: Place::Chest(chest),
                name: object.as_ref().map_or_else(String::new, |o| o.name.clone()),
                items: count(chest, &contents),
                capacity: object.map_or(0, |o| o.items_capacity),
            });
        }
        choices
    }

    /// The item as the sort reads it: owned, or in the open chest.
    fn read_item(host: &mut Host<'_>, id: u32) -> Option<DecalItem> {
        if let Some(item) = host.owned_items().into_iter().find(|i| i.object_id == id) {
            let properties = host.item_properties(id);
            return Some(DecalItem::from_inventory(&item, properties.as_ref()));
        }
        let item = host
            .loot_contents()
            .into_iter()
            .find(|i| i.object_id == id)?;
        let properties = host.loot_properties(id);
        Some(DecalItem::from_inventory(&item, properties.as_ref()))
    }

    /// The Inspector: every sort key the item has a value for.
    #[must_use]
    pub fn inspect(host: &mut Host<'_>, id: u32) -> Option<Vec<InspectorEntry>> {
        Self::read_item(host, id).map(|item| inspect::inspect(&item))
    }

    /// mudsort's dump of one key for the selected item, to chat
    /// (`propertyDumpSelection`); false when nothing usable is selected or
    /// the code names no key.
    pub fn dump_selection(host: &mut Host<'_>, code: &str) -> bool {
        let Some(flag) = codes::by_code(&code.to_uppercase()) else {
            return false;
        };
        let id = Self::selection(host);
        let Some(item) = Self::read_item(host, id) else {
            return false;
        };
        chat::say(host, &inspect::dump_line(flag, &item));
        true
    }

    // ---- The commands. ----

    fn pack(host: &mut Host<'_>, me: u32, number: i32) -> Option<u32> {
        host.owned_items()
            .into_iter()
            .find(|item| {
                item.container_id == me
                    && item.equipped_location == 0
                    && item.object_class == ObjectClass::Container
                    && item.container_slot.wrapping_add(1) == number
            })
            .map(|item| item.object_id)
    }

    /// `ProcessMSCommand`.
    fn command(&mut self, host: &mut Host<'_>, call: &CommandCall) {
        let command = commands::parse(&call.arguments);
        let Some(me) = self.me.clone() else {
            chat::say(host, "not in the world");
            return;
        };
        match command {
            Command::Help => {
                chat::say(host, commands::HELP_HEADER);
                for line in commands::HELP {
                    chat::say(host, line);
                }
            }
            Command::ClearClassFilter => {
                chat::say(host, "Clearing ObjectClass Filter.");
                self.update_settings(host, |s| s.class_filter.clear());
            }
            Command::SetClassFilter(filter) => {
                chat::say(host, &format!("Setting ObjectClass Filter: {filter}"));
                self.update_settings(host, |s| s.class_filter = filter);
            }
            Command::SetSource(target) => self.set_target(host, &me, target, true),
            Command::SetDest(target) => self.set_target(host, &me, target, false),
            Command::SetFlags(flags) => {
                chat::say(host, &format!("Updated Sort Flags: {flags}"));
                let unknown = crate::sort_string::parse(&flags).unknown;
                if !unknown.is_empty() {
                    chat::say(
                        host,
                        &format!("not a sort key, skipped: {}", unknown.join(", ")),
                    );
                }
                self.update_settings(host, |s| s.sort_string = flags);
            }
            Command::Start => {
                chat::say(host, "Starting Sort...");
                if let Err(error) = self.start(host) {
                    chat::say(host, &format!("cannot sort: {error}"));
                }
            }
            Command::Stop => {
                chat::say(host, "Stopping Sort...");
                self.stop();
            }
            Command::Unknown(_) => {
                chat::say(host, "unknown command; /ms help lists the commands");
            }
        }
    }

    fn set_target(&mut self, host: &mut Host<'_>, me: &Me, target: Target, source: bool) {
        let which = if source { "Source" } else { "Destination" };
        let id = match target {
            Target::Selection => {
                chat::say(host, &format!("Setting {which} Container to Selection..."));
                Self::selection(host)
            }
            Target::Player => {
                chat::say(
                    host,
                    &format!("Setting {which} Container to Player ID: {}", me.id),
                );
                me.id
            }
            Target::Pack(number) => {
                // No such pack: mudsort said nothing and changed nothing.
                let Some(id) = Self::pack(host, me.id, number) else {
                    return;
                };
                chat::say(host, &format!("Setting {which} Container to: {id}..."));
                id
            }
            Target::Id(id) => {
                chat::say(host, &format!("Setting {which} Container to: {id}..."));
                id
            }
            Target::Invalid(text) => {
                chat::say(host, &format!("not a container: {text}"));
                return;
            }
        };
        if source {
            self.set_source(host, id);
        } else {
            self.set_dest(host, id);
        }
    }
}
