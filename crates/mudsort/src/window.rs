//! The window: one themed panel (`markup/mudsort.xml`) with four tabs over
//! the plugin's view model, [`MudSort`]: Sort (from, to, the filters, the
//! recipe at a glance, the live preview of the move order, the meter and
//! the one Start/Stop button), Recipe, Presets and Options (with the
//! Inspector, which replaces mudsort's property dump to chat).
//!
//! The SDK pushes values: [`Window::refresh`] computes every bound value and
//! the panel sends only what changed. It runs after each control the player
//! uses, and on a tick when something the window shows may have changed (an
//! inventory, selection or theme event, a command, a run under way); a tick
//! with nothing new sends nothing.

use openac_plugin::ui::{
    ActionValue, Color, Item, ItemType, Items, Panel, PanelSpec, TypedBindingKind as K, TypedValue,
    UiAction,
};
use openac_plugin::world::{ObjectChangeKind, ObjectClass};
use openac_plugin::{Event, Host};

use crate::chat;
use crate::codes::{self, Category, Flag};
use crate::identify;
use crate::plugin::{ContainerChoice, MudSort};
use crate::run::{self, Phase, Place, PreviewRow, RowState};
use crate::settings::Preset;
use crate::sort_string::{self, SortKey};

/// The window's markup.
pub const MARKUP: &str = include_str!("../markup/mudsort.xml");
/// The window's id.
pub const WINDOW_ID: &str = "main";
/// The window's title and shelf name.
pub const TITLE: &str = "MudSort";
/// The shelf icon: mudsort's own view icon (10706 in the image table).
pub const ICON: u32 = 10706;

/// The To menu's entry that gives the items to the selected player.
pub const GIVE_TO_PLAYER: &str = "Give to selected player";

/// The class filter menu: mudsort's choices (`mainView.xml`,
/// `cmbObjClassFilters`) with its group headings, which pick nothing.
pub const CLASS_FILTERS: [&str; 27] = [
    "None",
    "— Equipment —",
    "Armor",
    "Clothing",
    "Jewelry",
    "— Weapons —",
    "MeleeWeapon",
    "MissileWeapon",
    "WandStaffOrb",
    "— Crafting —",
    "Food",
    "Key",
    "Lockpick",
    "Plant",
    "Salvage",
    "— Currency —",
    "Money",
    "TradeNote",
    "— Miscellaneous —",
    "Book",
    "Gem",
    "Scroll",
    "SpellComponent",
    "HealingKit",
    "ManaStone",
    "Misc",
    "Unknown",
];

/// What the class filter menu shows for a filter set by `/ms set ocfilter`
/// that is not one of its choices.
pub const CUSTOM_PREFIX: &str = "Custom: ";

/// The key picker's categories, in the order of its segmented choice.
pub const CATEGORIES: [&str; 7] = [
    "Common", "All", "Text", "Number", "Decimal", "Yes/No", "Computed",
];

/// A row waiting for its appraisal: amber, as in the design.
pub const NEEDS_ID_COLOR: u32 = 0xE0_A4_58;
/// A row whose move failed: red, as the design's remove buttons.
pub const FAILED_COLOR: u32 = 0xE0_8A_7A;

/// The window's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// From, to, filters, preview, Start.
    #[default]
    Sort,
    /// The keys.
    Recipe,
    /// Named recipes.
    Presets,
    /// The switches and the Inspector.
    Options,
}

/// The theme colours the window paints rows with, `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Body text.
    pub text: u32,
    /// Captions and hints.
    pub muted: u32,
    /// Highlights.
    pub accent: u32,
    /// Edges.
    pub border: u32,
}

impl Palette {
    /// The design's brass palette, for Classic (which has no palette).
    pub const CLASSIC: Self = Self {
        text: 0xE9_E2_D5,
        muted: 0xB2_A5_8E,
        accent: 0xC9_A6_65,
        border: 0x4C_43_35,
    };

