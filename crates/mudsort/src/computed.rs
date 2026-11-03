//! mudsort's computed keys (`SortFlag.cs`, `directValueOf`): each formula
//! and spell table as written. The spell tables are the item's cantrips that
//! raise the value (a Legendary Blood Thirst adds 10 to maximum damage).

use openac_plugin::world::ObjectClass;

use crate::codes::Computed;
use crate::keys::{ItemView, key};

/// A computed value, before mudsort turns it into the string it sorts by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// A whole number.
    Int(i32),
    /// A decimal number.
    Double(f64),
    /// An object class.
    Class(ObjectClass),
}

/// `BUFFED_WEAPON_DAMAGE` and `TOTAL_MISSILE_DAMAGE`: maximum damage added
/// by each Blood Drinker/Thirst cantrip.
pub const WEAPON_DAMAGE_SPELLS: [(u32, i32); 11] = [
    (2453, 2),
    (2486, 2),
    (2487, 2),
    (2598, 2),
    (3828, 3),
    (2454, 4),
    (2586, 4),
    (2629, 5),
    (2452, 6),
    (4661, 7),
    (6089, 10),
];

/// `BUFFED_ARMOR_LEVEL`: armor level added by each Impenetrability cantrip.
pub const ARMOR_LEVEL_SPELLS: [(u32, i32); 5] =
    [(2604, 20), (2592, 40), (4667, 60), (2349, 170), (2948, 220)];

/// `BUFFED_ELEMENTAL_DAMAGE`: elemental damage versus monsters added.
pub const ELEMENTAL_DAMAGE_SPELLS: [(u32, f64); 6] = [
    (3251, 0.01),
    (6035, 0.01),
    (2352, 0.02),
    (3250, 0.03),
    (4670, 0.05),
    (6098, 0.07),
];

/// `BUFFED_MELEE_DEFENSE`: melee defense bonus added.
pub const MELEE_DEFENSE_SPELLS: [(u32, f64); 6] = [
    (2600, 0.03),
    (3985, 0.04),
    (2588, 0.05),
    (4663, 0.07),
    (2488, 0.08),
    (6091, 0.09),
];

/// `BUFFED_ATTACK_BONUS`: attack bonus added.
pub const ATTACK_BONUS_SPELLS: [(u32, f64); 9] = [
    (2438, 0.01),
    (2439, 0.03),
    (2603, 0.03),
    (3984, 0.04),
    (2437, 0.05),
    (2591, 0.05),
    (2630, 0.06),
    (4666, 0.07),
    (6094, 0.09),
];

/// `BUFFED_MANA_CONVERSION`: what mana conversion is multiplied by.
pub const MANA_CONVERSION_SPELLS: [(u32, f64); 6] = [
    (3201, 1.05),
    (3199, 1.1),
    (3202, 1.15),
    (3200, 1.2),
    (6086, 1.25),
    (6087, 1.3),
];

fn lookup<T: Copy>(table: &[(u32, T)], spell: u32) -> Option<T> {
    table
        .iter()
        .find(|(id, _)| *id == spell)
        .map(|(_, value)| *value)
}

/// Each spell's whole-number bonus added to `base`, in spell order (C#
/// integer addition wraps).
fn add_int(item: &impl ItemView, base: i32, table: &[(u32, i32)]) -> i32 {
    item.spells()
        .iter()
        .filter_map(|spell| lookup(table, *spell))
        .fold(base, i32::wrapping_add)
}

fn add_double(item: &impl ItemView, base: f64, table: &[(u32, f64)]) -> f64 {
    item.spells()
        .iter()
        .filter_map(|spell| lookup(table, *spell))
        .fold(base, |value, bonus| value + bonus)
}

/// C#'s `(int)` of a double as the x86 .NET Framework does it: toward zero;
/// NaN or out of range is `int.MinValue`.
#[must_use]
pub fn dotnet_int(value: f64) -> i32 {
    if value > -2_147_483_649.0 && value < 2_147_483_648.0 {
        value as i32
    } else {
        i32::MIN
    }
}

/// The value of a computed key (`directValueOf`).
#[must_use]
pub fn value(computed: Computed, item: &impl ItemView) -> Value {
    match computed {
        Computed::ObjectClass => Value::Class(item.object_class()),
        Computed::TotalRatings => Value::Int(
            [
                key::DAM_RATING,
                key::DAM_RESIST_RATING,
                key::CRIT_RATING,
                key::CRIT_RESIST,
                key::CRIT_DAM_RATING,
                key::CRIT_DAM_RESIST_RATING,
                key::HEAL_BOOST_RATING,
                key::VITALITY_RATING,
            ]
            .iter()
            .fold(0i32, |sum, k| sum.wrapping_add(item.long(*k))),
        ),
        Computed::BuffedWeaponDamage => Value::Int(add_int(
            item,
            item.long(key::MAX_DAMAGE),
            &WEAPON_DAMAGE_SPELLS,
        )),
        Computed::BuffedArmorLevel => Value::Int(add_int(
            item,
            item.long(key::ARMOR_LEVEL),
            &ARMOR_LEVEL_SPELLS,
        )),
        Computed::BuffedElementalDamage => Value::Double(add_double(
            item,
            item.double(key::ELEMENTAL_DAMAGE_VERSUS_MONSTERS),
            &ELEMENTAL_DAMAGE_SPELLS,
        )),
        Computed::BuffedMeleeDefense => Value::Double(add_double(
            item,
            item.double(key::MELEE_DEFENSE_BONUS),
            &MELEE_DEFENSE_SPELLS,
        )),
        Computed::BuffedAttackBonus => Value::Double(add_double(
            item,
            item.double(key::ATTACK_BONUS),
            &ATTACK_BONUS_SPELLS,
        )),
        Computed::BuffedManaConversion => Value::Double(
            item.spells()
                .iter()
                .filter_map(|spell| lookup(&MANA_CONVERSION_SPELLS, *spell))
                .fold(item.double(key::MANA_C_BONUS), |value, factor| {
                    value * factor
                }),
        ),
        Computed::TotalMissileDamage => Value::Int(total_missile_damage(item)),
        Computed::TotalSummonDamage => Value::Double(total_summon_damage(item)),
    }
}

