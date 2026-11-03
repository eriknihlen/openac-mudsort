//! A small client for the run tests: the SDK's `FakeHost` for the host
//! services (log, chat, commands, the character), with the item and storage
//! imports answered from a world kept here, so a move really moves an item
//! and an appraisal really arrives.
//!
//! Requests are taken in, `Started`, and stay in flight until the test lets
//! the server answer ([`Sim::answer_move`], [`Sim::answer_appraisal`]), or
//! [`Harness::run`] does it for every request.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};

use ac_plugin_abi::Wire;
use ac_plugin_abi::items::{
    InventoryCommandKind, InventoryCompletion, InventoryItem, ItemAmount, ItemResult, ItemState,
    ItemStatus, LootState, MoveRequest, PickupRequest,
};
use ac_plugin_abi::social::SelectionState;
use ac_plugin_abi::storage::{StorageKey, StorageWrite};
use ac_plugin_abi::world::{
    ItemProperties, ObjectChange, ObjectChangeKind, ObjectClass, WorldObject,
};
use mudsort::MudSort;
use openac_plugin::testing::FakeHost;
use openac_plugin::{
    CharacterInfo, CommandCall, Event, Host, Import, Plugin, StartInfo, Transport,
};

/// The character's id and main pack.
pub const ME: u32 = 0x5000_0001;
/// Its name.
pub const NAME: &str = "Acdream";
/// A pack the character carries (pack slot 0).
pub const PACK1: u32 = 0x8000_1001;
/// A second carried pack (pack slot 1).
pub const PACK2: u32 = 0x8000_1002;
/// A chest standing nearby.
pub const CHEST: u32 = 0x7000_0001;
/// Another player.
pub const HORAN: u32 = 0x5000_0002;

/// An item-affecting request the plugin made, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Req {
    /// `identify`.
    Identify(u32),
    /// `identify_loot`.
    IdentifyLoot(u32),
    /// `move_item`.
    Move { id: u32, container: u32, slot: i32 },
    /// `put_in_open_container`.
    Put { id: u32, chest: u32, slot: i32 },
    /// `give_item`.
    Give { id: u32, to: u32 },
    /// `pick_up`.
    PickUp { id: u32, main_pack: bool },
}

impl Req {
    /// The item it is about.
    pub fn id(&self) -> u32 {
        match *self {
            Self::Identify(id) | Self::IdentifyLoot(id) => id,
            Self::Move { id, .. }
            | Self::Put { id, .. }
            | Self::Give { id, .. }
            | Self::PickUp { id, .. } => id,
        }
    }
    /// A move, put, give or pick-up.
    pub fn is_item_move(&self) -> bool {
        !matches!(self, Self::Identify(_) | Self::IdentifyLoot(_))
    }
}

/// The world and the fake host.
#[derive(Debug)]
pub struct Sim {
    pub fake: FakeHost,
    /// What the character owns (main pack, packs, their contents).
    pub owned: Vec<InventoryItem>,
    /// The open chest, 0 for none.
    pub chest: u32,
    /// What it holds.
    pub chest_items: Vec<InventoryItem>,
    /// Every object the client knows.
    pub objects: BTreeMap<u32, WorldObject>,
    /// Properties by id.
    pub properties: BTreeMap<u32, ItemProperties>,
    /// The selection.
    pub selected: Option<u32>,
    /// Storage: `<scope name>/<key>` to text.
    pub storage: BTreeMap<String, String>,
    /// What the next item requests answer instead of starting.
    pub move_answers: VecDeque<ItemResult>,
    /// What the next identify requests answer instead of starting.
    pub identify_answers: VecDeque<ItemStatus>,
    /// The item requests and identifies, in order.
    pub log: Vec<Req>,
    /// The request in flight.
    pub in_flight: Option<Req>,
    /// The appraisal awaited.
    pub awaiting: u32,
    completion: InventoryCompletion,
}

fn object(id: u32, name: &str, class: ObjectClass, owned: bool) -> WorldObject {
    WorldObject {
        object_id: id,
        name: name.to_owned(),
        object_class: class,
        is_owned: owned,
        has_appraisal_data: true,
        stack_size: 1,
        ..WorldObject::default()
    }
}

