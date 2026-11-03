//! mudsort's sort (`PluginCore.cs`, the `BUILDING_LIST` state): one pass
//! per key, last key first. A pass reads every item's value for the key as
//! a string (`SortFlag.valueOf`), sorts the distinct values with the
//! alphanumeric order (reversed for a descending key), and regroups the
//! items by value, keeping their order within a value. The first key is
//! applied last, so it decides; later keys break its ties.

use std::collections::{HashMap, HashSet};

use crate::alphanum;
use crate::codes::{Computed, Flag, Key};
use crate::computed::{self, Value, dotnet_int};
use crate::keys::ItemView;
use crate::sort_string::SortKey;

/// The string an item sorts by for a key (`SortFlag.valueOf`): a decimal
/// times 10,000, truncated to a whole number (so 1.15 sorts as 11500);
/// a yes-no as `True` or `False`; an object class by its name; anything
/// else as written.
#[must_use]
pub fn value_of(flag: &Flag, item: &impl ItemView) -> String {
    match flag.key {
        Key::Double(key) => scaled(item.double(key)),
        Key::Computed(c) => match computed::value(c, item) {
            Value::Double(value)
                if matches!(
                    c,
                    Computed::BuffedElementalDamage
                        | Computed::BuffedAttackBonus
                        | Computed::BuffedManaConversion
                        | Computed::BuffedMeleeDefense
                        | Computed::TotalSummonDamage
                ) =>
            {
                scaled(value)
            }
            // No other computed key is a decimal.
            Value::Double(value) => value.to_string(),
            Value::Int(value) => value.to_string(),
            Value::Class(class) => computed::class_name(class).to_owned(),
        },
        Key::Long(key) => item.long(key).to_string(),
        Key::String(key) => item.string(key).to_owned(),
        Key::Bool(key) => if item.bool(key) { "True" } else { "False" }.to_owned(),
    }
}

fn scaled(value: f64) -> String {
    dotnet_int(value * 10000.0).to_string()
}

/// The order mudsort moves `items` in for `keys`, reversed when
/// `reverse_all` (mudsort's "Reverse Sort List" option).
///
/// With no keys mudsort queued nothing, and so does this: the result is
/// empty.
///
/// The distinct values carry over from pass to pass and are re-sorted with
/// .NET's unstable sort each time, as in mudsort; it only shows for values
/// the alphanumeric order calls equal but are not the same text ("item 1"
/// and "item 01").
#[must_use]
pub fn sort<T: ItemView>(items: Vec<T>, keys: &[SortKey], reverse_all: bool) -> Vec<T> {
    if keys.is_empty() {
        return Vec::new();
    }
    let mut list = items;
    let mut values: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for key in keys.iter().rev() {
        let metrics: Vec<String> = list.iter().map(|item| value_of(key.flag, item)).collect();
        for metric in &metrics {
            if seen.insert(metric.clone()) {
                values.push(metric.clone());
            }
        }
        alphanum::dotnet_sort(&mut values, |a, b| alphanum::compare(a, b));
        if key.descending {
            values.reverse();
        }
        let rank: HashMap<&str, usize> = values
            .iter()
            .enumerate()
            .map(|(index, value)| (value.as_str(), index))
            .collect();
        let mut order: Vec<(usize, T)> = metrics
            .iter()
            .map(|metric| rank[metric.as_str()])
            .zip(list)
            .collect();
        // Stable: items with the same value keep their order.
        order.sort_by_key(|(rank, _)| *rank);
        list = order.into_iter().map(|(_, item)| item).collect();
    }
    if reverse_all {
        list.reverse();
    }
    list
}
