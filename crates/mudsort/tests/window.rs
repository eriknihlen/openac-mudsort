//! The window (`markup/mudsort.xml`): registered and bound through the
//! client's own UI state, and each control driving the plugin as the
//! design says.

mod sim;

use ac_plugin_ui::ActionValue as Av;
use mudsort::run::Phase;
use mudsort::settings::Preset;
use mudsort::sort_string::DEFAULT;
use mudsort::window::{self, BINDINGS, GIVE_TO_PLAYER, Tab};
use openac_plugin::calls;
use openac_plugin::items::InventoryItem;
use openac_plugin::ui::{TypedBindingKind, TypedValue};
use openac_plugin::world::{ObjectChange, ObjectChangeKind, ObjectClass, SelectionChange};
use openac_plugin::{Event, LogLevel};
use sim::{CHEST, HORAN, Harness, ME, PACK1, PACK2, Sim};

const C: u32 = 0x8000_2001;
const A: u32 = 0x8000_2002;
const B: u32 = 0x8000_2003;
const PLATE: u32 = 0x8000_3001;

/// Three gems in the main pack, armor levels 10, 30 and 20; two without an
/// appraisal. Sorted by armor level, largest first: A, B, C.
fn three_gems(sim: &mut Sim) {
    sim.add_item(C, "Gem C", ObjectClass::Gem, ME, 10, false);
    sim.add_item(A, "Gem A", ObjectClass::Gem, ME, 30, true);
    sim.add_item(B, "Gem B", ObjectClass::Gem, ME, 20, false);
}

/// Logged in with three gems and a breastplate, the recipe `AL-`, and the
/// window refreshed.
fn window() -> Harness {
    let mut h = Harness::logged_in(|sim| {
        three_gems(sim);
        sim.add_item(PLATE, "Breastplate", ObjectClass::Armor, ME, 50, true);
    });
    h.type_text("SetSortString", "AL-");
    h
}

fn sort_string(h: &Harness) -> String {
    h.plugin.settings().sort_string.clone()
}

#[test]
fn the_window_registers_on_the_shelf_with_every_binding_pushed() {
    let mut h = window();
    h.tick();
    let spec = window::panel_spec();
    let registration = spec.registration();
    assert_eq!(registration.window_id, "main");
    assert_eq!(registration.title, "MudSort");
    assert!(registration.show_in_side_panel);
    assert_eq!(registration.icon_surface_id, window::ICON);
    assert!(!registration.start_visible);
    // The client built it without a warning for the author.
    assert!(
        h.sim
            .fake
            .logs
            .iter()
            .all(|(level, _)| *level != LogLevel::Warn),
        "{:?}",
        h.sim.fake.logs
    );
    for (name, kind) in BINDINGS {
        let is_handler = matches!(
            kind,
            TypedBindingKind::Action
                | TypedBindingKind::IntAction
                | TypedBindingKind::TextAction
                | TypedBindingKind::BoolAction
        );
        if !is_handler {
            assert!(
                h.sim.fake.panel_typed_value(h.panel(), name).is_some(),
                "{name} has no value"
            );
        }
    }
    for list in ["Chips", "Recipe", "Available", "Presets", "Inspect"] {
        assert!(
            h.sim.fake.panel_item_count(h.panel(), list).is_some(),
            "{list} has no items"
        );
    }
    assert!(h.flag("SortTab"));
    assert_eq!(h.text("RunLabel"), "Start sorting");
}

#[test]
fn the_tabs_switch_the_pages() {
    let mut h = window();
    for (handler, tab, binding) in [
        ("ShowRecipe", Tab::Recipe, "RecipeTab"),
        ("ShowPresets", Tab::Presets, "PresetsTab"),
        ("ShowOptions", Tab::Options, "OptionsTab"),
        ("ShowSort", Tab::Sort, "SortTab"),
    ] {
        h.click(handler);
        assert_eq!(h.plugin.window().unwrap().tab(), tab);
        for page in ["SortTab", "RecipeTab", "PresetsTab", "OptionsTab"] {
            assert_eq!(h.flag(page), page == binding, "{handler}: {page}");
        }
    }
    h.click("EditRecipe");
    assert!(h.flag("RecipeTab"));
}

