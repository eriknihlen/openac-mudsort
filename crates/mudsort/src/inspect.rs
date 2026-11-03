//! What the Inspector shows: an item's value for each sort key, as mudsort
//! showed it (`SortFlag.directValueOf` written with .NET's `ToString`).
//! It replaces mudsort's property dump to chat; [`dump_line`] is that dump
//! for one key (`SortFlag.propertyDumpSelection`).

use crate::codes::{self, Category, Flag, Key};
use crate::computed::{self, Value, class_name};
use crate::keys::{DecalItem, ItemView, key};

/// One key's value for an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorEntry {
    /// The two-letter code.
    pub code: &'static str,
    /// mudsort's name for the key.
    pub name: &'static str,
    /// Its plain-English name.
    pub display_name: &'static str,
    /// Its category.
    pub category: Category,
    /// The value, as mudsort wrote it.
    pub value: String,
}

/// A key's value as mudsort wrote it (`directValueOf(obj).ToString()`):
/// a decimal in .NET's general format, a yes-no as `True` or `False`, an
/// object class by its name. The sort itself reads [`crate::sort::value_of`].
#[must_use]
pub fn direct_value(flag: &Flag, item: &impl ItemView) -> String {
    match flag.key {
        Key::Computed(c) => match computed::value(c, item) {
            Value::Int(value) => value.to_string(),
            Value::Double(value) => dotnet_double(value),
            Value::Class(class) => class_name(class).to_owned(),
        },
        Key::Long(k) => item.long(k).to_string(),
        Key::Double(k) => dotnet_double(item.double(k)),
        Key::String(k) => item.string(k).to_owned(),
        Key::Bool(k) => if item.bool(k) { "True" } else { "False" }.to_owned(),
    }
}

/// Every key the item has a value for, in the code table's order: the
/// computed keys always, a table key when the item's table holds it.
#[must_use]
pub fn inspect(item: &DecalItem) -> Vec<InspectorEntry> {
    codes::table()
        .flags
        .iter()
        .filter(|flag| match flag.key {
            Key::Computed(_) => true,
            Key::Long(k) => item.longs.contains_key(&k),
            Key::Double(k) => item.doubles.contains_key(&k),
            Key::String(k) => item.strings.contains_key(&k),
            Key::Bool(k) => item.bools.contains_key(&k),
        })
        .map(|flag| InspectorEntry {
            code: flag.code(),
            name: flag.name,
            display_name: flag.display_name(),
            category: flag.category(),
            value: direct_value(flag, item),
        })
        .collect()
}

/// mudsort's dump of one key for the selected item
/// (`propertyDumpSelection`): `<item name> : <key name> : <value>`.
#[must_use]
pub fn dump_line(flag: &Flag, item: &impl ItemView) -> String {
    format!(
        "{} : {} : {}",
        item.string(key::NAME),
        flag.name,
        direct_value(flag, item)
    )
}

/// A double as .NET Framework's `Double.ToString()` writes it: fifteen
/// significant digits, trailing zeros dropped, scientific from 1E+15 and
/// below 0.0001 (1E-05).
#[must_use]
pub fn dotnet_double(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    let scientific = format!("{value:.14e}");
    let rounded: f64 = scientific.parse().unwrap_or(value);
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..15).contains(&exponent) {
        return rounded.to_string();
    }
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}E{sign}{:02}", exponent.abs())
}
