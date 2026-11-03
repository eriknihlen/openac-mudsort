//! Reading the golden files `tools/golden/run.ps1` wrote by running
//! mudsort's own code (`SortFlag.cs`, `Enums.cs`, `AlphanumComparator.cs`
//! and `PluginCore.cs`'s sort loop and parser) on .NET Framework.

#![allow(dead_code)]

use mudsort::keys::DecalItem;
use openac_plugin::world::ObjectClass;

/// A golden file's text.
pub fn golden(name: &str) -> String {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A string written between brackets, so spaces at either end show.
pub fn unbracket(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or_else(|| panic!("not bracketed: {text:?}"))
}

/// One case of `items.txt`.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub sort_string: String,
    pub reverse: bool,
    pub items: Vec<DecalItem>,
}

/// Every case of `items.txt`.
pub fn cases() -> Vec<Case> {
    let text = golden("items.txt");
    let mut cases = Vec::new();
    let mut current: Option<Case> = None;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields[0] {
            "case" => {
                current = Some(Case {
                    name: fields[1].to_owned(),
                    sort_string: fields[2].to_owned(),
                    reverse: fields[3] == "1",
                    items: Vec::new(),
                });
            }
            "item" => current.as_mut().unwrap().items.push(item(&fields)),
            "end" => cases.push(current.take().unwrap()),
            other => panic!("unknown line {other}"),
        }
    }
    cases
}

fn item(fields: &[&str]) -> DecalItem {
    let mut item = DecalItem {
        id: fields[1].parse().unwrap(),
        object_class: ObjectClass::from_tag(fields[2].parse().unwrap()).unwrap(),
        ..DecalItem::default()
    };
    for field in &fields[3..] {
        if let Some(spells) = field.strip_prefix("P=") {
            item.spells.extend(
                spells
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse::<u32>().unwrap()),
            );
            continue;
        }
        let (key, value) = field[1..].split_once('=').unwrap();
        let key: u32 = key.parse().unwrap();
        match &field[..1] {
            "L" => {
                item.longs.insert(key, value.parse().unwrap());
            }
            "D" => {
                item.doubles.insert(key, value.parse().unwrap());
            }
            "S" => {
                item.strings.insert(key, value.to_owned());
            }
            "B" => {
                item.bools.insert(key, value == "True");
            }
            other => panic!("unknown field kind {other}"),
        }
    }
    item
}