impl Sim {
    /// A character with two packs, a chest, and another player near.
    pub fn new() -> Self {
        let mut fake = FakeHost::new("openac.mudsort");
        fake.character = Some(CharacterInfo {
            id: ME,
            name: NAME.to_owned(),
            ..CharacterInfo::default()
        });
        let mut sim = Self {
            fake,
            owned: Vec::new(),
            chest: 0,
            chest_items: Vec::new(),
            objects: BTreeMap::new(),
            properties: BTreeMap::new(),
            selected: None,
            storage: BTreeMap::new(),
            move_answers: VecDeque::new(),
            identify_answers: VecDeque::new(),
            log: Vec::new(),
            in_flight: None,
            awaiting: 0,
            completion: InventoryCompletion::default(),
        };
        let mut me = object(ME, NAME, ObjectClass::Player, false);
        me.items_capacity = 102;
        sim.objects.insert(ME, me);
        let mut horan = object(HORAN, "Horan", ObjectClass::Player, false);
        horan.items_capacity = 102;
        sim.objects.insert(HORAN, horan);
        for (slot, id) in [PACK1, PACK2].into_iter().enumerate() {
            let mut pack = object(
                id,
                &format!("Pack {}", slot + 1),
                ObjectClass::Container,
                true,
            );
            pack.items_capacity = 24;
            sim.objects.insert(id, pack);
            sim.owned.push(InventoryItem {
                object_id: id,
                name: format!("Pack {}", slot + 1),
                object_class: ObjectClass::Container,
                container_id: ME,
                container_slot: slot as i32,
                items_capacity: 24,
                ..InventoryItem::default()
            });
        }
        let mut chest = object(CHEST, "Chest", ObjectClass::Container, false);
        chest.items_capacity = 120;
        sim.objects.insert(CHEST, chest);
        sim
    }

    /// Opens the chest.
    pub fn open_chest(&mut self) {
        self.chest = CHEST;
    }

    fn renumber(list: &mut [InventoryItem], container: u32) {
        let mut members: Vec<&mut InventoryItem> = list
            .iter_mut()
            .filter(|item| {
                item.container_id == container && item.object_class != ObjectClass::Container
            })
            .collect();
        members.sort_by_key(|item| item.container_slot);
        for (slot, item) in members.into_iter().enumerate() {
            item.container_slot = slot as i32;
        }
    }

    fn next_slot(list: &[InventoryItem], container: u32) -> i32 {
        list.iter()
            .filter(|item| {
                item.container_id == container && item.object_class != ObjectClass::Container
            })
            .count() as i32
    }

    /// An item in `container` (owned, or the chest's), not appraised when
    /// `appraised` is false. `value` is its value (the `VA` key) and armor
    /// level (`AL`).
    pub fn add_item(
        &mut self,
        id: u32,
        name: &str,
        class: ObjectClass,
        container: u32,
        armor_level: i32,
        appraised: bool,
    ) {
        let in_chest = container == CHEST;
        let list = if in_chest {
            &mut self.chest_items
        } else {
            &mut self.owned
        };
        let slot = Self::next_slot(list, container);
        list.push(InventoryItem {
            object_id: id,
            name: name.to_owned(),
            object_class: class,
            container_id: container,
            container_slot: slot,
            armor_level,
            ..InventoryItem::default()
        });
        let mut world = object(id, name, class, !in_chest);
        world.has_appraisal_data = appraised;
        self.objects.insert(id, world);
        self.properties.insert(
            id,
            ItemProperties {
                ints: vec![(28, armor_level)],
                ..ItemProperties::default()
            },
        );
    }

    /// The names of the items in `container`, in slot order.
    pub fn contents(&self, container: u32) -> Vec<String> {
        let list = if container == CHEST {
            &self.chest_items
        } else {
            &self.owned
        };
        let mut items: Vec<&InventoryItem> = list
            .iter()
            .filter(|item| {
                item.container_id == container && item.object_class != ObjectClass::Container
            })
            .collect();
        items.sort_by_key(|item| item.container_slot);
        items.iter().map(|item| item.name.clone()).collect()
    }

    /// The item moves, puts, gives and pick-ups, in order.
    pub fn moves(&self) -> Vec<Req> {
        self.log
            .iter()
            .filter(|r| r.is_item_move())
            .cloned()
            .collect()
    }

    /// The identifies, in order.
    pub fn identifies(&self) -> Vec<u32> {
        self.log
            .iter()
            .filter(|r| !r.is_item_move())
            .map(Req::id)
            .collect()
    }

    fn take(&mut self, id: u32) -> Option<InventoryItem> {
        if let Some(index) = self.owned.iter().position(|i| i.object_id == id) {
            let item = self.owned.remove(index);
            Self::renumber(&mut self.owned, item.container_id);
            return Some(item);
        }
        let index = self.chest_items.iter().position(|i| i.object_id == id)?;
        let item = self.chest_items.remove(index);
        Self::renumber(&mut self.chest_items, CHEST);
        Some(item)
    }

