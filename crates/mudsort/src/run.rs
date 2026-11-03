//! The sort run: mudsort's state machine (`PluginCore.cs`, `activate`,
//! `cancel` and `Current_RenderFrame_Sort`), driven by the plugin's tick
//! instead of Decal's render frame.
//!
//! - **Identifying** (`INITIATED`): every selected item without an
//!   appraisal is identified first, one request at a time.
//! - **Building the list** (`BUILDING_LIST`): the items are read afresh and
//!   sorted ([`crate::sort::sort`]).
//! - **Moving** (`MOVING_ITEMS`): one item at a time, each to the same slot
//!   of the destination; the next only once the client says the last one
//!   finished (MossTank's `stack_cram.rs` controller: one move, then wait for
//!   `last_inventory_completion` to change or the client to stop being
//!   busy).
//! - **Done**: chat lines, and `/tell <self>, done sorting!` when asked.
//!
//! The run is plain data for the window to show: [`Sorter::progress`] and
//! [`Sorter::rows`], the exact move order with each row's state.

use std::collections::HashMap;

use openac_plugin::Host;
use openac_plugin::items::{InventoryItem, ItemResult, ItemStatus};
use openac_plugin::world::ObjectClass;

use crate::chat;
use crate::identify::{self, Identifier, Via};
use crate::inspect::direct_value;
use crate::keys::{DecalItem, ItemView, key};
use crate::select::select;
use crate::sort::sort;
use crate::sort_string::SortKey;

/// How many `Busy` answers one row gets before it is given up on: MossTank's
/// `AUTHENTIC_ABANDON_ATTEMPTS` (`stack_cram.rs`), the refusals it allows
/// one stack or cram pair.
pub const MAX_BUSY_ANSWERS: u32 = 80;

/// How many sort keys a row shows.
pub const SHOWN_KEYS: usize = 3;

/// A container a sort reads from or puts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The character's main pack (the character's id).
    MainPack(u32),
    /// A pack the character carries.
    SidePack(u32),
    /// The open outside container (a chest).
    Chest(u32),
    /// Another player: items are given (`GiveItem`), not placed.
    Player(u32),
}

impl Place {
    /// Its object id.
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::MainPack(id) | Self::SidePack(id) | Self::Chest(id) | Self::Player(id) => id,
        }
    }

    /// The open chest.
    #[must_use]
    pub const fn is_chest(self) -> bool {
        matches!(self, Self::Chest(_))
    }
}

/// What `id` is to the character `me`: its main pack, a pack it carries,
/// the open chest, another player; `None` for anything else.
#[must_use]
pub fn classify(host: &mut Host<'_>, id: u32, me: u32) -> Option<Place> {
    if id == 0 {
        return None;
    }
    if id == me {
        return Some(Place::MainPack(id));
    }
    if host.loot_state().current_container_id == id {
        return Some(Place::Chest(id));
    }
    let object = host.object(id)?;
    match object.object_class {
        ObjectClass::Container if object.is_owned => Some(Place::SidePack(id)),
        ObjectClass::Player => Some(Place::Player(id)),
        _ => None,
    }
}

/// Whether `id` may be a source or destination (`setSourceContainer`,
/// `setDestContainer`): an object holding items (`ItemSlots` above 0). The
/// character always is; a player is too, as mudsort's give to a player
/// relies on.
#[must_use]
pub fn valid_container(host: &mut Host<'_>, id: u32, me: u32) -> bool {
    if id == 0 {
        return false;
    }
    id == me
        || host.object(id).is_some_and(|object| {
            object.items_capacity > 0 || object.object_class == ObjectClass::Player
        })
}

/// Where a run stands (mudsort's `State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// Nothing running (`IDLE`).
    #[default]
    Idle,
    /// Identifying the items (`INITIATED`).
    Identifying,
    /// Sorting them (`BUILDING_LIST`).
    BuildingList,
    /// Moving them (`MOVING_ITEMS`).
    Moving,
    /// Finished.
    Done,
}

impl Phase {
    /// The progress bar's text in mudsort (`PreText`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Identifying => "identifying...",
            Self::BuildingList => "building list...",
            Self::Moving => "working...",
            Self::Done => "done!",
        }
    }

    /// A run is under way.
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Identifying | Self::BuildingList | Self::Moving)
    }
}

