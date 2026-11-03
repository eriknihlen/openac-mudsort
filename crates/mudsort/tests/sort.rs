//! The sort and the values it sorts by, against mudsort's own code
//! (`SortFlag.cs`, `valueOf` and `directValueOf`; `PluginCore.cs`, the
//! `BUILDING_LIST` state) over the cases in `tests/golden/items.txt`.

mod common;

use mudsort::codes;
use mudsort::keys::ItemView;
use mudsort::sort::{sort, value_of};
use mudsort::sort_string;

/// Every flag's value for every golden item, as `SortFlag.valueOf` wrote
/// it on .NET Framework (x86): numbers, `True`/`False`, class names,
/// decimals times 10,000 truncated (NaN and out of range read
/// -2147483648), and the ten computed keys.
#[test]
fn every_value_matches_valueof() {
    let golden = common::golden("values_golden.txt");
    let cases = common::cases();
    let mut checked = 0;
    for line in golden.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let case = cases.iter().find(|c| c.name == fields[0]).unwrap();
        let id: u32 = fields[1].parse().unwrap();
        let item = case.items.iter().find(|i| i.id() == id).unwrap();
        for cell in &fields[2..] {
            let (code, expected) = cell.split_once('=').unwrap();
            let flag = codes::by_code(code).unwrap();
            assert_eq!(
                value_of(flag, item),
                expected,
                "case {} item {id} key {code}",
                case.name
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked,
        113 * cases.iter().map(|c| c.items.len()).sum::<usize>()
    );
}

/// Every golden case sorts into the order mudsort's loop produced.
#[test]
fn every_case_sorts_as_mudsort_sorted_it() {
    let golden = common::golden("sort_golden.txt");
    let cases = common::cases();
    assert_eq!(golden.lines().count(), cases.len());
    for line in golden.lines() {
        let (name, order) = line.split_once('\t').unwrap();
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let keys = sort_string::parse(&case.sort_string.to_uppercase()).keys;
        let sorted = sort(case.items.iter().collect(), &keys, case.reverse);
        let actual: Vec<String> = sorted.iter().map(|i| i.id().to_string()).collect();
        assert_eq!(actual.join(" "), order, "case {name}");
    }
}

/// The readable form of three golden cases.
#[test]
fn numbers_ties_and_reversal() {
    let cases = common::cases();
    let order = |name: &str| -> Vec<u32> {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let keys = sort_string::parse(&case.sort_string).keys;
        sort(case.items.iter().collect(), &keys, case.reverse)
            .iter()
            .map(|i| i.id())
            .collect()
    };
    // Armor levels 9, 10, 2, -1, -5, 0, 100, -10, (none), 10: negatives
    // first by magnitude, then 0 (and the item without one), 2, 9, 10, 10,
    // 100; the two 10s keep their order.
    assert_eq!(order("numbers_ascending"), [4, 5, 8, 6, 9, 3, 1, 2, 10, 7]);
    // Descending reverses the value order; ties still keep theirs.
    assert_eq!(order("numbers_descending"), [7, 2, 10, 1, 3, 6, 9, 8, 5, 4]);
    // Names B, A, B, A, C, A: each name's items in their first order.
    assert_eq!(order("ties_keep_input_order"), [2, 4, 6, 1, 3, 5]);
    // No keys: mudsort queues nothing.
    assert!(order("empty_sort_string").is_empty());
    assert!(order("unknown_codes_only").is_empty());
}