#[test]
fn from_and_to_list_the_containers_and_set_the_source_and_destination() {
    let mut h = window();
    let packs = [
        "Main pack · 4 of 102",
        "Pack 1 · Pack 1 · 0 of 24",
        "Pack 2 · Pack 2 · 0 of 24",
    ];
    assert_eq!(h.texts("FromItems"), packs);
    let mut to = packs.map(str::to_owned).to_vec();
    to.push(GIVE_TO_PLAYER.to_owned());
    assert_eq!(h.texts("ToItems"), to);
    assert_eq!(h.text("FromSelected"), "Main pack · 4 of 102");

    h.type_text("SetTo", "Pack 2 · Pack 2 · 0 of 24");
    assert_eq!(h.plugin.dest(), PACK2);
    assert_eq!(h.text("ToSelected"), "Pack 2 · Pack 2 · 0 of 24");
    // A line whose count went stale while the menu was open still names
    // its pack.
    h.type_text("SetTo", "Pack 1 · Pack 1 · 7 of 24");
    assert_eq!(h.plugin.dest(), PACK1);

    // The open chest joins the list.
    h.sim.open_chest();
    h.send(Event::ContainerOpened(CHEST));
    h.tick();
    assert_eq!(h.texts("FromItems").last().unwrap(), "Open: Chest");
    h.type_text("SetFrom", "Open: Chest");
    assert_eq!(h.plugin.source(), CHEST);
    assert_eq!(h.text("FromSelected"), "Open: Chest");
}

#[test]
fn give_to_selected_player_needs_a_player_selected() {
    let mut h = window();
    h.type_text("SetTo", GIVE_TO_PLAYER);
    assert_eq!(h.plugin.dest(), ME);
    assert_eq!(
        h.chat().last().unwrap(),
        "[mudsort]: Select the player to give the items to first."
    );
    h.sim.selected = Some(HORAN);
    h.type_text("SetTo", GIVE_TO_PLAYER);
    assert_eq!(h.plugin.dest(), HORAN);
    assert_eq!(h.text("ToSelected"), "Give to Horan");
    assert!(
        h.text("MeterText")
            .ends_with("to Horan, 2 to identify first")
    );
}

#[test]
fn use_selection_and_swap_set_the_source_and_destination() {
    let mut h = window();
    h.sim.selected = Some(PACK1);
    h.click("ToSelection");
    assert_eq!(h.plugin.dest(), PACK1);
    h.sim.selected = Some(PACK2);
    h.click("FromSelection");
    assert_eq!(h.plugin.source(), PACK2);
    h.click("Swap");
    assert_eq!((h.plugin.source(), h.plugin.dest()), (PACK1, PACK2));
    assert_eq!(h.text("FromSelected"), "Pack 1 · Pack 1 · 0 of 24");
    assert_eq!(h.text("ToSelected"), "Pack 2 · Pack 2 · 0 of 24");
}

