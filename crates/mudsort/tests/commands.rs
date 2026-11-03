//! The `/ms` commands (`PluginCore.cs`, `ProcessMSCommand`).

mod sim;

use mudsort::commands::{Command, HELP, HELP_HEADER, Target, parse, parse_id};
use mudsort::run::Phase;
use openac_plugin::world::ObjectClass;
use sim::{CHEST, HORAN, Harness, ME, PACK1, PACK2};

fn last(h: &Harness) -> String {
    h.chat().last().cloned().unwrap_or_default()
}

#[test]
fn lines_read_as_mudsort_read_them() {
    assert_eq!(parse(""), Command::Help);
    assert_eq!(parse("help"), Command::Help);
    assert_eq!(parse("HELP me"), Command::Help);
    assert_eq!(parse("start"), Command::Start);
    // Matched by the start of the line, as mudsort did.
    assert_eq!(parse("startle"), Command::Start);
    assert_eq!(parse("stop"), Command::Stop);
    assert_eq!(parse("set ocfilter"), Command::ClearClassFilter);
    assert_eq!(parse("clear ocfilter"), Command::ClearClassFilter);
    assert_eq!(
        parse("set ocfilter ManaSt"),
        Command::SetClassFilter("manast".to_owned())
    );
    assert_eq!(parse("set source"), Command::SetSource(Target::Selection));
    assert_eq!(parse("set dest"), Command::SetDest(Target::Selection));
    assert_eq!(
        parse("set source player"),
        Command::SetSource(Target::Player)
    );
    assert_eq!(parse("set dest pack 3"), Command::SetDest(Target::Pack(3)));
    assert_eq!(parse("set dest pack3"), Command::SetDest(Target::Pack(3)));
    assert_eq!(
        parse("set source 1342177281"),
        Command::SetSource(Target::Id(0x5000_0001))
    );
    assert_eq!(
        parse("set source packs"),
        Command::SetSource(Target::Invalid("packs".to_owned()))
    );
    assert_eq!(
        parse("set flags oc,al-"),
        Command::SetFlags("OC,AL-".to_owned())
    );
    assert_eq!(
        parse("frobnicate"),
        Command::Unknown("/ms frobnicate".to_owned())
    );
}

#[test]
fn ids_read_as_int32_parse_and_more() {
    assert_eq!(parse_id(" 42 "), Some(42));
    assert_eq!(parse_id("-2147483647"), Some(0x8000_0001));
    assert_eq!(parse_id("2147483649"), Some(0x8000_0001));
    assert_eq!(parse_id("0x80000001"), Some(0x8000_0001));
    assert_eq!(parse_id("abc"), None);
}

#[test]
fn help_lists_mudsorts_lines() {
    let mut h = Harness::logged_in(|_| {});
    let before = h.chat().len();
    h.ms("");
    let lines: Vec<String> = h.chat()[before..].to_vec();
    let mut expected = vec![format!("[mudsort]: {HELP_HEADER}")];
    expected.extend(HELP.iter().map(|l| format!("[mudsort]: {l}")));
    assert_eq!(lines, expected);
    assert_eq!(lines.len(), 8);
}

#[test]
fn source_and_dest_are_the_character_at_login() {
    let h = Harness::logged_in(|_| {});
    assert_eq!(h.plugin.source(), ME);
    assert_eq!(h.plugin.dest(), ME);
    assert_eq!(h.chat()[0], "[mudsort]: Plugin online.");
}

#[test]
fn pack_n_is_the_pack_in_pack_slot_n_minus_one() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("set source pack 2");
    assert_eq!(h.plugin.source(), PACK2);
    assert_eq!(
        last(&h),
        format!("[mudsort]: Setting Source Container to: {PACK2}...")
    );
    h.ms("set dest pack 1");
    assert_eq!(h.plugin.dest(), PACK1);
    assert_eq!(
        last(&h),
        format!("[mudsort]: Setting Destination Container to: {PACK1}...")
    );
    // No such pack: nothing said, nothing changed.
    let said = h.chat().len();
    h.ms("set dest pack 9");
    assert_eq!(h.plugin.dest(), PACK1);
    assert_eq!(h.chat().len(), said);
}

