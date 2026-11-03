//! The code table against mudsort's own (`SortFlag.cs`, the static
//! constructor, over `Enums.cs`).

mod common;

use mudsort::codes::{self, Category, Computed, Key};

/// The table mudsort's static constructor registers, as the harness read it
/// from `SortFlag.sortedFlagList` (code, kind, name, key value, common).
#[test]
fn every_code_matches_mudsorts_registered_flags() {
    let golden = common::golden("flags_golden.txt");
    let mut expected: Vec<String> = golden
        .lines()
        .filter(|line| !line.starts_with("log\t"))
        .map(str::to_owned)
        .collect();
    expected.sort();
    let mut actual: Vec<String> = codes::table()
        .flags
        .iter()
        .map(|flag| {
            let (kind, value) = match flag.key {
                Key::Computed(_) => ("Computed", "-".to_owned()),
                Key::String(v) => ("String", v.to_string()),
                Key::Long(v) => ("Long", v.to_string()),
                Key::Double(v) => ("Double", v.to_string()),
                Key::Bool(v) => ("Bool", v.to_string()),
            };
            let common = if flag.common { "common" } else { "-" };
            format!("{}\t{kind}\t{}\t{value}\t{common}", flag.code(), flag.name)
        })
        .collect();
    actual.sort();
    assert_eq!(actual.len(), 113);
    assert_eq!(actual, expected);
}

/// The table `tools/golden/mudsort_codes.py` derived from `Enums.cs`
/// independently: the same 103 enum codes, in mudsort's registration order,
/// and the same three keys without a code.
#[test]
fn registration_order_and_uncoded_keys_match_the_derived_table() {
    let golden = common::golden("code_table.txt");
    let table = codes::table();
    let enum_flags: Vec<_> = table
        .flags
        .iter()
        .filter(|f| !matches!(f.key, Key::Computed(_)))
        .collect();
    let mut flags = enum_flags.iter();
    let mut uncoded = Vec::new();
    for line in golden.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        if fields[0] == "DUP" {
            uncoded.push(fields[2]);
            continue;
        }
        let flag = flags.next().expect("more golden codes than flags");
        assert_eq!(flag.code(), fields[0], "{line}");
        assert_eq!(flag.name, fields[2], "{line}");
    }
    assert!(flags.next().is_none());
    let actual: Vec<_> = table.uncoded.iter().map(|(name, _)| *name).collect();
    assert_eq!(actual, uncoded);
    assert_eq!(actual, ["SpellCount", "Slot", "Scale"]);
}

/// mudsort logs one "duplicate code entry" per key left without a code
/// (`SortFlag.cs`, `Util.Log`): the code it last tried.
#[test]
fn three_keys_get_no_code_as_mudsort_logged() {
    let golden = common::golden("flags_golden.txt");
    let logged: Vec<_> = golden
        .lines()
        .filter_map(|l| l.strip_prefix("log\tduplicate code entry : "))
        .collect();
    assert_eq!(logged, ["SP", "SL", "SC"]);
    assert_eq!(codes::table().uncoded.len(), 3);
}

/// The ten computed flags are registered first with fixed codes
/// (`SortFlag.cs`, the static fields).
#[test]
fn computed_flags_come_first() {
    let table = codes::table();
    let first: Vec<_> = table.flags[..10].iter().map(|f| f.code()).collect();
    assert_eq!(
        first,
        ["OC", "TR", "BW", "BE", "BD", "BA", "BP", "BM", "TM", "TS"]
    );
    assert_eq!(
        codes::by_code("OC").unwrap().key,
        Key::Computed(Computed::ObjectClass)
    );
}

/// `SortFlag.CommonFlags` spells one name "SkilLevelReq", which matches no
/// key: `SkillLevelReq` (SL) is not common, in mudsort or here.
#[test]
fn common_flags_include_the_misspelt_one_as_mudsort_does() {
    let table = codes::table();
    assert!(!table.by_name("SkillLevelReq").unwrap().common);
    assert!(table.by_name("ArmorLevel").unwrap().common);
    // "Heritage" is common in the list but is not a mudsort key.
    assert!(table.by_name("Heritage").is_none());
    assert_eq!(table.flags.iter().filter(|f| f.common).count(), 29);
}

#[test]
fn every_flag_has_a_display_name_and_category() {
    for flag in &codes::table().flags {
        assert!(!flag.display_name().is_empty(), "{}", flag.name);
    }
    for (name, _) in &codes::table().uncoded {
        assert!(!codes::display_name(name).is_empty(), "{name}");
    }
    assert_eq!(codes::by_code("NM").unwrap().category(), Category::Text);
    assert_eq!(codes::by_code("AL").unwrap().category(), Category::Number);
    assert_eq!(codes::by_code("MB").unwrap().category(), Category::Decimal);
    assert_eq!(codes::by_code("RT").unwrap().category(), Category::YesNo);
    assert_eq!(codes::by_code("BW").unwrap().category(), Category::Computed);
    assert_eq!(codes::by_code("AL").unwrap().display_name(), "Armor level");
}