/// A row's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Waiting for its appraisal.
    NeedsId,
    /// Ready to move.
    Ready,
    /// Its move is in flight.
    Moving,
    /// Moved.
    Moved,
    /// Its move was refused or given up on.
    Failed,
}

impl RowState {
    /// Its label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NeedsId => "Needs ID",
            Self::Ready => "Ready",
            Self::Moving => "Moving",
            Self::Moved => "Moved",
            Self::Failed => "Failed",
        }
    }
}

/// One row of the preview: an item, in move order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRow {
    /// The item's id.
    pub id: u32,
    /// Its name.
    pub name: String,
    /// Its values for the first [`SHOWN_KEYS`] sort keys, as mudsort wrote
    /// them.
    pub values: Vec<String>,
    /// Its state.
    pub state: RowState,
    /// Why it failed, or that it goes unidentified.
    pub note: Option<String>,
}

/// The item a run is working on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentItem {
    /// Its id.
    pub id: u32,
    /// Its name.
    pub name: String,
}

/// How far a run is (mudsort's progress bar).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    /// The phase.
    pub phase: Phase,
    /// Identified items while identifying; finished rows (moved or failed)
    /// while moving.
    pub done: usize,
    /// The items to identify, or the rows to move.
    pub total: usize,
    /// The item being identified or moved.
    pub current: Option<CurrentItem>,
}

/// What a run sorts, from where to where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    /// The source's id.
    pub source: u32,
    /// The destination's id.
    pub dest: u32,
    /// The sort keys.
    pub keys: Vec<SortKey>,
    /// Object class name prefix, ignoring case.
    pub class_filter: Option<String>,
    /// Item name part, ignoring case.
    pub name_filter: Option<String>,
    /// The slot every item is put at.
    pub start_slot: i32,
    /// Reverse the whole list.
    pub reverse: bool,
    /// Whom to `/tell ..., done sorting!` at the end (the character's own
    /// name), `None` for no tell.
    pub announce_to: Option<String>,
}

/// Which request a row needs next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Out of the open chest into the main pack (the first of two steps
    /// for a chest item going anywhere but the chest).
    PickUp,
    /// Into the destination.
    Place,
}

#[derive(Debug, Clone)]
struct Row {
    item: DecalItem,
    state: RowState,
    note: Option<String>,
    step: Step,
    busy_answers: u32,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    row: usize,
    id: u32,
}

macro_rules! delegate_item_view {
    ($ty:ty, $field:ident) => {
        impl ItemView for $ty {
            fn id(&self) -> u32 {
                self.$field.id()
            }
            fn object_class(&self) -> ObjectClass {
                self.$field.object_class()
            }
            fn container(&self) -> u32 {
                self.$field.container()
            }
            fn long(&self, key: u32) -> i32 {
                self.$field.long(key)
            }
            fn double(&self, key: u32) -> f64 {
                self.$field.double(key)
            }
            fn string(&self, key: u32) -> &str {
                self.$field.string(key)
            }
            fn bool(&self, key: u32) -> bool {
                self.$field.bool(key)
            }
            fn spells(&self) -> &[u32] {
                self.$field.spells()
            }
        }
    };
}

delegate_item_view!(Row, item);

/// The items a sort from `source` takes, read with their properties.
#[must_use]
pub fn selected_items(host: &mut Host<'_>, source: Place, config: &RunConfig) -> Vec<DecalItem> {
    let (listed, in_chest) = match source {
        Place::Chest(_) => (host.loot_contents(), true),
        _ => (host.owned_items(), false),
    };
    let candidates: Vec<(InventoryItem, DecalItem)> = listed
        .into_iter()
        .filter(|item| item.container_id == source.id())
        .map(|item| {
            let bare = DecalItem::from_inventory(&item, None);
            (item, bare)
        })
        .collect();
    let bare: Vec<DecalItem> = candidates.iter().map(|(_, bare)| bare.clone()).collect();
    let name_filter = config.name_filter.as_deref().map(str::to_lowercase);
    let chosen: Vec<u32> = select(&bare, source.id(), config.class_filter.as_deref())
        .into_iter()
        .filter(|item| {
            name_filter
                .as_deref()
                .is_none_or(|filter| item.string(key::NAME).to_lowercase().contains(filter))
        })
        .map(ItemView::id)
        .collect();
    candidates
        .into_iter()
        .filter(|(item, _)| chosen.contains(&item.object_id))
        .map(|(item, _)| {
            let properties = if in_chest {
                host.loot_properties(item.object_id)
            } else {
                host.item_properties(item.object_id)
            };
            DecalItem::from_inventory(&item, properties.as_ref())
        })
        .collect()
}

