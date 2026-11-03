//! The Inspector (`SortFlag.directValueOf`, `propertyDumpSelection`).

mod sim;

use mudsort::MudSort;
use mudsort::codes;
use mudsort::inspect::{dotnet_double, dump_line, inspect};
use mudsort::keys::{DecalItem, key};
use openac_plugin::world::ObjectClass;
use sim::{Harness, ME};

fn breastplate() -> DecalItem {
    let mut item = DecalItem {
        id: 7,
        object_class: ObjectClass::Armor,
        container: ME,
        ..DecalItem::default()
    };
    item.strings.insert(key::NAME, "Breastplate".to_owned());
    item.longs.insert(key::ARMOR_LEVEL, 210);
    item.doubles.insert(key::SLASH_PROT, 1.15);
    item.bools.insert(1, false);
    item
}

#[test]
fn every_key_with_a_value_and_the_computed_keys() {
    let entries = inspect(&breastplate());
    let find = |code: &str| entries.iter().find(|e| e.code == code);
    assert_eq!(find("OC").unwrap().value, "Armor");
    assert_eq!(find("AL").unwrap().value, "210");
    assert_eq!(find("AL").unwrap().display_name, "Armor level");
    assert_eq!(find("NM").unwrap().value, "Breastplate");
    let slash = codes::table()
        .by_name("SlashProt")
        .map(|f| f.code())
        .unwrap();
    assert_eq!(find(slash).unwrap().value, "1.15");
    // Keys the item has no value for are left out.
    assert!(find("VA").is_none());
    assert!(find("BR").is_none());
    // The ten computed keys are always there.
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.category == codes::Category::Computed)
            .count(),
        10
    );
}

#[test]
fn the_dump_line_is_name_key_value() {
    let item = breastplate();
    let flag = codes::by_code("AL").unwrap();
    assert_eq!(dump_line(flag, &item), "Breastplate : ArmorLevel : 210");
}

#[test]
fn decimals_are_written_as_dotnet_framework_writes_them() {
    assert_eq!(dotnet_double(0.0), "0");
    assert_eq!(dotnet_double(1.15), "1.15");
    assert_eq!(dotnet_double(0.1 + 0.2), "0.3");
    assert_eq!(dotnet_double(f64::from(0.8_f32)), "0.800000011920929");
    assert_eq!(dotnet_double(-2.5), "-2.5");
    assert_eq!(dotnet_double(1e15), "1E+15");
    assert_eq!(dotnet_double(0.00001), "1E-05");
    assert_eq!(dotnet_double(0.0001), "0.0001");
    assert_eq!(dotnet_double(123_456.0), "123456");
}

#[test]
fn the_selected_item_is_inspected_and_dumped() {
    let mut h = Harness::logged_in(|sim| {
        sim.add_item(0x8000_2001, "Gem", ObjectClass::Gem, ME, 12, true);
    });
    let entries = h
        .with_host(|_, host| MudSort::inspect(host, 0x8000_2001))
        .unwrap();
    assert!(entries.iter().any(|e| e.code == "AL" && e.value == "12"));
    h.sim.selected = Some(0x8000_2001);
    assert!(h.with_host(|_, host| MudSort::dump_selection(host, "al")));
    assert_eq!(h.chat().last().unwrap(), "[mudsort]: Gem : ArmorLevel : 12");
    assert!(!h.with_host(|_, host| MudSort::dump_selection(host, "zz")));
}
