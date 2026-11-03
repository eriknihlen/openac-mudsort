//! The string order against mudsort's `AlphanumComparator.Compare`, run on
//! .NET Framework under en-US (`tools/golden/Harness.cs`, `Alphanum`).

mod common;

use std::cmp::Ordering;

use mudsort::alphanum::{compare, dotnet_sort};

fn sign(c: char) -> Ordering {
    match c {
        '<' => Ordering::Less,
        '=' => Ordering::Equal,
        '>' => Ordering::Greater,
        other => panic!("bad cell {other}"),
    }
}

/// Every pair of the corpus (numbers as the sort writes them, class names,
/// item names, punctuation, case, accents, and every printable ASCII
/// character) compares as mudsort's comparator compared it.
#[test]
fn every_pair_matches_the_comparator_on_dotnet() {
    let golden = common::golden("alphanum_golden.txt");
    let strings: Vec<&str> = golden
        .lines()
        .filter_map(|l| l.strip_prefix("S\t"))
        .map(common::unbracket)
        .collect();
    let rows: Vec<&str> = golden
        .lines()
        .filter_map(|l| l.strip_prefix("M\t"))
        .collect();
    assert_eq!(rows.len(), strings.len());
    let mut wrong = Vec::new();
    for (a, row) in strings.iter().zip(&rows) {
        for (b, cell) in strings.iter().zip(row.chars()) {
            let expected = sign(cell);
            let actual = compare(a, b);
            if actual != expected {
                wrong.push(format!("{a:?} vs {b:?}: {actual:?}, .NET {expected:?}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} pairs differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// Digit runs compare as numbers (`AlphanumComparator.Compare`, the
/// `Convert.ToInt64` branch): 9 before 10, and "01" equals "1".
#[test]
fn digit_runs_compare_as_numbers() {
    assert_eq!(compare("9", "10"), Ordering::Less);
    assert_eq!(compare("a9", "a10"), Ordering::Less);
    assert_eq!(compare("x9y", "x10y"), Ordering::Less);
    assert_eq!(compare("01", "1"), Ordering::Equal);
    assert_eq!(compare("Salvage (25)", "Salvage (100)"), Ordering::Less);
}

/// A negative number is a "-" run then a digit run, so negatives come
/// before zero and among themselves by size, smallest magnitude first:
/// -1, -5, -10, 0, 2.
#[test]
fn negatives_sort_by_magnitude_before_zero() {
    let mut values = vec!["2", "-10", "0", "-1", "-5"];
    values.sort_by(|a, b| compare(a, b));
    assert_eq!(values, ["-1", "-5", "-10", "0", "2"]);
}

/// The string that runs out first is smaller (`Compare`, the marker tests).
#[test]
fn a_prefix_sorts_first() {
    assert_eq!(compare("", "a"), Ordering::Less);
    assert_eq!(compare("Pyreal", "Pyreal Mote"), Ordering::Less);
    assert_eq!(compare("", ""), Ordering::Equal);
}

/// Text runs compare in the culture's order: case only breaks ties (lower
/// case first), apostrophes and hyphens only after that.
#[test]
fn text_runs_compare_in_culture_order() {
    assert_eq!(compare("apple", "Banana"), Ordering::Less);
    assert_eq!(compare("apple", "Apple"), Ordering::Less);
    assert_eq!(compare("coop", "co-op"), Ordering::Less);
    assert_eq!(compare("co-op", "Coop"), Ordering::Less);
    assert_eq!(compare("Asherons Call", "Asheron's Call"), Ordering::Less);
}

/// `ArrayList.Sort(new AlphanumComparator())` on .NET Framework over 200
/// shuffled lists of distinct strings in classes the comparator calls equal
/// ("x 1", "x 01", ... "x 00000001"): .NET's sort is not stable, and the
/// order it leaves each class in is reproduced exactly.
#[test]
fn dotnet_sort_matches_arraylist_sort() {
    let golden = common::golden("dotnet_sort_golden.txt");
    let lines: Vec<&str> = golden.lines().collect();
    assert_eq!(lines.len(), 400);
    for pair in lines.chunks(2) {
        let mut list: Vec<&str> = pair[0].strip_prefix("I\t").unwrap().split('\t').collect();
        let expected: Vec<&str> = pair[1].strip_prefix("O\t").unwrap().split('\t').collect();
        dotnet_sort(&mut list, |a, b| compare(a, b));
        assert_eq!(list, expected);
    }
}

/// .NET's unstable sort leaves values the comparer calls equal in an order
/// of its own; the golden `leading_zero_names` case pins it.
#[test]
fn dotnet_sort_orders_like_dotnet() {
    let mut values = vec![
        "item 01", "item 1", "item 2", "Item 1", "item 10", "item 001",
    ];
    dotnet_sort(&mut values, |a, b| compare(a, b));
    assert_eq!(
        values,
        [
            "item 001", "item 1", "item 01", "item 2", "item 10", "Item 1"
        ]
    );
    let mut many: Vec<String> = (0..500).map(|i| ((i * 7919) % 1000).to_string()).collect();
    dotnet_sort(&mut many, |a, b| compare(a, b));
    assert!(
        many.windows(2)
            .all(|w| compare(&w[0], &w[1]) != Ordering::Greater)
    );
}