    fn insert(&mut self, mut item: InventoryItem, container: u32, slot: i32) {
        let id = item.object_id;
        let in_chest = container == CHEST;
        let list = if in_chest {
            &mut self.chest_items
        } else {
            &mut self.owned
        };
        for other in list.iter_mut() {
            if other.container_id == container
                && other.object_class != ObjectClass::Container
                && other.container_slot >= slot
            {
                other.container_slot += 1;
            }
        }
        item.container_id = container;
        item.container_slot = slot;
        list.push(item);
        Self::renumber(list, container);
        if let Some(object) = self.objects.get_mut(&id) {
            object.is_owned = !in_chest;
        }
    }

    /// The server answers the request in flight (`weenie_error` 0 for a
    /// success) and the object's move event is returned.
    pub fn answer_move(&mut self, weenie_error: u32) -> Option<Event> {
        let request = self.in_flight.take()?;
        let id = request.id();
        let kind = match request {
            Req::Move { .. } => InventoryCommandKind::PutInContainer,
            Req::Put { .. } => InventoryCommandKind::PutInContainer,
            Req::Give { .. } => InventoryCommandKind::Give,
            Req::PickUp { .. } => InventoryCommandKind::Pickup,
            _ => InventoryCommandKind::Unknown,
        };
        if weenie_error == 0 {
            match request {
                Req::Move {
                    container, slot, ..
                } => {
                    let item = self.take(id)?;
                    self.insert(item, container, slot);
                }
                Req::Put { slot, .. } => {
                    let item = self.take(id)?;
                    self.insert(item, CHEST, slot);
                }
                Req::Give { .. } => {
                    self.take(id);
                    self.objects.remove(&id);
                }
                Req::PickUp { .. } => {
                    let item = self.take(id)?;
                    let slot = Self::next_slot(&self.owned, ME);
                    self.insert(item, ME, slot);
                }
                _ => {}
            }
        }
        self.completion = InventoryCompletion {
            revision: self.completion.revision + 1,
            kind,
            source_id: id,
            weenie_error,
        };
        Some(Event::ObjectChanged(ObjectChange {
            object_id: id,
            kind: ObjectChangeKind::Moved,
            revision: self.completion.revision,
            changed_fields: 0,
            current: self.objects.get(&id).cloned(),
        }))
    }

    /// The appraisal awaited arrives, and its event is returned.
    pub fn answer_appraisal(&mut self) -> Option<Event> {
        if self.awaiting == 0 {
            return None;
        }
        let id = std::mem::take(&mut self.awaiting);
        let object = self.objects.get_mut(&id)?;
        object.has_appraisal_data = true;
        Some(Event::ObjectChanged(ObjectChange {
            object_id: id,
            kind: ObjectChangeKind::IdentReceived,
            revision: 1,
            changed_fields: 0,
            current: Some(object.clone()),
        }))
    }

    /// The appraisal awaited is given up on by the client (no answer).
    pub fn drop_appraisal(&mut self) {
        self.awaiting = 0;
    }

    fn item_request(&mut self, request: Req) -> ItemResult {
        if let Some(answer) = self.move_answers.pop_front() {
            self.log.push(request);
            return answer;
        }
        if self.in_flight.is_some() {
            self.log.push(request);
            return ItemResult::of(ItemStatus::Busy);
        }
        self.log.push(request.clone());
        self.in_flight = Some(request);
        ItemResult::of(ItemStatus::Started)
    }

    fn identify(&mut self, request: Req) -> ItemResult {
        let id = request.id();
        self.log.push(request);
        if let Some(status) = self.identify_answers.pop_front() {
            return ItemResult::of(status);
        }
        if self.awaiting != 0 {
            return ItemResult::of(ItemStatus::Busy);
        }
        self.awaiting = id;
        ItemResult::of(ItemStatus::Started)
    }
}

fn decode<T: Wire>(bytes: &[u8]) -> T {
    T::from_bytes(bytes).expect("a request the SDK encoded")
}

fn storage_name(key: &StorageKey) -> String {
    format!("{}/{}", key.place.scope_name, key.key)
}

