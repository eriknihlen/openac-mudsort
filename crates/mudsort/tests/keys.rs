//! Reading an item the client reports as Decal's tables held it (Mosswart
//! Massacre's `inventory/builder.rs`, `create`, which reproduces Decal's
//! item tables).

use mudsort::keys::{DecalItem, ItemView, key, short_icon};
use mudsort::sort::value_of;
use openac_plugin::items::{InventoryItem, ObjectHeader};
use openac_plugin::world::{ArmorProfile, ItemProperties, ObjectClass, WeaponProfile};

fn sword() -> InventoryItem {
    InventoryItem {
        object_id: 0x8000_1234,
        weenie_class_id: 3001,
        name: "Spear".to_owned(),
        item_type: 1,
        container_id: 0x5000_000A,
        valid_locations: 0x0010_0000,
        object_class: ObjectClass::MeleeWeapon,
        icon_id: 0x0600_1234,
        burden: 200,
        value: 1500,
        material_type: 60,
        container_slot: 4,
        appraised_spell_ids: vec![2453, 6089, 0x8000_0000 | 2453],
        ..InventoryItem::default()
    }
}

fn weapon_properties() -> ItemProperties {
    ItemProperties {
        ints: vec![(28, 0), (370, 3), (5, 999)],
        floats: vec![(29, 1.15)],
        strings: vec![(7, "Mine".to_owned())],
        bools: vec![(91, true)],
        weapon: Some(WeaponProfile {
            damage: 50,
            damage_type: 1,
            weapon_time: 35,
            weapon_skill: 44,
            damage_variance: 0.5,
            damage_mod: 1.0,
            weapon_offense: 1.18,
            max_velocity: 0.0,
            ..WeaponProfile::default()
        }),
        ..ItemProperties::default()
    }
}

/// The object's own fields go under Decal's keys; an appraisal's property
/// tables under their AC ids; the weapon profile under Decal's weapon keys.
#[test]
fn a_weapon_reads_as_decal_read_it() {
    let item = DecalItem::from_inventory(&sword(), Some(&weapon_properties()));
    assert_eq!(item.id(), 0x8000_1234);
    assert_eq!(item.container(), 0x5000_000A);
    assert_eq!(item.object_class(), ObjectClass::MeleeWeapon);
    assert_eq!(item.string(key::NAME), "Spear");
    assert_eq!(item.string(7), "Mine");
    assert_eq!(item.long(key::TYPE), 3001);
    assert_eq!(item.long(key::ICON), 0x1234);
    assert_eq!(item.long(key::CATEGORY), 1);
    assert_eq!(item.long(key::EQUIPABLE_SLOTS), 0x0010_0000);
    // The object's burden is what Decal kept, over the appraisal's.
    assert_eq!(item.long(key::BURDEN), 200);
    assert_eq!(item.long(key::VALUE), 1500);
    assert_eq!(item.long(key::MATERIAL), 60);
    assert_eq!(item.long(key::SLOT), 4);
    assert_eq!(item.long(key::DAM_RATING), 3);
    assert_eq!(item.long(key::MAX_DAMAGE), 50);
    assert_eq!(item.long(key::WEAP_SPEED), 35);
    assert_eq!(item.long(key::EQUIP_SKILL), 44);
    assert_eq!(item.double(key::MELEE_DEFENSE_BONUS), 1.15);
    assert_eq!(item.double(key::ATTACK_BONUS), 1.18);
    assert_eq!(item.double(key::VARIANCE), 0.5);
    assert!(item.bool(91));
    // The spells it carries, without its own running enchantments.
    assert_eq!(item.spells(), [2453, 6089]);
    assert_eq!(item.long(key::SPELL_COUNT), 2);
    // Never sent: Decal's defaults.
    assert_eq!(item.long(key::STACK_COUNT), 0);
    assert_eq!(item.long(key::SLOT_LEGACY), 0);
    assert_eq!(item.string(key::SECONDARY_NAME), "");
}

/// Decal always held a value, -1 when the server sent none; an appraised
/// value stands when the object carried none.
#[test]
fn value_defaults_to_minus_one() {
    let mut plain = sword();
    plain.value = 0;
    let item = DecalItem::from_inventory(&plain, None);
    assert_eq!(item.long(key::VALUE), -1);
    let appraised = ItemProperties {
        ints: vec![(19, 25)],
        ..ItemProperties::default()
    };
    assert_eq!(
        DecalItem::from_inventory(&plain, Some(&appraised)).long(key::VALUE),
        25
    );
}

/// Once the description's header word is known, a field counts as sent by
/// its bit, so a zero that was sent is held and a value that was not is not.
#[test]
fn header_bits_decide_what_was_sent() {
    let mut stack = sword();
    stack.stack_size = 1;
    stack.maximum_stack_size = 1;
    // Unknown header: a stack of at most 1 is not a stack.
    assert_eq!(
        DecalItem::from_inventory(&stack, None).long(key::STACK_COUNT),
        0
    );
    stack.header = Some(ObjectHeader {
        weenie_header_flags: Some(0x0000_1000 | 0x0000_2000),
        setup_id: Some(0x0200_0001),
        ..ObjectHeader::default()
    });
    let item = DecalItem::from_inventory(&stack, None);
    assert_eq!(item.long(key::STACK_COUNT), 1);
    assert_eq!(item.long(key::STACK_MAX), 1);
    assert_eq!(item.long(key::MODEL), 0x0200_0001);
    // Burden, value and material were not flagged: not held.
    assert_eq!(item.long(key::BURDEN), 0);
    assert_eq!(item.long(key::VALUE), -1);
    assert_eq!(item.long(key::MATERIAL), 0);
}

/// A wielded item: its container is the wielder, and its old-style slot
/// reads -1.
#[test]
fn a_wielded_item() {
    let mut wielded = sword();
    wielded.container_id = 0;
    wielded.wielder_id = 0x5000_000A;
    wielded.equipped_location = 0x0010_0000;
    wielded.container_slot = -1;
    let item = DecalItem::from_inventory(&wielded, None);
    assert_eq!(item.container(), 0x5000_000A);
    assert_eq!(item.long(key::EQUIPPED_SLOTS), 0x0010_0000);
    assert_eq!(item.long(key::SLOT), -1);
    assert_eq!(item.long(key::SLOT_LEGACY), -1);
}

/// Armor's protections come from its appraisal profile, as multipliers.
#[test]
fn armor_protections() {
    let properties = ItemProperties {
        armor: Some(ArmorProfile {
            armor_level: 500,
            slash_mod: 1.2,
            electric_mod: 0.6,
            ..ArmorProfile::default()
        }),
        ..ItemProperties::default()
    };
    let item = DecalItem::from_inventory(&sword(), Some(&properties));
    assert_eq!(item.double(key::SLASH_PROT), f64::from(1.2f32));
    assert_eq!(item.double(key::LIGHTNING_PROT), f64::from(0.6f32));
    // The f32 widened to a double sorts as Decal's did: 1.2000000476... x
    // 10,000, truncated.
    let flag = mudsort::codes::by_code("SH").unwrap();
    assert_eq!(value_of(flag, &item), "12000");
}

#[test]
fn icons_lose_the_table_prefix() {
    assert_eq!(short_icon(0x0600_5B0C), 0x5B0C);
    assert_eq!(short_icon(0x1234), 0x1234);
}