    fn rgb(color: &Color) -> u32 {
        (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
    }

    /// A theme's colours.
    #[must_use]
    pub fn from_colors(text: &Color, muted: &Color, accent: &Color, border: &Color) -> Self {
        Self {
            text: Self::rgb(text),
            muted: Self::rgb(muted),
            accent: Self::rgb(accent),
            border: Self::rgb(border),
        }
    }

    fn of_theme(host: &mut Host<'_>) -> Self {
        host.theme().palette.map_or(Self::CLASSIC, |p| {
            Self::from_colors(&p.text, &p.muted, &p.accent, &p.border)
        })
    }
}

/// A row state's colour.
#[must_use]
pub fn state_color(state: RowState, palette: &Palette) -> u32 {
    match state {
        RowState::NeedsId => NEEDS_ID_COLOR,
        RowState::Ready => palette.text,
        RowState::Moving => palette.accent,
        RowState::Moved => palette.muted,
        RowState::Failed => FAILED_COLOR,
    }
}

/// A category's name in the key picker.
#[must_use]
pub fn category_name(category: Category) -> &'static str {
    match category {
        Category::Text => "Text",
        Category::Number => "Number",
        Category::Decimal => "Decimal",
        Category::YesNo => "Yes/No",
        Category::Computed => "Computed",
    }
}

/// Whether the key picker lists `flag` under category `index` of
/// [`CATEGORIES`].
#[must_use]
pub fn in_category(flag: &Flag, index: usize) -> bool {
    match index {
        0 => flag.common,
        1 => true,
        _ => CATEGORIES.get(index) == Some(&category_name(flag.category())),
    }
}

/// A container as the From and To menus show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Its id.
    pub id: u32,
    /// The menu's line: "Main pack · 41 of 102".
    pub label: String,
    /// The line without the count: "Main pack", what the meter calls it.
    pub stem: String,
}

fn fill(items: usize, capacity: i32) -> String {
    if capacity > 0 {
        format!("{items} of {capacity}")
    } else {
        items.to_string()
    }
}

/// The menus' lines for the containers: "Main pack · 41 of 102", "Pack 2 ·
/// Armor Sack · 9 of 24" (packs numbered in pack-slot order, as `/ms set
/// dest pack N`), "Open: Storage Chest".
#[must_use]
pub fn choices(containers: &[ContainerChoice]) -> Vec<Choice> {
    let mut pack = 0;
    containers
        .iter()
        .map(|c| {
            let count = fill(c.items, c.capacity);
            let (stem, label) = match c.place {
                Place::MainPack(_) => ("Main pack".to_owned(), None),
                Place::SidePack(_) => {
                    pack += 1;
                    (format!("Pack {pack} · {}", c.name), None)
                }
                Place::Chest(_) => {
                    let stem = format!("Open: {}", c.name);
                    (stem.clone(), Some(stem))
                }
                Place::Player(_) => (c.name.clone(), Some(c.name.clone())),
            };
            Choice {
                id: c.place.id(),
                label: label.unwrap_or_else(|| format!("{stem} · {count}")),
                stem,
            }
        })
        .collect()
}

/// The container a menu line names: the line itself, or the same container
/// with another count (its contents changed while the menu was open).
#[must_use]
pub fn find_choice<'a>(choices: &'a [Choice], text: &str) -> Option<&'a Choice> {
    choices.iter().find(|c| c.label == text).or_else(|| {
        choices
            .iter()
            .find(|c| c.stem == text || text.starts_with(&format!("{} · ", c.stem)))
    })
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "item" } else { "items" }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(done: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn index(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(i32::MAX)
}

fn text_of(value: &ActionValue) -> &str {
    match value {
        ActionValue::Text(text) => text,
        _ => "",
    }
}

fn int_of(value: &ActionValue) -> Option<i32> {
    match value {
        ActionValue::Int(v) => Some(*v),
        _ => None,
    }
}

fn direction(key: &SortKey) -> &'static str {
    if key.descending {
        "High to low"
    } else {
        "Low to high"
    }
}