impl Transport for Sim {
    fn call(&mut self, import: Import, bytes: &[u8]) -> Vec<u8> {
        // The fake records every call and answers the host services.
        let base = self.fake.call(import, bytes);
        match import {
            Import::ItemsOwned => self.owned.to_bytes(),
            Import::ItemsState => ItemState {
                is_available: true,
                is_busy: self.in_flight.is_some(),
                last_inventory_completion: self.completion.clone(),
                ..ItemState::default()
            }
            .to_bytes(),
            Import::LootState => {
                let mut state = LootState {
                    is_available: true,
                    current_container_id: self.chest,
                    current_contents_ready: self.chest != 0,
                    last_inventory_completion: self.completion.clone(),
                    ..LootState::default()
                };
                state.appraisal.awaiting_id = self.awaiting;
                state.to_bytes()
            }
            Import::LootContents => if self.chest != 0 {
                self.chest_items.clone()
            } else {
                Vec::new()
            }
            .to_bytes(),
            Import::WorldObject => {
                let id: u32 = decode(bytes);
                self.objects.get(&id).cloned().to_bytes()
            }
            Import::ItemsProperties => {
                let id: u32 = decode(bytes);
                let owned = self.owned.iter().any(|i| i.object_id == id);
                owned
                    .then(|| self.properties.get(&id).cloned())
                    .flatten()
                    .to_bytes()
            }
            Import::LootProperties => {
                let id: u32 = decode(bytes);
                let here = self.chest_items.iter().any(|i| i.object_id == id);
                here.then(|| self.properties.get(&id).cloned())
                    .flatten()
                    .to_bytes()
            }
            Import::WorldIdentify => {
                let id: u32 = decode(bytes);
                self.identify(Req::Identify(id)).to_bytes()
            }
            Import::LootIdentify => {
                let id: u32 = decode(bytes);
                self.identify(Req::IdentifyLoot(id)).to_bytes()
            }
            Import::ItemsMove => {
                let r: MoveRequest = decode(bytes);
                self.item_request(Req::Move {
                    id: r.object_id,
                    container: r.container_id,
                    slot: r.placement,
                })
                .to_bytes()
            }
            Import::LootPut => {
                let r: MoveRequest = decode(bytes);
                self.item_request(Req::Put {
                    id: r.object_id,
                    chest: r.container_id,
                    slot: r.placement,
                })
                .to_bytes()
            }
            Import::ItemsGive => {
                let r: ItemAmount = decode(bytes);
                self.item_request(Req::Give {
                    id: r.object_id,
                    to: r.target_id,
                })
                .to_bytes()
            }
            Import::LootPickup => {
                let r: PickupRequest = decode(bytes);
                self.item_request(Req::PickUp {
                    id: r.object_id,
                    main_pack: r.main_pack,
                })
                .to_bytes()
            }
            Import::SelectionState => SelectionState {
                selected: self.selected,
                previous: None,
            }
            .to_bytes(),
            Import::StorageReadText => {
                let key: StorageKey = decode(bytes);
                let text = self.storage.get(&storage_name(&key)).cloned();
                Ok::<_, String>(text).to_bytes()
            }
            Import::StorageWrite => {
                let write: StorageWrite = decode(bytes);
                let text = String::from_utf8(write.bytes.clone()).expect("UTF-8");
                self.storage.insert(
                    storage_name(&StorageKey {
                        place: write.place,
                        key: write.key,
                    }),
                    text,
                );
                Ok::<(), String>(()).to_bytes()
            }
            _ => base,
        }
    }
}

/// The plugin on the simulated client.
#[derive(Debug)]
pub struct Harness {
    pub sim: Sim,
    pub plugin: MudSort,
}

impl Harness {
    /// Enabled and logged in, with `setup` applied to the world first.
    pub fn logged_in(setup: impl FnOnce(&mut Sim)) -> Self {
        let mut sim = Sim::new();
        setup(&mut sim);
        let mut harness = Self {
            sim,
            plugin: MudSort::default(),
        };
        harness.enable();
        harness.send(Event::LoginComplete);
        harness
    }

    /// Enables the plugin.
    pub fn enable(&mut self) {
        let info = StartInfo {
            plugin_id: "openac.mudsort".to_owned(),
            api_version: openac_plugin::API_CURRENT.to_string(),
            client_version: "0.1.10".to_owned(),
        };
        self.plugin
            .enable(&mut Host::with_transport(&mut self.sim), &info)
            .expect("enabled");
    }

    /// One tick.
    pub fn tick(&mut self) {
        self.plugin.tick(&mut Host::with_transport(&mut self.sim));
    }

    /// Delivers an event.
    pub fn send(&mut self, event: Event) {
        self.plugin
            .on_event(&mut Host::with_transport(&mut self.sim), event);
    }