#[test]
fn the_class_and_name_filters_change_the_preview() {
    let mut h = window();
    assert_eq!(
        h.texts("RowNames"),
        ["Breastplate", "Gem A", "Gem B", "Gem C"]
    );
    assert_eq!(h.text("ClassSelected"), "None");
    assert_eq!(h.texts("ClassItems").len(), window::CLASS_FILTERS.len());

    h.type_text("SetClass", "Gem");
    assert_eq!(h.plugin.settings().class_filter, "Gem");
    assert_eq!(h.text("ClassSelected"), "Gem");
    assert_eq!(h.texts("RowNames"), ["Gem A", "Gem B", "Gem C"]);
    // A heading picks nothing.
    h.type_text("SetClass", "— Equipment —");
    assert_eq!(h.plugin.settings().class_filter, "Gem");

    h.type_text("SetNameFilter", "gem b");
    assert_eq!(h.texts("RowNames"), ["Gem B"]);
    assert_eq!(h.text("NameFilter"), "gem b");
    assert_eq!(h.text("PreviewSummary"), "1 item · 1 need identify");
    h.type_text("SetNameFilter", "");
    h.type_text("SetClass", "None");
    assert_eq!(h.plugin.settings().class_filter, "");
    assert_eq!(h.texts("RowNames").len(), 4);

    // A filter /ms set ocfilter gave that is none of the choices.
    h.ms("set ocfilter ge");
    h.tick();
    assert_eq!(h.text("ClassSelected"), "Custom: ge");
    assert_eq!(h.texts("ClassItems").last().unwrap(), "Custom: ge");
    assert_eq!(h.texts("RowNames").len(), 3);

    h.act("SetStartSlot", Av::Int(3));
    assert_eq!(h.plugin.settings().start_slot, 3);
    assert_eq!(h.value("StartSlot"), TypedValue::Int(3));
    assert_eq!(h.saved().start_slot, 3);
}

#[test]
fn the_preview_shows_the_move_order_with_icons_values_and_states() {
    let h = window();
    assert_eq!(h.texts("RowNumbers"), ["1", "2", "3", "4"]);
    // The icon column carries the item ids (the client composes each one's
    // inventory icon).
    assert_eq!(
        h.value("RowIcons"),
        TypedValue::UintList(vec![PLATE, A, B, C])
    );
    assert_eq!(h.text("Column0"), "Armor level");
    assert_eq!(h.text("Column1"), "");
    assert_eq!(h.texts("RowValues0"), ["50", "30", "20", "10"]);
    assert_eq!(
        h.texts("RowStates"),
        ["Ready", "Ready", "Needs ID", "Needs ID"]
    );
    let palette = window::Palette::CLASSIC;
    assert_eq!(
        h.value("RowStateColors"),
        TypedValue::UintList(vec![
            palette.text,
            palette.text,
            window::NEEDS_ID_COLOR,
            window::NEEDS_ID_COLOR
        ])
    );
    assert_eq!(h.text("PreviewSummary"), "4 items · 2 need identify");
    assert_eq!(
        h.text("MeterText"),
        "Ready — 4 items to Main pack, 2 to identify first"
    );
}

#[test]
fn the_preview_follows_the_inventory_and_a_quiet_tick_sends_nothing() {
    let mut h = window();
    h.tick();
    let pushes = |h: &Harness| {
        h.sim
            .fake
            .calls
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    openac_plugin::Import::UiPanelSet | openac_plugin::Import::UiPanelSetTyped
                )
            })
            .count()
    };
    let before = pushes(&h);
    for _ in 0..5 {
        h.tick();
    }
    assert_eq!(pushes(&h), before, "quiet ticks pushed values");

    // A creature walking about changes nothing.
    h.send(Event::ObjectChanged(ObjectChange {
        object_id: 0x8000_9999,
        kind: ObjectChangeKind::Moved,
        revision: 1,
        changed_fields: 0,
        current: None,
    }));
    h.tick();
    assert_eq!(pushes(&h), before);

    // A new item in the main pack shows at once.
    h.sim
        .add_item(0x8000_4001, "Gem D", ObjectClass::Gem, ME, 40, true);
    let created = h.sim.objects.get(&0x8000_4001).cloned();
    h.send(Event::ObjectChanged(ObjectChange {
        object_id: 0x8000_4001,
        kind: ObjectChangeKind::Created,
        revision: 2,
        changed_fields: 0,
        current: created,
    }));
    h.tick();
    assert_eq!(
        h.texts("RowNames"),
        ["Breastplate", "Gem D", "Gem A", "Gem B", "Gem C"]
    );
    assert_eq!(pushes(&h), before + 1);
}