/// Every name the markup binds outside its lists, with its kind.
pub const BINDINGS: &[(&str, K)] = &[
    // Tabs.
    ("SortTab", K::Bool),
    ("RecipeTab", K::Bool),
    ("PresetsTab", K::Bool),
    ("OptionsTab", K::Bool),
    ("ShowSort", K::Action),
    ("ShowRecipe", K::Action),
    ("ShowPresets", K::Action),
    ("ShowOptions", K::Action),
    ("Status", K::Text),
    // Sort: from and to.
    ("FromItems", K::TextList),
    ("FromSelected", K::Text),
    ("SetFrom", K::TextAction),
    ("FromSelection", K::Action),
    ("Swap", K::Action),
    ("ToItems", K::TextList),
    ("ToSelected", K::Text),
    ("SetTo", K::TextAction),
    ("ToSelection", K::Action),
    // Sort: filters.
    ("ClassItems", K::TextList),
    ("ClassSelected", K::Text),
    ("SetClass", K::TextAction),
    ("NameFilter", K::Text),
    ("SetNameFilter", K::TextAction),
    ("StartSlot", K::Int),
    ("SetStartSlot", K::IntAction),
    ("EditRecipe", K::Action),
    // Sort: the preview.
    ("PreviewSummary", K::Text),
    ("PreviewSelected", K::Int),
    ("SelectRow", K::IntAction),
    ("RowNumbers", K::TextList),
    ("RowIcons", K::UintList),
    ("RowNames", K::TextList),
    ("Column0", K::Text),
    ("Column1", K::Text),
    ("Column2", K::Text),
    ("RowValues0", K::TextList),
    ("RowValues1", K::TextList),
    ("RowValues2", K::TextList),
    ("RowStates", K::TextList),
    ("RowStateColors", K::UintList),
    // Sort: the run.
    ("MeterFill", K::Float),
    ("MeterText", K::Text),
    ("RunLabel", K::Text),
    ("Run", K::Action),
    // Recipe.
    ("Reverse", K::Bool),
    ("SetReverse", K::BoolAction),
    ("SortString", K::Text),
    ("SetSortString", K::TextAction),
    ("CopySortString", K::Action),
    ("UnknownCodes", K::Text),
    ("HasUnknownCodes", K::Bool),
    ("Category", K::Int),
    ("SetCategory", K::IntAction),
    ("KeySearch", K::Text),
    ("SetKeySearch", K::TextAction),
    // Presets.
    ("PresetName", K::Text),
    ("SetPresetName", K::TextAction),
    ("SubmitPresetName", K::TextAction),
    ("SavePreset", K::Action),
    // Options.
    ("IdentifyOnLogin", K::Bool),
    ("SetIdentifyOnLogin", K::BoolAction),
    ("Announce", K::Bool),
    ("SetAnnounce", K::BoolAction),
    ("InspectIcon", K::Uint),
    ("InspectName", K::Text),
    ("InspectState", K::Text),
];

/// The window as the client registers it: on the shelf with mudsort's icon
/// ("MS" when the icon cannot be drawn), closed until opened.
#[must_use]
pub fn panel_spec() -> PanelSpec {
    BINDINGS
        .iter()
        .fold(
            PanelSpec::new(WINDOW_ID, TITLE, MARKUP)
                .binding_type("MudSortWindow")
                .icon_text("MS")
                .icon(ICON)
                .start_hidden(),
            |spec, (name, kind)| spec.bind_typed(name, *kind),
        )
        .bind_items(
            "Chips",
            ItemType::new("Chip")
                .bind("Label", K::Text)
                .bind("Tip", K::Text)
                .bind("Flip", K::Action),
        )
        .bind_items(
            "Recipe",
            ItemType::new("RecipeKey")
                .bind("Number", K::Text)
                .bind("Code", K::Text)
                .bind("Name", K::Text)
                .bind("Category", K::Text)
                .bind("Direction", K::Text)
                .bind("CanUp", K::Bool)
                .bind("CanDown", K::Bool)
                .bind("Flip", K::Action)
                .bind("Up", K::Action)
                .bind("Down", K::Action)
                .bind("Remove", K::Action),
        )
        .bind_items(
            "Available",
            ItemType::new("AvailableKey")
                .bind("Code", K::Text)
                .bind("Name", K::Text)
                .bind("Hint", K::Text)
                .bind("Add", K::Action),
        )
        .bind_items(
            "Presets",
            ItemType::new("PresetRow")
                .bind("Name", K::Text)
                .bind("Text", K::Text)
                .bind("Frame", K::Uint)
                .bind("Load", K::Action)
                .bind("Delete", K::Action),
        )
        .bind_items(
            "Inspect",
            ItemType::new("InspectRow")
                .bind("Code", K::Text)
                .bind("Name", K::Text)
                .bind("Value", K::Text)
                .bind("CanAdd", K::Bool)
                .bind("Add", K::Action),
        )
}

/// The window's own state, beside the plugin's.
#[derive(Debug)]
pub struct Window {
    panel: Panel,
    tab: Tab,
    /// The item the Inspector shows.
    inspected: Option<u32>,
    /// The key picker's category, an index of [`CATEGORIES`].
    category: usize,
    search: String,
    preset_name: String,
    palette: Palette,
    /// Something the window shows may have changed.
    dirty: bool,
    /// The preview shows the last run (until the player changes what a run
    /// would do).
    show_run: bool,
    last_phase: Phase,
    /// The ids of the preview's rows, in order.
    shown: Vec<u32>,
    /// The open chest when last refreshed.
    chest: u32,
}