fn row_view(row: &Row, keys: &[SortKey]) -> PreviewRow {
    PreviewRow {
        id: row.item.id,
        name: row.item.string(key::NAME).to_owned(),
        values: keys
            .iter()
            .take(SHOWN_KEYS)
            .map(|key| direct_value(key.flag, &row.item))
            .collect(),
        state: row.state,
        note: row.note.clone(),
    }
}

/// The run.
#[derive(Debug, Default)]
pub struct Sorter {
    phase: Phase,
    config: Option<RunConfig>,
    source: Option<Place>,
    dest: Option<Place>,
    /// The rows in the order shown: the order known so far while
    /// identifying, the move order from building the list on.
    rows: Vec<Row>,
    /// The selected ids in the order selected: what the sort starts from.
    selection: Vec<u32>,
    next: usize,
    pending: Option<Pending>,
    observed_revision: i64,
}

impl Sorter {
    /// Idle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The phase.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The run's configuration, while it has one.
    #[must_use]
    pub fn config(&self) -> Option<&RunConfig> {
        self.config.as_ref()
    }

    /// Starts a run (`activate`), cancelling one under way; says how many
    /// items it took ("N items added to sort list...").
    ///
    /// # Errors
    /// The source is not the character, one of its packs or the open
    /// chest, or the destination is none of those nor a player.
    pub fn start(
        &mut self,
        host: &mut Host<'_>,
        config: RunConfig,
        me: u32,
        identifier: &mut Identifier,
    ) -> Result<usize, String> {
        self.cancel(identifier);
        let source = classify(host, config.source, me)
            .filter(|place| !matches!(place, Place::Player(_)))
            .ok_or("the source is not your pack, one of your packs or the open chest")?;
        let dest = classify(host, config.dest, me).ok_or(
            "the destination is not your pack, one of your packs, the open chest or a player",
        )?;
        let items = selected_items(host, source, &config);
        let in_chest = source.is_chest();
        let first_step = if in_chest && !dest.is_chest() {
            Step::PickUp
        } else {
            Step::Place
        };
        let via = if in_chest { Via::Loot } else { Via::World };
        let mut wanted = Vec::new();
        let rows: Vec<Row> = items
            .into_iter()
            .map(|item| {
                let needs_id = !identify::has_appraisal(host, item.id);
                if needs_id {
                    wanted.push((item.id, via));
                }
                Row {
                    item,
                    state: if needs_id {
                        RowState::NeedsId
                    } else {
                        RowState::Ready
                    },
                    note: None,
                    step: first_step,
                    busy_answers: 0,
                }
            })
            .collect();
        identifier.push_front(&wanted);
        self.selection = rows.iter().map(|row| row.item.id).collect();
        // The order known so far, for the preview; without keys mudsort
        // queues nothing, so the selection order stands until then.
        self.rows = if config.keys.is_empty() {
            rows
        } else {
            sort(rows, &config.keys, config.reverse)
        };
        self.observed_revision = host.item_state().last_inventory_completion.revision;
        self.next = 0;
        self.pending = None;
        self.source = Some(source);
        self.dest = Some(dest);
        self.config = Some(config);
        self.phase = Phase::Identifying;
        chat::say(
            host,
            &format!("{} items added to sort list...", self.rows.len()),
        );
        Ok(self.rows.len())
    }

    /// Stops a run (`cancel`): nothing more is asked of the client; a
    /// request already sent is left to finish. True when a run was under
    /// way.
    pub fn cancel(&mut self, identifier: &mut Identifier) -> bool {
        let was_running = self.phase.is_running();
        if was_running {
            identifier.remove(&self.selection);
            for row in &mut self.rows {
                if row.state == RowState::Moving {
                    row.state = RowState::Ready;
                }
            }
            self.phase = Phase::Idle;
        }
        self.pending = None;
        was_running
    }

