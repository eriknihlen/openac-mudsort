//! Sort strings against mudsort's parser (`PluginCore.cs`,
//! `createSortFlagListFromString`; `SortFlag.cs`, `decode`).

mod common;

use mudsort::sort_string::{self, DEFAULT, MUDSORT_DEFAULT};

fn describe(text: &str) -> String {
    sort_string::serialize(&sort_string::parse(text).keys)
}

/// Every golden string decodes as mudsort decoded it upper-cased (the
/// `/ms set flags` path upper-cases; the edit box did not, so there `oc`
/// matched nothing; here case never matters).
#[test]
fn strings_decode_as_mudsort_decodes_them_upper_cased() {
    let golden = common::golden("sort_string_golden.txt");
    for line in golden.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let input = common::unbracket(fields[0]);
        assert_eq!(describe(input), fields[2], "input {input:?}");
    }
}

/// mudsort's own default (`Properties/Settings.settings`) decodes to eight
/// keys: `DG` and `MA` name nothing and are dropped. `ES` survives, but it
/// is EquippedSlots (where an item is worn now), 0 for anything in a pack,
/// so it never orders anything (DIVERGENCES.md, MS-1).
#[test]
fn mudsorts_default_loses_two_codes_and_sorts_by_equipped_slots() {
    let parsed = sort_string::parse(MUDSORT_DEFAULT);
    assert_eq!(
        sort_string::serialize(&parsed.keys),
        "OC,ES,CV,AL-,AB-,ED-,ML,NM"
    );
    assert_eq!(parsed.unknown, ["DG-", "MA"]);
    assert_eq!(parsed.keys[1].flag.name, "EquippedSlots");
}

/// The shipped default keeps mudsort's intent: where it can be worn (EQ),
/// damage modifier (DB) and material (MT) in place of the dead codes.
#[test]
fn the_shipped_default_names_ten_keys() {
    let parsed = sort_string::parse(DEFAULT);
    assert!(parsed.unknown.is_empty());
    let names: Vec<_> = parsed.keys.iter().map(|k| k.flag.name).collect();
    assert_eq!(
        names,
        [
            "ObjectClass",
            "EquipableSlots",
            "Coverage",
            "ArmorLevel",
            "AttackBonus",
            "DamageBonus",
            "ElementalDmgBonus",
            "MinLevelRestrict",
            "Material",
            "Name"
        ]
    );
    assert_eq!(sort_string::serialize(&parsed.keys), DEFAULT);
}

/// A part is descending only when it is exactly three characters and the
/// third is `-` (`createSortFlagListFromString`).
#[test]
fn descending_needs_exactly_three_characters() {
    assert_eq!(describe("AL-"), "AL-");
    assert_eq!(describe("al-"), "AL-");
    assert_eq!(describe("AL- "), "AL");
    assert_eq!(describe("AL--"), "AL");
    assert_eq!(describe("ALX"), "AL");
}

/// Unknown parts are skipped and reported; empty parts are skipped; a part
/// with a space before the code names nothing, as in mudsort.
#[test]
fn unknown_parts_are_reported() {
    let parsed = sort_string::parse("ZZ,,NM, AL,A");
    assert_eq!(sort_string::serialize(&parsed.keys), "NM");
    assert_eq!(parsed.unknown, ["ZZ", " AL", "A"]);
    assert!(sort_string::parse("").keys.is_empty());
    assert!(sort_string::parse("").unknown.is_empty());
}

/// mudsort kept one direction per key: a key named twice takes its last
/// part's direction both times (`SortFlag.descending` is shared).
#[test]
fn a_repeated_key_takes_its_last_direction() {
    assert_eq!(describe("AL,AL-"), "AL-,AL-");
    assert_eq!(describe("AL-,AL"), "AL,AL");
}