#[test]
fn start_and_stop_from_the_button_drive_the_run_and_the_meter() {
    let mut h = window();
    h.type_text("SetClass", "Gem");
    h.type_text("SetTo", "Pack 1 · Pack 1 · 0 of 24");
    assert_eq!(
        h.text("MeterText"),
        "Ready — 3 items to Pack 1 · Pack 1, 2 to identify first"
    );
    h.click("Run");
    assert_eq!(h.plugin.sorter().phase(), Phase::Identifying);
    assert_eq!(h.text("RunLabel"), "Stop");
    assert_eq!(h.text("Status"), "identifying…");
    assert_eq!(h.text("MeterText"), "Identifying… 1 of 3 ready");

    h.click("Run");
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert_eq!(h.text("RunLabel"), "Start sorting");
    assert_eq!(h.text("Status"), "");

    h.click("Run");
    // Identify both, build the list, send the first move.
    for _ in 0..20 {
        h.tick();
        if let Some(event) = h.sim.answer_appraisal() {
            h.send(event);
        }
        if h.sim.in_flight.is_some() {
            break;
        }
    }
    h.tick();
    assert_eq!(h.plugin.sorter().phase(), Phase::Moving);
    assert_eq!(h.text("Status"), "sorting…");
    assert_eq!(h.text("MeterText"), "Moving 1 of 3 to Pack 1 · Pack 1");
    assert_eq!(h.texts("RowStates"), ["Moving", "Ready", "Ready"]);

    h.run(40);
    assert_eq!(h.plugin.sorter().phase(), Phase::Done);
    // The last move's answer ended the run; the next tick shows it.
    h.tick();
    assert_eq!(h.text("RunLabel"), "Sort again");
    assert_eq!(
        h.text("MeterText"),
        "Done — 3 items sorted into Pack 1 · Pack 1"
    );
    assert_eq!(h.value("MeterFill"), TypedValue::Float(1.0));
    assert_eq!(h.texts("RowStates"), ["Moved", "Moved", "Moved"]);
    assert_eq!(h.sim.contents(PACK1), ["Gem C", "Gem B", "Gem A"]);

    // Changing the recipe goes back to the preview.
    h.type_text("SetSortString", "NM");
    assert_eq!(h.text("RunLabel"), "Start sorting");
}

#[test]
fn a_run_that_cannot_start_says_why() {
    let mut h = window();
    h.ms("set source 12345");
    h.tick();
    assert!(h.text("MeterText").starts_with("Cannot sort: the source"));
    assert_eq!(h.text("FromSelected"), "(none)");
    h.click("Run");
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
    assert!(
        h.chat()
            .last()
            .unwrap()
            .starts_with("[mudsort]: cannot sort: the source")
    );
}

#[test]
fn recipe_edits_update_the_sort_string_and_are_saved() {
    let mut h = window();
    h.type_text("SetSortString", "OC,AL-");
    assert_eq!(h.item_field("Recipe", "Code"), ["OC", "AL"]);
    assert_eq!(
        h.item_field("Recipe", "Direction"),
        ["Low to high", "High to low"]
    );
    assert_eq!(h.item_field("Recipe", "Category"), ["Computed", "Number"]);
    assert_eq!(
        h.item_field("Chips", "Label"),
        ["Object class, low first", "Armor level, high first"]
    );

    h.click("Recipe[1]/Flip");
    assert_eq!(sort_string(&h), "OC,AL");
    h.click("Recipe[1]/Up");
    assert_eq!(sort_string(&h), "AL,OC");
    h.click("Recipe[0]/Down");
    assert_eq!(sort_string(&h), "OC,AL");
    h.click("Available[0:NM]/Add");
    assert_eq!(sort_string(&h), "OC,AL,NM");
    h.click("Recipe[0]/Remove");
    assert_eq!(sort_string(&h), "AL,NM");
    h.click("Chips[1]/Flip");
    assert_eq!(sort_string(&h), "AL,NM-");
    assert_eq!(h.text("SortString"), "AL,NM-");
    assert_eq!(h.text("Column1"), "Name");

    h.act("SetReverse", Av::Bool(true));
    assert!(h.plugin.settings().reverse);
    assert!(h.flag("Reverse"));

    let saved = h.saved();
    assert_eq!(saved.sort_string, "AL,NM-");
    assert!(saved.reverse);

    h.click("CopySortString");
    assert_eq!(
        h.sim.fake.requests::<calls::ClientSetClipboard>(),
        ["AL,NM-".to_owned()]
    );
}