impl Window {
    /// Registers the window.
    ///
    /// # Errors
    /// The client refused the markup or its bindings.
    pub fn open(host: &mut Host<'_>) -> Result<Self, String> {
        let panel = host.register_panel(&panel_spec())?;
        Ok(Self {
            panel,
            tab: Tab::Sort,
            inspected: None,
            category: 0,
            search: String::new(),
            preset_name: String::new(),
            palette: Palette::of_theme(host),
            dirty: true,
            show_run: false,
            last_phase: Phase::Idle,
            shown: Vec::new(),
            chest: 0,
        })
    }

    /// The panel's number (its events' `panel`).
    #[must_use]
    pub fn panel_id(&self) -> u32 {
        self.panel.id()
    }

    /// The tab shown.
    #[must_use]
    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// The item the Inspector shows.
    #[must_use]
    pub fn inspected(&self) -> Option<u32> {
        self.inspected
    }

    /// Whether a tick should refresh: something changed, or a run is under
    /// way or just changed phase.
    #[must_use]
    pub fn needs_refresh(&self, phase: Phase) -> bool {
        self.dirty || phase.is_running() || phase != self.last_phase
    }

    /// An event the plugin heard: notes whether it may change what the
    /// window shows; the world selection feeds the Inspector.
    pub fn on_event(&mut self, host: &mut Host<'_>, event: &Event) {
        match event {
            Event::ObjectChanged(change) => {
                let id = change.object_id;
                // Ours (the preview's, the Inspector's, something we own or
                // the open chest holds); creatures walking about are not.
                let relevant = self.shown.contains(&id)
                    || self.inspected == Some(id)
                    || change.kind == ObjectChangeKind::Released && id == self.chest
                    || change.current.as_ref().is_some_and(|o| {
                        o.is_owned
                            || (self.chest != 0 && o.container_id == self.chest)
                            || o.object_id == self.chest
                    });
                if relevant {
                    self.dirty = true;
                }
            }
            Event::SelectionChanged(change) => {
                if let Some(id) = change.selected
                    && MudSort::inspect(host, id).is_some()
                {
                    self.inspected = Some(id);
                }
                self.dirty = true;
            }
            Event::ThemeChanged(info) => {
                self.palette = info.palette.as_ref().map_or(Palette::CLASSIC, |p| {
                    Palette::from_colors(&p.text, &p.muted, &p.accent, &p.border)
                });
                self.dirty = true;
            }
            Event::LoginComplete
            | Event::Logoff
            | Event::Command(_)
            | Event::ContainerOpened(_)
            | Event::ContainerClosed(_) => self.dirty = true,
            _ => {}
        }
    }

    /// One of the window's controls was used; the window is refreshed.
    pub fn act(&mut self, app: &mut MudSort, host: &mut Host<'_>, action: &UiAction) {
        if let Some(row) = action.row_action() {
            if let Some(step) = row.rows.first() {
                let key = step.key.as_deref();
                self.row_act(app, host, &step.list, step.index, key, &row.handler);
            }
        } else {
            self.window_act(app, host, &action.handler, &action.value);
        }
        self.refresh(app, host);
    }

