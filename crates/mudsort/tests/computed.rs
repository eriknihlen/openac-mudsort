//! The computed keys (`SortFlag.cs`, `directValueOf`), in readable form;
//! `sort.rs` checks every one of them against mudsort's code.

use mudsort::codes::Computed;
use mudsort::computed::{Value, class_name, dotnet_int, value};
use mudsort::keys::{DecalItem, key};
use openac_plugin::world::ObjectClass;

fn item(longs: &[(u32, i32)], doubles: &[(u32, f64)], spells: &[u32]) -> DecalItem {
    DecalItem {
        longs: longs.iter().copied().collect(),
        doubles: doubles.iter().copied().collect(),
        spells: spells.to_vec(),
        ..DecalItem::default()
    }
}

/// `BUFFED_WEAPON_DAMAGE`: maximum damage plus each damage cantrip
/// (Legendary Blood Thirst 6089 adds 10); other spells add nothing.
#[test]
fn buffed_weapon_damage_adds_the_cantrips() {
    let sword = item(&[(key::MAX_DAMAGE, 50)], &[], &[6089, 2453, 1234]);
    assert_eq!(value(Computed::BuffedWeaponDamage, &sword), Value::Int(62));
}

/// `BUFFED_ARMOR_LEVEL`: armor level plus each Impenetrability cantrip.
#[test]
fn buffed_armor_level_adds_the_cantrips() {
    let coat = item(&[(key::ARMOR_LEVEL, 500)], &[], &[2604, 4667, 2948]);
    assert_eq!(value(Computed::BuffedArmorLevel, &coat), Value::Int(800));
}

/// `BUFFED_MANA_CONVERSION` multiplies; the others add.
#[test]
fn mana_conversion_multiplies() {
    let orb = item(&[], &[(key::MANA_C_BONUS, 0.5)], &[3199, 3202, 3200, 6086]);
    let Value::Double(v) = value(Computed::BuffedManaConversion, &orb) else {
        panic!()
    };
    assert_eq!(v, 0.5 * 1.1 * 1.15 * 1.2 * 1.25);
    let blade = item(&[], &[(key::ATTACK_BONUS, 1.1)], &[2438, 6094]);
    assert_eq!(
        value(Computed::BuffedAttackBonus, &blade),
        Value::Double(1.1 + 0.01 + 0.09)
    );
}

/// `TOTAL_MISSILE_DAMAGE`: a third of the damage modifier's percentage,
/// truncated, plus the elemental bonus and the damage cantrips; 0 without
/// a damage modifier, whatever else the item has.
#[test]
fn total_missile_damage() {
    let bow = item(
        &[(key::ELEMENTAL_DMG_BONUS, 20)],
        &[(key::DAMAGE_BONUS, 2.5)],
        &[2629, 3828],
    );
    // (2.5 - 1) * 100 / 3 = 50, + 20, + 5 + 3.
    assert_eq!(value(Computed::TotalMissileDamage, &bow), Value::Int(78));
    let plain = item(&[(key::ELEMENTAL_DMG_BONUS, 20)], &[], &[2629]);
    assert_eq!(value(Computed::TotalMissileDamage, &plain), Value::Int(0));
}

/// `TOTAL_SUMMON_DAMAGE`: mudsort's formula; with no ratings it is about
/// 55.86, which sorts as 558608.
#[test]
fn total_summon_damage() {
    let none = item(&[], &[], &[]);
    let Value::Double(v) = value(Computed::TotalSummonDamage, &none) else {
        panic!()
    };
    assert_eq!(dotnet_int(v * 10000.0), 558_608);
}

/// `CALCED_TOTAL_RATINGS`: the eight gear ratings summed.
#[test]
fn total_ratings_sum_eight_ratings() {
    let ring = item(
        &[
            (key::DAM_RATING, 3),
            (key::DAM_RESIST_RATING, 4),
            (key::CRIT_RATING, 5),
            (key::CRIT_RESIST, 6),
            (key::CRIT_DAM_RATING, 7),
            (key::CRIT_DAM_RESIST_RATING, 8),
            (key::HEAL_BOOST_RATING, 9),
            (key::VITALITY_RATING, 10),
            // Not a rating mudsort counts.
            (377, 100),
        ],
        &[],
        &[],
    );
    assert_eq!(value(Computed::TotalRatings, &ring), Value::Int(52));
}

/// C#'s `(int)` of a double on x86: toward zero, NaN and out of range give
/// `int.MinValue` (the golden `decimals` case shows .NET doing it).
#[test]
fn dotnet_int_truncates_like_x86() {
    assert_eq!(dotnet_int(11499.999), 11499);
    assert_eq!(dotnet_int(-500.7), -500);
    assert_eq!(dotnet_int(f64::NAN), i32::MIN);
    assert_eq!(dotnet_int(1e14), i32::MIN);
    assert_eq!(dotnet_int(-1e14), i32::MIN);
    assert_eq!(dotnet_int(2_147_483_647.9), i32::MAX);
}

/// OpenAC-rust's object classes have Decal's numbers and names
/// (`Decal.Adapter.Wrappers.ObjectClass`); mudsort sorts by the name.
#[test]
fn class_names_are_decals() {
    for class in ObjectClass::ALL {
        assert_eq!(class_name(*class), format!("{class:?}"));
    }
    assert_eq!(ObjectClass::WandStaffOrb.tag(), 31);
    assert_eq!(ObjectClass::Foci.tag(), 38);
    assert_eq!(ObjectClass::Scroll.tag(), 42);
}