#[test]
fn a_sort_string_with_an_unknown_code_shows_the_warning() {
    let mut h = window();
    assert!(!h.flag("HasUnknownCodes"));
    h.type_text("SetSortString", "OC,ZZ,AL-");
    assert_eq!(sort_string(&h), "OC,ZZ,AL-");
    assert!(h.flag("HasUnknownCodes"));
    assert_eq!(h.text("UnknownCodes"), "Unknown code skipped: ZZ");
    assert_eq!(h.item_field("Recipe", "Code"), ["OC", "AL"]);
    h.type_text("SetSortString", "QQ,OC,ZZ");
    assert_eq!(h.text("UnknownCodes"), "Unknown codes skipped: QQ, ZZ");
    h.type_text("SetSortString", "OC");
    assert!(!h.flag("HasUnknownCodes"));
}

#[test]
fn the_key_picker_lists_keys_not_in_the_recipe_by_category_and_search() {
    let mut h = window();
    let common = h.item_field("Available", "Code");
    assert!(common.contains(&"OC".to_owned()));
    assert!(!common.contains(&"AL".to_owned()), "AL is in the recipe");
    assert!(
        common
            .iter()
            .all(|code| { mudsort::codes::by_code(code).is_some_and(|flag| flag.common) })
    );
    assert_eq!(
        h.item_field("Available", "Hint")[0],
        "Computed · ObjectClass"
    );

    h.act("SetCategory", Av::Int(2));
    assert_eq!(h.value("Category"), TypedValue::Int(2));
    assert!(
        h.item_field("Available", "Hint")
            .iter()
            .all(|hint| hint.starts_with("Text · "))
    );
    h.act("SetCategory", Av::Int(1));
    h.type_text("SetKeySearch", "armor");
    let found = h.item_field("Available", "Name");
    assert!(found.contains(&"Armor set".to_owned()));
    assert!(found.contains(&"Buffed armor level".to_owned()));
    assert!(found.iter().all(|n| n.to_lowercase().contains("armor")));
}

#[test]
fn presets_save_load_and_delete() {
    let mut h = window();
    let names = h.item_field("Presets", "Name");
    assert_eq!(names, ["Everything", "Saved 1", "Saved 2", "Saved 3"]);

    // A blank name saves nothing.
    h.click("SavePreset");
    assert_eq!(h.plugin.settings().presets.len(), 4);

    h.type_text("SetPresetName", "Gems by armor");
    h.click("SavePreset");
    assert_eq!(
        h.plugin.settings().presets.last(),
        Some(&Preset::new("Gems by armor", "AL-"))
    );
    assert_eq!(h.text("PresetName"), "");
    // The current recipe's preset is framed in the accent colour.
    let frames = h.item_field("Presets", "Frame");
    let accent = format!("Uint({})", 0xFF00_0000 | window::Palette::CLASSIC.accent);
    assert_eq!(frames[4], accent);
    assert_ne!(frames[0], accent);

    h.click("ShowPresets");
    h.click("Presets[0]/Load");
    assert_eq!(sort_string(&h), DEFAULT);
    assert!(h.flag("SortTab"));

    // Enter in the name field saves too, replacing a preset of that name.
    h.act("SubmitPresetName", Av::Text("Gems by armor".to_owned()));
    assert_eq!(
        h.plugin.settings().presets.last(),
        Some(&Preset::new("Gems by armor", DEFAULT))
    );
    assert_eq!(h.plugin.settings().presets.len(), 5);

    h.click("Presets[1]/Delete");
    assert_eq!(
        h.item_field("Presets", "Name"),
        ["Everything", "Saved 2", "Saved 3", "Gems by armor"]
    );
    assert_eq!(h.saved().presets.len(), 4);
}