    fn row_act(
        &mut self,
        app: &mut MudSort,
        host: &mut Host<'_>,
        list: &str,
        index: usize,
        key: Option<&str>,
        handler: &str,
    ) {
        let flag = key.and_then(codes::by_code);
        match (list, handler) {
            ("Chips" | "Recipe", "Flip") => self.edit_keys(app, host, |keys| {
                if let Some(k) = keys.get_mut(index) {
                    k.descending = !k.descending;
                }
            }),
            ("Recipe", "Up") if index > 0 => {
                self.edit_keys(app, host, |keys| {
                    if index < keys.len() {
                        keys.swap(index - 1, index);
                    }
                });
            }
            ("Recipe", "Down") => self.edit_keys(app, host, |keys| {
                if index + 1 < keys.len() {
                    keys.swap(index, index + 1);
                }
            }),
            ("Recipe", "Remove") => self.edit_keys(app, host, |keys| {
                if index < keys.len() {
                    keys.remove(index);
                }
            }),
            ("Available" | "Inspect", "Add") => {
                if let Some(flag) = flag {
                    self.edit_keys(app, host, |keys| {
                        if !keys.iter().any(|k| k.flag == flag) {
                            keys.push(SortKey {
                                flag,
                                descending: false,
                            });
                        }
                    });
                }
            }
            ("Presets", "Load") => {
                if let Some(preset) = app.settings().presets.get(index).cloned() {
                    self.change(app, host, |s| s.sort_string = preset.sort_string);
                    self.tab = Tab::Sort;
                }
            }
            ("Presets", "Delete") => self.change(app, host, |s| {
                if index < s.presets.len() {
                    s.presets.remove(index);
                }
            }),
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn window_act(
        &mut self,
        app: &mut MudSort,
        host: &mut Host<'_>,
        handler: &str,
        value: &ActionValue,
    ) {
        let text = text_of(value);
        match handler {
            "ShowSort" => self.tab = Tab::Sort,
            "ShowRecipe" | "EditRecipe" => self.tab = Tab::Recipe,
            "ShowPresets" => self.tab = Tab::Presets,
            "ShowOptions" => self.tab = Tab::Options,
            "SetFrom" => {
                let found = find_choice(&choices(&app.containers(host)), text).map(|c| c.id);
                if let Some(id) = found {
                    app.set_source(host, id);
                    self.show_run = false;
                }
            }
            "SetTo" => {
                if text == GIVE_TO_PLAYER {
                    self.give_to_selected(app, host);
                } else {
                    let found = find_choice(&choices(&app.containers(host)), text).map(|c| c.id);
                    if let Some(id) = found {
                        app.set_dest(host, id);
                        self.show_run = false;
                    }
                }
            }
            "FromSelection" => {
                let id = host.selection().selected.unwrap_or(0);
                app.set_source(host, id);
                self.show_run = false;
            }
            "ToSelection" => {
                let id = host.selection().selected.unwrap_or(0);
                app.set_dest(host, id);
                self.show_run = false;
            }
            "Swap" => {
                app.swap();
                self.show_run = false;
            }
            "SetClass" => {
                if text == CLASS_FILTERS[0] {
                    self.change(app, host, |s| s.class_filter.clear());
                } else if CLASS_FILTERS.contains(&text) && !text.starts_with('—') {
                    let filter = text.to_owned();
                    self.change(app, host, |s| s.class_filter = filter);
                }
            }
            "SetNameFilter" => {
                let filter = text.to_owned();
                self.change(app, host, |s| s.name_filter = filter);
            }
            "SetStartSlot" => {
                if let Some(slot) = int_of(value) {
                    self.change(app, host, |s| s.start_slot = slot.max(0));
                }
            }
            "SelectRow" => {
                if let Some(id) = int_of(value)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| self.shown.get(i))
                {
                    self.inspected = Some(*id);
                }
            }
            "Run" => {
                if app.sorter().phase().is_running() {
                    app.stop();
                } else {
                    match app.start(host) {
                        Ok(_) => self.show_run = true,
                        Err(error) => chat::say(host, &format!("cannot sort: {error}")),
                    }
                }
            }
            "SetReverse" => {
                if let Some(on) = value.as_bool() {
                    self.change(app, host, |s| s.reverse = on);
                }
            }
            "SetSortString" => {
                let typed = text.to_owned();
                self.change(app, host, |s| s.sort_string = typed);
            }
            "CopySortString" => {
                let text = app.settings().sort_string.clone();
                host.set_clipboard(&text);
            }
            "SetCategory" => {
                if let Some(i) = int_of(value).and_then(|i| usize::try_from(i).ok())
                    && i < CATEGORIES.len()
                {
                    self.category = i;
                }
            }
            "SetKeySearch" => text.clone_into(&mut self.search),
            "SetPresetName" => text.clone_into(&mut self.preset_name),
            "SubmitPresetName" => {
                text.clone_into(&mut self.preset_name);
                self.save_preset(app, host);
            }
            "SavePreset" => self.save_preset(app, host),
            "SetIdentifyOnLogin" => {
                if let Some(on) = value.as_bool() {
                    app.update_settings(host, |s| s.identify_on_login = on);
                }
            }
            "SetAnnounce" => {
                if let Some(on) = value.as_bool() {
                    app.update_settings(host, |s| s.announce_when_done = on);
                }
            }
            _ => {}
        }
    }

    /// The To menu's "Give to selected player": the selection, when it is
    /// another player.
    fn give_to_selected(&mut self, app: &mut MudSort, host: &mut Host<'_>) {
        let me = app.me().map_or(0, |me| me.id);
        let selected = host.selection().selected.unwrap_or(0);
        if matches!(run::classify(host, selected, me), Some(Place::Player(_))) {
            app.set_dest(host, selected);
            self.show_run = false;
        } else {
            chat::say(host, "Select the player to give the items to first.");
        }
    }

    /// Changes a setting that changes what a run would do: the preview
    /// shows the new order from now on.
    fn change(
        &mut self,
        app: &mut MudSort,
        host: &mut Host<'_>,
        change: impl FnOnce(&mut crate::settings::Settings),
    ) {
        app.update_settings(host, change);
        self.show_run = false;
    }

    /// Edits the recipe's keys and writes them back as the sort string.
    fn edit_keys(
        &mut self,
        app: &mut MudSort,
        host: &mut Host<'_>,
        edit: impl FnOnce(&mut Vec<SortKey>),
    ) {
        let mut keys = app.settings().keys().keys;
        edit(&mut keys);
        let text = sort_string::serialize(&keys);
        self.change(app, host, |s| s.sort_string = text);
    }

    /// Saves the current recipe under the name typed (replacing a preset of
    /// that name); a blank name saves nothing.
    fn save_preset(&mut self, app: &mut MudSort, host: &mut Host<'_>) {
        let name = self.preset_name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let current = app.settings().sort_string.clone();
        app.update_settings(host, |s| {
            match s.presets.iter_mut().find(|p| p.name == name) {
                Some(preset) => preset.sort_string = current,
                None => s.presets.push(Preset::new(&name, &current)),
            }
        });
        self.preset_name.clear();
    }

    /// Pushes every value the window shows; the panel sends only what
    /// changed.
    pub fn refresh(&mut self, app: &MudSort, host: &mut Host<'_>) {
        let values = self.values(app, host);
        for (name, value) in values {
            self.panel.set(name, value);
        }
        if let Err(error) = self.panel.flush(host) {
            host.warn(&format!("The window could not be updated: {error}"));
        }
        self.dirty = false;
        self.last_phase = app.sorter().phase();
    }

    /// The name the meter gives a container or player.
    fn place_name(host: &mut Host<'_>, choices: &[Choice], id: u32) -> String {
        if id == 0 {
            return "nowhere".to_owned();
        }
        if let Some(choice) = choices.iter().find(|c| c.id == id) {
            return choice.stem.clone();
        }
        host.object(id)
            .map_or_else(|| format!("{id:#010X}"), |object| object.name)
    }

    /// What a menu shows as chosen.
    fn menu_text(host: &mut Host<'_>, choices: &[Choice], id: u32, giving: bool) -> String {
        if id == 0 {
            return "(none)".to_owned();
        }
        if let Some(choice) = choices.iter().find(|c| c.id == id) {
            return choice.label.clone();
        }
        match host.object(id) {
            Some(object) if giving && object.object_class == ObjectClass::Player => {
                format!("Give to {}", object.name)
            }
            Some(object) => object.name,
            None => format!("{id:#010X}"),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn values(&mut self, app: &MudSort, host: &mut Host<'_>) -> Vec<(&'static str, TypedValue)> {
        let settings = app.settings();
        let phase = app.sorter().phase();
        let running = phase.is_running();
        let show_run = running || (phase == Phase::Done && self.show_run);
        let containers = app.containers(host);
        self.chest = containers
            .iter()
            .find_map(|c| match c.place {
                Place::Chest(id) => Some(id),
                _ => None,
            })
            .unwrap_or(0);
        let choices = choices(&containers);
        let parsed = settings.keys();

        // The rows: the run's while it has them, else what a run would do.
        let (rows, problem): (Vec<PreviewRow>, Option<String>) = if show_run {
            (app.rows(), None)
        } else {
            match app.preview(host) {
                Ok(rows) => (rows, None),
                Err(error) => (Vec::new(), Some(error)),
            }
        };
        let columns: Vec<&str> = if show_run {
            app.columns()
        } else {
            parsed
                .keys
                .iter()
                .take(run::SHOWN_KEYS)
                .map(|k| k.flag.display_name())
                .collect()
        };
        self.shown = rows.iter().map(|r| r.id).collect();
        let need = rows.iter().filter(|r| r.state == RowState::NeedsId).count();
        let column = |i: usize| -> Vec<String> {
            rows.iter()
                .map(|r| r.values.get(i).cloned().unwrap_or_default())
                .collect()
        };

        // The meter.
        let me = app.me().map(|me| me.id);
        let dest = if show_run {
            app.sorter().config().map_or(app.dest(), |c| c.dest)
        } else {
            app.dest()
        };
        let dest_name = Self::place_name(host, &choices, dest);
        let progress = app.progress();
        let (meter_fill, meter_text) = match phase {
            Phase::Identifying => (
                ratio(progress.done, progress.total),
                format!("Identifying… {} of {} ready", progress.done, progress.total),
            ),
            Phase::BuildingList => (0.0, "Building the list…".to_owned()),
            Phase::Moving => (
                ratio(progress.done, progress.total),
                format!(
                    "Moving {} of {} to {dest_name}",
                    (progress.done + 1).min(progress.total),
                    progress.total
                ),
            ),
            Phase::Done if show_run => {
                let moved = rows.iter().filter(|r| r.state == RowState::Moved).count();
                let failed = rows.iter().filter(|r| r.state == RowState::Failed).count();
                let mut text = format!("Done — {moved} {} sorted into {dest_name}", plural(moved));
                if failed > 0 {
                    text.push_str(&format!(", {failed} failed"));
                }
                (1.0, text)
            }
            _ => {
                let text = match (&problem, me) {
                    (_, None) => "Log in to sort".to_owned(),
                    (Some(error), _) => format!("Cannot sort: {error}"),
                    (None, Some(me)) if run::classify(host, dest, me).is_none() => {
                        "Cannot sort: choose where the items go".to_owned()
                    }
                    (None, Some(_)) => {
                        let mut text = format!(
                            "Ready — {} {} to {dest_name}",
                            rows.len(),
                            plural(rows.len())
                        );
                        if need > 0 {
                            text.push_str(&format!(", {need} to identify first"));
                        }
                        text
                    }
                };
                (0.0, text)
            }
        };
        let summary = match (&problem, rows.len()) {
            (Some(_), _) => String::new(),
            (None, 0) => "No items".to_owned(),
            (None, n) if need > 0 => format!("{n} {} · {need} need identify", plural(n)),
            (None, n) => format!("{n} {} · all identified", plural(n)),
        };
        let status = match phase {
            Phase::Identifying => "identifying…",
            Phase::BuildingList | Phase::Moving => "sorting…",
            Phase::Idle | Phase::Done => "",
        };
        let run_label = if running {
            "Stop"
        } else if show_run {
            "Sort again"
        } else {
            "Start sorting"
        };

        // The class filter.
        let mut class_items: Vec<String> = CLASS_FILTERS.iter().map(|s| (*s).to_owned()).collect();
        let class_selected = match settings.class_filter() {
            None => CLASS_FILTERS[0].to_owned(),
            Some(filter) => {
                match CLASS_FILTERS
                    .iter()
                    .find(|c| !c.starts_with('—') && c.eq_ignore_ascii_case(filter))
                {
                    Some(choice) => (*choice).to_owned(),
                    None => {
                        let custom = format!("{CUSTOM_PREFIX}{filter}");
                        class_items.push(custom.clone());
                        custom
                    }
                }
            }
        };

        // The recipe.
        let keys = &parsed.keys;
        let chips: Items = keys
            .iter()
            .map(|k| {
                let (now, next) = if k.descending {
                    ("high first", "low")
                } else {
                    ("low first", "high")
                };
                Item::new()
                    .set("Label", format!("{}, {now}", k.flag.display_name()))
                    .set("Tip", format!("Click to sort {next} first"))
            })
            .collect();
        let recipe: Items = keys
            .iter()
            .enumerate()
            .map(|(i, k)| {
                Item::new()
                    .set("Number", (i + 1).to_string())
                    .set("Code", k.flag.code())
                    .set("Name", k.flag.display_name())
                    .set("Category", category_name(k.flag.category()))
                    .set("Direction", direction(k))
                    .set("CanUp", i > 0)
                    .set("CanDown", i + 1 < keys.len())
            })
            .collect();
        let unknown = &parsed.unknown;
        let unknown_text = match unknown.len() {
            0 => String::new(),
            1 => format!("Unknown code skipped: {}", unknown[0]),
            _ => format!("Unknown codes skipped: {}", unknown.join(", ")),
        };
        let query = self.search.trim().to_lowercase();
        let available: Items = codes::table()
            .flags
            .iter()
            .filter(|flag| !keys.iter().any(|k| k.flag == *flag))
            .filter(|flag| in_category(flag, self.category))
            .filter(|flag| {
                query.is_empty()
                    || format!("{} {} {}", flag.display_name(), flag.code(), flag.name)
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|flag| {
                Item::new()
                    .set("Code", flag.code())
                    .set("Name", flag.display_name())
                    .set(
                        "Hint",
                        format!("{} · {}", category_name(flag.category()), flag.name),
                    )
            })
            .collect();

        // The presets.
        let opaque = |rgb: u32| 0xFF00_0000 | rgb;
        let presets: Items = settings
            .presets
            .iter()
            .map(|p| {
                let current = p.sort_string.eq_ignore_ascii_case(&settings.sort_string);
                Item::new()
                    .set("Name", p.name.as_str())
                    .set("Text", p.sort_string.as_str())
                    .set(
                        "Frame",
                        opaque(if current {
                            self.palette.accent
                        } else {
                            self.palette.border
                        }),
                    )
            })
            .collect();

        // The Inspector.
        let entries = self.inspected.and_then(|id| MudSort::inspect(host, id));
        if entries.is_none() {
            self.inspected = None;
        }
        let entries = entries.unwrap_or_default();
        let entry = |code: &str| {
            entries
                .iter()
                .find(|e| e.code == code)
                .map(|e| e.value.clone())
        };
        let (inspect_name, inspect_state) = match self.inspected {
            Some(id) => {
                let class = entry("OC").unwrap_or_default();
                let known = if identify::has_appraisal(host, id) {
                    "identified"
                } else {
                    "not identified yet"
                };
                (
                    entry("NM").unwrap_or_default(),
                    format!("{class} · {known}"),
                )
            }
            None => ("Nothing selected".to_owned(), String::new()),
        };
        let inspect: Items = entries
            .iter()
            .map(|e| {
                let in_recipe = keys.iter().any(|k| k.flag.code() == e.code);
                Item::new()
                    .set("Code", e.code)
                    .set("Name", e.display_name)
                    .set("Value", e.value.as_str())
                    .set("CanAdd", !in_recipe)
            })
            .collect();
        let preview_selected = self
            .inspected
            .and_then(|id| self.shown.iter().position(|s| *s == id))
            .map_or(-1, index);

        let header = |i: usize| columns.get(i).copied().unwrap_or("").to_owned();
        let mut to_items: Vec<String> = choices.iter().map(|c| c.label.clone()).collect();
        to_items.push(GIVE_TO_PLAYER.to_owned());
        vec![
            ("SortTab", (self.tab == Tab::Sort).into()),
            ("RecipeTab", (self.tab == Tab::Recipe).into()),
            ("PresetsTab", (self.tab == Tab::Presets).into()),
            ("OptionsTab", (self.tab == Tab::Options).into()),
            ("Status", status.into()),
            (
                "FromItems",
                choices
                    .iter()
                    .map(|c| c.label.clone())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (
                "FromSelected",
                Self::menu_text(host, &choices, app.source(), false).into(),
            ),
            ("ToItems", to_items.into()),
            (
                "ToSelected",
                Self::menu_text(host, &choices, app.dest(), true).into(),
            ),
            ("ClassItems", class_items.into()),
            ("ClassSelected", class_selected.into()),
            ("NameFilter", settings.name_filter.as_str().into()),
            ("StartSlot", settings.start_slot.into()),
            ("Chips", chips.into()),
            ("PreviewSummary", summary.into()),
            ("PreviewSelected", preview_selected.into()),
            (
                "RowNumbers",
                (1..=rows.len())
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            ("RowIcons", self.shown.clone().into()),
            (
                "RowNames",
                rows.iter()
                    .map(|r| r.name.clone())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            ("Column0", header(0).into()),
            ("Column1", header(1).into()),
            ("Column2", header(2).into()),
            ("RowValues0", column(0).into()),
            ("RowValues1", column(1).into()),
            ("RowValues2", column(2).into()),
            (
                "RowStates",
                rows.iter()
                    .map(|r| r.state.label().to_owned())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (
                "RowStateColors",
                rows.iter()
                    .map(|r| state_color(r.state, &self.palette))
                    .collect::<Vec<_>>()
                    .into(),
            ),
            ("MeterFill", meter_fill.into()),
            ("MeterText", meter_text.into()),
            ("RunLabel", run_label.into()),
            ("Recipe", recipe.into()),
            ("Reverse", settings.reverse.into()),
            ("SortString", settings.sort_string.as_str().into()),
            ("UnknownCodes", unknown_text.into()),
            ("HasUnknownCodes", (!unknown.is_empty()).into()),
            ("Category", index(self.category).into()),
            ("KeySearch", self.search.as_str().into()),
            ("Available", available.into()),
            ("Presets", presets.into()),
            ("PresetName", self.preset_name.as_str().into()),
            ("IdentifyOnLogin", settings.identify_on_login.into()),
            ("Announce", settings.announce_when_done.into()),
            ("InspectIcon", self.inspected.unwrap_or(0).into()),
            ("InspectName", inspect_name.into()),
            ("InspectState", inspect_state.into()),
            ("Inspect", inspect.into()),
        ]
    }
}
