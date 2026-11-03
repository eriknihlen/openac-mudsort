//! Which items a sort takes (`PluginCore.cs`, `activate` and
//! `addWorldObject`).

use mudsort::keys::{DecalItem, ItemView, key};
use mudsort::select::select;
use openac_plugin::world::ObjectClass;

const PACK: u32 = 0x5000_0001;
const OTHER: u32 = 0x5000_0002;

fn item(id: u32, container: u32, class: ObjectClass, slot: i32, equipped: i32) -> DecalItem {
    let mut item = DecalItem {
        id,
        container,
        object_class: class,
        ..DecalItem::default()
    };
    item.longs.insert(key::SLOT, slot);
    if equipped != 0 {
        item.longs.insert(key::EQUIPPED_SLOTS, equipped);
    }
    item
}

fn ids(items: &[&DecalItem]) -> Vec<u32> {
    items.iter().map(|i| i.id()).collect()
}

/// `activate`: in the source, not worn (EquippedSlots 0), with a slot
/// (Slot not -1), not a focus; `addWorldObject`: a pack is not taken and,
/// without recursion, not opened.
#[test]
fn takes_loose_items_of_the_source_only() {
    let items = [
        item(1, PACK, ObjectClass::Armor, 0, 0),
        item(2, OTHER, ObjectClass::Armor, 0, 0),
        item(3, PACK, ObjectClass::Armor, -1, 0),
        item(4, PACK, ObjectClass::Armor, 2, 0x200),
        item(5, PACK, ObjectClass::Foci, 3, 0),
        item(6, PACK, ObjectClass::Container, 4, 0),
        item(7, 6, ObjectClass::Gem, 0, 0),
        item(8, PACK, ObjectClass::Gem, 5, 0),
    ];
    assert_eq!(ids(&select(&items, PACK, None)), [1, 8]);
}

/// A wielded item's container is its wielder (Decal's `Container`), so a
/// sort of the character's own items meets it, and drops it as equipped.
#[test]
fn a_wielded_item_is_dropped_as_equipped() {
    let mut sword = item(1, PACK, ObjectClass::MeleeWeapon, -1, 0x0010_0000);
    sword.longs.remove(&key::SLOT);
    assert!(select(&[sword], PACK, None).is_empty());
}

/// The class filter keeps classes whose name starts with it, ignoring case
/// (`ObjectClass.ToString().ToLower().StartsWith(ocfilter.ToLower())`).
#[test]
fn class_filter_is_a_case_insensitive_prefix() {
    let items = [
        item(1, PACK, ObjectClass::MeleeWeapon, 0, 0),
        item(2, PACK, ObjectClass::ManaStone, 1, 0),
        item(3, PACK, ObjectClass::MissileWeapon, 2, 0),
        item(4, PACK, ObjectClass::Salvage, 3, 0),
    ];
    assert_eq!(ids(&select(&items, PACK, Some("m"))), [1, 2, 3]);
    assert_eq!(ids(&select(&items, PACK, Some("MELEE"))), [1]);
    assert_eq!(ids(&select(&items, PACK, Some("salvage"))), [4]);
    assert_eq!(
        ids(&select(&items, PACK, Some("Weapon"))),
        Vec::<u32>::new()
    );
    assert_eq!(ids(&select(&items, PACK, Some(""))), [1, 2, 3, 4]);
}