#[test]
fn the_options_switches_are_saved() {
    let mut h = window();
    assert!(h.flag("IdentifyOnLogin"));
    assert!(h.flag("Announce"));
    h.act("SetIdentifyOnLogin", Av::Bool(false));
    h.act("SetAnnounce", Av::Bool(false));
    assert!(!h.plugin.settings().identify_on_login);
    assert!(!h.plugin.settings().announce_when_done);
    assert!(!h.flag("IdentifyOnLogin"));
    assert!(!h.flag("Announce"));
    let saved = h.saved();
    assert!(!saved.identify_on_login);
    assert!(!saved.announce_when_done);
}

#[test]
fn a_preview_row_or_the_world_selection_fills_the_inspector() {
    let mut h = window();
    assert_eq!(h.text("InspectName"), "Nothing selected");
    assert_eq!(h.value("InspectIcon"), TypedValue::Uint(0));

    h.act("SelectRow", Av::Int(1));
    assert_eq!(h.plugin.window().unwrap().inspected(), Some(A));
    assert_eq!(h.text("InspectName"), "Gem A");
    assert_eq!(h.text("InspectState"), "Gem · identified");
    assert_eq!(h.value("InspectIcon"), TypedValue::Uint(A));
    assert_eq!(h.value("PreviewSelected"), TypedValue::Int(1));
    let codes = h.item_field("Inspect", "Code");
    let values = h.item_field("Inspect", "Value");
    let al = codes.iter().position(|c| c == "AL").unwrap();
    assert_eq!(values[al], "30");

    // An item selected in the world.
    h.sim.selected = Some(B);
    h.send(Event::SelectionChanged(SelectionChange {
        previous: Some(A),
        selected: Some(B),
    }));
    h.tick();
    assert_eq!(h.text("InspectName"), "Gem B");
    assert_eq!(h.text("InspectState"), "Gem · not identified yet");
    // Selecting a player leaves the Inspector on the item.
    h.send(Event::SelectionChanged(SelectionChange {
        previous: Some(B),
        selected: Some(HORAN),
    }));
    h.tick();
    assert_eq!(h.text("InspectName"), "Gem B");
}

#[test]
fn plus_in_the_inspector_adds_the_key() {
    let mut h = window();
    h.type_text("SetSortString", "OC");
    h.act("SelectRow", Av::Int(0));
    let codes = h.item_field("Inspect", "Code");
    let can_add = h.item_field("Inspect", "CanAdd");
    let at = |code: &str| codes.iter().position(|c| c == code).unwrap();
    assert_eq!(can_add[at("OC")], "Bool(false)");
    assert_eq!(can_add[at("AL")], "Bool(true)");
    h.click("Inspect[3:AL]/Add");
    assert_eq!(sort_string(&h), "OC,AL");
    let can_add = h.item_field("Inspect", "CanAdd");
    assert_eq!(can_add[at("AL")], "Bool(false)");
    // Adding a key already there changes nothing.
    h.click("Inspect[3:AL]/Add");
    assert_eq!(sort_string(&h), "OC,AL");
}

#[test]
fn an_item_given_away_leaves_the_inspector_and_the_preview() {
    let mut h = window();
    h.act("SelectRow", Av::Int(0));
    assert_eq!(h.text("InspectName"), "Breastplate");
    h.sim
        .owned
        .retain(|item: &InventoryItem| item.object_id != PLATE);
    h.send(Event::ObjectChanged(ObjectChange {
        object_id: PLATE,
        kind: ObjectChangeKind::Released,
        revision: 3,
        changed_fields: 0,
        current: None,
    }));
    h.tick();
    assert_eq!(h.text("InspectName"), "Nothing selected");
    assert_eq!(h.texts("RowNames"), ["Gem A", "Gem B", "Gem C"]);
}