    /// One tick.
    pub fn tick(&mut self, host: &mut Host<'_>, identifier: &Identifier) {
        match self.phase {
            Phase::Identifying => self.tick_identifying(identifier),
            Phase::BuildingList => self.build_list(host),
            Phase::Moving => self.tick_moving(host),
            Phase::Idle | Phase::Done => {}
        }
    }

    /// An object moved or left: when it is the item in flight, the run
    /// looks at once whether its move finished.
    pub fn object_moved(&mut self, host: &mut Host<'_>, id: u32) {
        if self.phase == Phase::Moving && self.pending.is_some_and(|p| p.id == id) {
            self.tick_moving(host);
        }
    }

    fn tick_identifying(&mut self, identifier: &Identifier) {
        for row in &mut self.rows {
            if row.state == RowState::NeedsId && !identifier.is_pending(row.item.id) {
                row.state = RowState::Ready;
                if identifier.gave_up(row.item.id) {
                    row.note = Some("could not be identified".to_owned());
                }
            }
        }
        if self.rows.iter().all(|row| row.state != RowState::NeedsId) {
            self.phase = Phase::BuildingList;
        }
    }

    fn build_list(&mut self, host: &mut Host<'_>) {
        let (Some(config), Some(source)) = (self.config.clone(), self.source) else {
            self.phase = Phase::Idle;
            return;
        };
        let mut old: HashMap<u32, Row> =
            self.rows.drain(..).map(|row| (row.item.id, row)).collect();
        // Read afresh: the appraisals arrived since the selection.
        let mut fresh: HashMap<u32, DecalItem> = selected_items(host, source, &config)
            .into_iter()
            .map(|item| (item.id, item))
            .collect();
        let rows: Vec<Row> = self
            .selection
            .iter()
            .filter_map(|id| {
                let mut row = old.remove(id)?;
                row.item = fresh.remove(id)?;
                Some(row)
            })
            .collect();
        self.rows = sort(rows, &config.keys, config.reverse);
        self.next = 0;
        chat::say(host, &format!("{} items in queue...", self.rows.len()));
        self.phase = Phase::Moving;
    }

    fn tick_moving(&mut self, host: &mut Host<'_>) {
        let state = host.item_state();
        if let Some(pending) = self.pending {
            let completion = &state.last_inventory_completion;
            let answer = if completion.revision != self.observed_revision {
                self.observed_revision = completion.revision;
                (completion.source_id == pending.id).then_some(completion.weenie_error)
            } else {
                None
            };
            match answer {
                Some(0) => self.step_done(pending.row),
                Some(error) => self.fail(
                    pending.row,
                    format!("the server refused it (error {error:#X})"),
                ),
                // No longer busy and no word: done, as stack_cram takes it.
                None if !state.is_busy => self.step_done(pending.row),
                None => return,
            }
            self.pending = None;
            if state.is_busy {
                return;
            }
        } else if state.is_busy {
            return;
        }
        self.issue(host);
    }

    /// Asks for the next request, skipping rows that finish at once.
    fn issue(&mut self, host: &mut Host<'_>) {
        let (Some(config), Some(dest)) = (self.config.clone(), self.dest) else {
            return;
        };
        loop {
            let Some(row) = self.rows.get(self.next) else {
                self.finish(host, &config);
                return;
            };
            if matches!(row.state, RowState::Moved | RowState::Failed) {
                self.next += 1;
                continue;
            }
            let id = row.item.id;
            let result = request(host, id, row.step, dest, config.start_slot);
            match result.status {
                ItemStatus::Started => {
                    self.rows[self.next].state = RowState::Moving;
                    self.pending = Some(Pending { row: self.next, id });
                    return;
                }
                ItemStatus::Completed => self.step_done(self.next),
                ItemStatus::Busy => {
                    let row = &mut self.rows[self.next];
                    row.busy_answers += 1;
                    if row.busy_answers <= MAX_BUSY_ANSWERS {
                        return;
                    }
                    self.fail(self.next, "the client stayed busy".to_owned());
                }
                other => {
                    let why = result.notice.unwrap_or_else(|| format!("{other:?}"));
                    self.fail(self.next, why);
                }
            }
        }
    }