/// `TOTAL_MISSILE_DAMAGE`: a third of the damage modifier's percentage, plus
/// the elemental damage bonus, plus the damage cantrips; 0 for an item with
/// no damage modifier.
fn total_missile_damage(item: &impl ItemView) -> i32 {
    let base_mod = item.double(key::DAMAGE_BONUS);
    if base_mod == 0.0 {
        return 0;
    }
    let elemental = item.long(key::ELEMENTAL_DMG_BONUS);
    let cantrip = add_int(item, 0, &WEAPON_DAMAGE_SPELLS);
    let base = dotnet_int((base_mod - 1.0) * 100.0 / 3.0);
    base.wrapping_add(elemental).wrapping_add(cantrip)
}

/// `TOTAL_SUMMON_DAMAGE`: mudsort's damage score for a summoning essence
/// from its damage, critical and critical damage ratings, as written.
fn total_summon_damage(item: &impl ItemView) -> f64 {
    let rating_dmg = item.long(key::DAM_RATING);
    let rating_crit = item.long(key::CRIT_RATING);
    let rating_crit_dam = item.long(key::CRIT_DAM_RATING);
    let d = f64::from(rating_dmg);
    let c = f64::from(rating_crit);
    let dc = f64::from(rating_dmg.wrapping_add(rating_crit_dam));
    (0.625 * (1.0 + d / 100.0) * (0.9 - c / 100.0) + 2.0 * (1.0 + dc / 100.0) * (0.1 + c / 100.0))
        / 0.01365
}

/// The name .NET's `ToString()` gives a Decal object class. OpenAC-rust's
/// classes carry Decal's numbers and names; Decal has no combat pet class
/// (its 43 is `NumObjectClasses`), and the client never puts one in a pack.
#[must_use]
pub fn class_name(class: ObjectClass) -> &'static str {
    match class {
        ObjectClass::Unknown => "Unknown",
        ObjectClass::MeleeWeapon => "MeleeWeapon",
        ObjectClass::Armor => "Armor",
        ObjectClass::Clothing => "Clothing",
        ObjectClass::Jewelry => "Jewelry",
        ObjectClass::Monster => "Monster",
        ObjectClass::Food => "Food",
        ObjectClass::Money => "Money",
        ObjectClass::Misc => "Misc",
        ObjectClass::MissileWeapon => "MissileWeapon",
        ObjectClass::Container => "Container",
        ObjectClass::Gem => "Gem",
        ObjectClass::SpellComponent => "SpellComponent",
        ObjectClass::Key => "Key",
        ObjectClass::Portal => "Portal",
        ObjectClass::TradeNote => "TradeNote",
        ObjectClass::ManaStone => "ManaStone",
        ObjectClass::Plant => "Plant",
        ObjectClass::BaseCooking => "BaseCooking",
        ObjectClass::BaseAlchemy => "BaseAlchemy",
        ObjectClass::BaseFletching => "BaseFletching",
        ObjectClass::CraftedCooking => "CraftedCooking",
        ObjectClass::CraftedAlchemy => "CraftedAlchemy",
        ObjectClass::CraftedFletching => "CraftedFletching",
        ObjectClass::Player => "Player",
        ObjectClass::Vendor => "Vendor",
        ObjectClass::Door => "Door",
        ObjectClass::Corpse => "Corpse",
        ObjectClass::Lifestone => "Lifestone",
        ObjectClass::HealingKit => "HealingKit",
        ObjectClass::Lockpick => "Lockpick",
        ObjectClass::WandStaffOrb => "WandStaffOrb",
        ObjectClass::Bundle => "Bundle",
        ObjectClass::Book => "Book",
        ObjectClass::Journal => "Journal",
        ObjectClass::Sign => "Sign",
        ObjectClass::Housing => "Housing",
        ObjectClass::Npc => "Npc",
        ObjectClass::Foci => "Foci",
        ObjectClass::Salvage => "Salvage",
        ObjectClass::Ust => "Ust",
        ObjectClass::Services => "Services",
        ObjectClass::Scroll => "Scroll",
        ObjectClass::CombatPet => "CombatPet",
    }
}