    /// Runs `f` with a host.
    pub fn with_host<R>(&mut self, f: impl FnOnce(&mut MudSort, &mut Host<'_>) -> R) -> R {
        f(&mut self.plugin, &mut Host::with_transport(&mut self.sim))
    }

    /// Types `/ms <arguments>`.
    pub fn ms(&mut self, arguments: &str) {
        self.send(Event::Command(CommandCall {
            verb: "ms".to_owned(),
            arguments: arguments.to_owned(),
            raw: format!("/ms {arguments}"),
        }));
    }

    /// The chat lines posted, in order, as the chat shows them (the
    /// backslash escaping a `<` dropped).
    pub fn chat(&self) -> Vec<String> {
        self.sim
            .fake
            .posted
            .iter()
            .map(|p| p.text.replace("\\<", "<"))
            .collect()
    }

    /// The window's panel.
    pub fn panel(&self) -> u32 {
        self.plugin.window().expect("the window").panel_id()
    }

    /// The player uses one of the window's controls: its handler fires as
    /// the client fires it, through the client's UI state, which drops a
    /// handler the window does not bind or an argument of another kind (so
    /// the test fails here).
    pub fn act(&mut self, handler: &str, value: ac_plugin_ui::ActionValue) {
        let panel = self.panel();
        self.sim.fake.ui.input(ac_plugin_ui::UiInput::Action {
            panel,
            handler: handler.to_owned(),
            value,
        });
        let events = self.sim.fake.ui.take_events();
        assert!(
            !events.is_empty(),
            "the window binds no handler {handler} taking that argument"
        );
        for (_, event) in events {
            self.send(ac_plugin_abi::ui_host::event(event));
        }
    }

    /// A button, tab or other control without an argument.
    pub fn click(&mut self, handler: &str) {
        self.act(handler, ac_plugin_ui::ActionValue::None);
    }

    /// Typed text, a menu's pick.
    pub fn type_text(&mut self, handler: &str, text: &str) {
        self.act(handler, ac_plugin_ui::ActionValue::Text(text.to_owned()));
    }

    /// The value last pushed for a window binding.
    pub fn value(&self, name: &str) -> openac_plugin::ui::TypedValue {
        self.sim
            .fake
            .panel_typed_value(self.panel(), name)
            .unwrap_or_else(|| panic!("no value pushed for {name}"))
    }

    /// A text binding's value.
    pub fn text(&self, name: &str) -> String {
        match self.value(name) {
            openac_plugin::ui::TypedValue::Text(text) => text,
            other => panic!("{name} is {other:?}"),
        }
    }

    /// A text-list binding's value.
    pub fn texts(&self, name: &str) -> Vec<String> {
        match self.value(name) {
            openac_plugin::ui::TypedValue::TextList(list) => list,
            other => panic!("{name} is {other:?}"),
        }
    }

    /// A bool binding's value.
    pub fn flag(&self, name: &str) -> bool {
        match self.value(name) {
            openac_plugin::ui::TypedValue::Bool(on) => on,
            other => panic!("{name} is {other:?}"),
        }
    }

    /// The values of one field of a list's items (a `<repeat>`'s rows), as
    /// text.
    pub fn item_field(&self, list: &str, field: &str) -> Vec<String> {
        let panel = self.panel();
        let view = self
            .sim
            .fake
            .ui
            .panels()
            .find(|p| p.id == panel)
            .expect("the panel");
        match view.values.get(list) {
            Some(ac_plugin_ui::UiValue::Items(items)) => items
                .iter()
                .map(|item| match item.values.get(field) {
                    Some(ac_plugin_ui::UiValue::Text(text)) => text.clone(),
                    Some(other) => format!("{other:?}"),
                    None => String::new(),
                })
                .collect(),
            other => panic!("{list} is {other:?}"),
        }
    }

    /// The settings as saved in storage.
    pub fn saved(&self) -> mudsort::settings::Settings {
        let text = self
            .sim
            .storage
            .iter()
            .find(|(key, _)| key.ends_with(mudsort::settings::SETTINGS_KEY))
            .map(|(_, text)| text.clone())
            .expect("settings saved");
        mudsort::settings::Settings::from_json(&text).expect("settings")
    }

    /// Ticks until the run is no longer running, answering every request
    /// in flight with a success after each tick; at most `ticks` ticks.
    pub fn run(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.tick();
            if let Some(event) = self.sim.answer_appraisal() {
                self.send(event);
            }
            if let Some(event) = self.sim.answer_move(0) {
                self.send(event);
            }
            if !self.plugin.sorter().phase().is_running() {
                return;
            }
        }
    }
}