    fn step_done(&mut self, index: usize) {
        let Some(row) = self.rows.get_mut(index) else {
            return;
        };
        if row.step == Step::PickUp {
            row.step = Step::Place;
            row.state = RowState::Ready;
        } else {
            row.state = RowState::Moved;
            row.note = None;
        }
    }

    fn fail(&mut self, index: usize, why: String) {
        if let Some(row) = self.rows.get_mut(index) {
            row.state = RowState::Failed;
            row.note = Some(why);
        }
    }

    fn finish(&mut self, host: &mut Host<'_>, config: &RunConfig) {
        self.phase = Phase::Done;
        self.pending = None;
        if let Some(name) = &config.announce_to {
            host.chat_submit(&format!("/tell {name}, done sorting!"));
        }
        chat::say(host, "done sorting items!");
    }

    /// How far the run is.
    #[must_use]
    pub fn progress(&self, identifier: &Identifier) -> Progress {
        let current = |id: u32| {
            self.rows
                .iter()
                .find(|row| row.item.id == id)
                .map(|row| CurrentItem {
                    id,
                    name: row.item.string(key::NAME).to_owned(),
                })
        };
        match self.phase {
            Phase::Identifying => Progress {
                phase: self.phase,
                done: self
                    .rows
                    .iter()
                    .filter(|row| row.state != RowState::NeedsId)
                    .count(),
                total: self.rows.len(),
                current: identifier.awaiting().and_then(current),
            },
            Phase::BuildingList | Phase::Moving | Phase::Done => Progress {
                phase: self.phase,
                done: self
                    .rows
                    .iter()
                    .filter(|row| matches!(row.state, RowState::Moved | RowState::Failed))
                    .count(),
                total: self.rows.len(),
                current: (self.phase == Phase::Moving)
                    .then(|| self.rows.get(self.next).map(|row| row.item.id))
                    .flatten()
                    .and_then(current),
            },
            Phase::Idle => Progress::default(),
        }
    }

    /// The rows, in the order shown (the move order once the list is
    /// built).
    #[must_use]
    pub fn rows(&self) -> Vec<PreviewRow> {
        let keys = self.config.as_ref().map_or(&[][..], |c| &c.keys[..]);
        self.rows.iter().map(|row| row_view(row, keys)).collect()
    }
}

/// The request that takes a row's next step to `dest` at `slot`: mudsort's
/// `MoveItem(id, dest, slot, true)` for one of the character's own
/// containers, a put into the open chest, `GiveItem` to a player; a chest
/// item's first step is a pick-up into the main pack (`DIVERGENCES.md`,
/// MS-7).
fn request(host: &mut Host<'_>, id: u32, step: Step, dest: Place, slot: i32) -> ItemResult {
    if step == Step::PickUp {
        return host.pick_up(id, true);
    }
    match dest {
        Place::MainPack(container) | Place::SidePack(container) => {
            host.move_item(id, container, 0, slot)
        }
        Place::Chest(chest) => host.put_in_open_container(id, chest, 0, slot),
        Place::Player(player) => host.give_item(id, player, 0),
    }
}

/// The rows a run would make now, without starting one: the selection in
/// the order known so far, rows without an appraisal marked as needing one.
///
/// # Errors
/// As [`Sorter::start`], for the source.
pub fn preview(
    host: &mut Host<'_>,
    config: &RunConfig,
    me: u32,
) -> Result<Vec<PreviewRow>, String> {
    let source = classify(host, config.source, me)
        .filter(|place| !matches!(place, Place::Player(_)))
        .ok_or("the source is not your pack, one of your packs or the open chest")?;
    let rows: Vec<Row> = selected_items(host, source, config)
        .into_iter()
        .map(|item| Row {
            state: if identify::has_appraisal(host, item.id) {
                RowState::Ready
            } else {
                RowState::NeedsId
            },
            item,
            note: None,
            step: Step::Place,
            busy_answers: 0,
        })
        .collect();
    let rows = if config.keys.is_empty() {
        rows
    } else {
        sort(rows, &config.keys, config.reverse)
    };
    Ok(rows.iter().map(|row| row_view(row, &config.keys)).collect())
}