#[test]
fn player_is_the_character_itself() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("set dest pack 1");
    h.ms("set dest player");
    assert_eq!(h.plugin.dest(), ME);
    assert_eq!(
        last(&h),
        format!("[mudsort]: Setting Destination Container to Player ID: {ME}")
    );
}

#[test]
fn an_id_or_the_selection_is_checked_like_set_source_container() {
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(0x8000_2001, "Gem", ObjectClass::Gem, ME, 0, true);
    });
    h.ms(&format!("set source {PACK1}"));
    assert_eq!(h.plugin.source(), PACK1);
    assert_eq!(
        last(&h),
        format!("[mudsort]: Setting Source Container to: {PACK1}...")
    );
    // A gem holds no items: invalid (mudsort's field read "invalid!").
    h.ms(&format!("set source {}", 0x8000_2001u32));
    assert_eq!(h.plugin.source(), 0);
    // Nothing the client knows: invalid too.
    h.ms("set source 77");
    assert_eq!(h.plugin.source(), 0);

    h.sim.selected = Some(PACK2);
    h.ms("set source");
    assert_eq!(h.plugin.source(), PACK2);
    assert_eq!(
        last(&h),
        "[mudsort]: Setting Source Container to Selection..."
    );
    h.sim.selected = Some(HORAN);
    h.ms("set dest");
    assert_eq!(h.plugin.dest(), HORAN);
    h.sim.selected = None;
    h.ms("set dest");
    assert_eq!(h.plugin.dest(), 0);
    h.sim.open_chest();
    h.ms(&format!("set dest {CHEST}"));
    assert_eq!(h.plugin.dest(), CHEST);
}

#[test]
fn an_argument_naming_nothing_is_reported() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("set source pack x");
    assert_eq!(h.plugin.source(), ME);
    assert_eq!(last(&h), "[mudsort]: not a container: pack x");
}

#[test]
fn flags_set_the_sort_string_upper_cased() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("set flags oc,al-,zz");
    assert_eq!(h.plugin.settings().sort_string, "OC,AL-,ZZ");
    let chat = h.chat();
    assert_eq!(
        chat[chat.len() - 2],
        "[mudsort]: Updated Sort Flags: OC,AL-,ZZ"
    );
    assert_eq!(
        chat[chat.len() - 1],
        "[mudsort]: not a sort key, skipped: ZZ"
    );
}

#[test]
fn the_class_filter_is_set_and_cleared() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("set ocfilter Armor");
    assert_eq!(h.plugin.settings().class_filter, "armor");
    assert_eq!(last(&h), "[mudsort]: Setting ObjectClass Filter: armor");
    h.ms("clear ocfilter");
    assert_eq!(h.plugin.settings().class_filter, "");
    assert_eq!(last(&h), "[mudsort]: Clearing ObjectClass Filter.");
    h.ms("set ocfilter gem");
    h.ms("set ocfilter");
    assert_eq!(h.plugin.settings().class_filter, "");
}

#[test]
fn start_and_stop_say_so() {
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(0x8000_2001, "Gem", ObjectClass::Gem, ME, 0, true);
    });
    h.ms("start");
    assert!(h.chat().contains(&"[mudsort]: Starting Sort...".to_owned()));
    assert!(h.plugin.sorter().phase().is_running());
    h.ms("stop");
    assert_eq!(last(&h), "[mudsort]: Stopping Sort...");
    assert_eq!(h.plugin.sorter().phase(), Phase::Idle);
}

#[test]
fn an_unknown_command_is_answered() {
    let mut h = Harness::logged_in(|_| {});
    h.ms("frobnicate");
    assert_eq!(
        last(&h),
        "[mudsort]: unknown command; /ms help lists the commands"
    );
}

#[test]
fn the_verb_is_registered() {
    let h = Harness::logged_in(|_| {});
    assert!(h.sim.fake.commands.iter().any(|c| c.verb == "ms"));
}
