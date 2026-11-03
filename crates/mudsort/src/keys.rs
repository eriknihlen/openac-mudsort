//! How the sort reads an item: Decal's `WorldObject.Values` over Decal's key
//! space, which is what mudsort's keys name.
//!
//! Decal kept four tables per object (integer, floating-point, string,
//! boolean), keyed by AC property id, with its own keys (from 0x0A000000 up)
//! for what the object's description carries outside the property tables.
//! [`ItemView`] is that reading; [`DecalItem`] holds the four tables, and
//! [`DecalItem::from_inventory`] fills them from the client's item record and
//! properties the way Decal filled them.
//!
//! The fill follows Mosswart Massacre's port (`MosswartMassacre`
//! `rust/crates/mosswartmassacre/src/inventory/builder.rs`, `create`), which
//! reproduces Decal's tables key for key and was checked against 3,058 real
//! Decal item dumps. A key Decal had that it does not fill is listed in
//! `DIVERGENCES.md` with what reads instead.

use std::collections::BTreeMap;

use openac_plugin::items::InventoryItem;
use openac_plugin::world::{ItemProperties, ObjectClass};

/// Decal's value keys the sort, the computed keys and the selection read.
pub mod key {
    // Integer table: AC property ids Decal fills from the object itself.
    /// `Burden`.
    pub const BURDEN: u32 = 5;
    /// `EquippedSlots`: where it is worn now, 0 when not.
    pub const EQUIPPED_SLOTS: u32 = 10;
    /// `Value`.
    pub const VALUE: u32 = 19;
    /// `ArmorLevel`.
    pub const ARMOR_LEVEL: u32 = 28;
    /// `UsesTotal`.
    pub const USES_TOTAL: u32 = 91;
    /// `UsesRemaining`.
    pub const USES_REMAINING: u32 = 92;
    /// `Material`.
    pub const MATERIAL: u32 = 131;
    /// `ElementalDmgBonus`.
    pub const ELEMENTAL_DMG_BONUS: u32 = 204;
    /// `DamRating`.
    pub const DAM_RATING: u32 = 370;
    /// `DamResistRating`.
    pub const DAM_RESIST_RATING: u32 = 371;
    /// `CritRating`.
    pub const CRIT_RATING: u32 = 372;
    /// `CritResist`.
    pub const CRIT_RESIST: u32 = 373;
    /// `CritDamRating`.
    pub const CRIT_DAM_RATING: u32 = 374;
    /// `CritDamResistRating`.
    pub const CRIT_DAM_RESIST_RATING: u32 = 375;
    /// `HealBoostRating`.
    pub const HEAL_BOOST_RATING: u32 = 376;
    /// `VitalityRating`.
    pub const VITALITY_RATING: u32 = 379;

    // Integer table: Decal's own keys.
    /// `Type`: the weenie class id.
    pub const TYPE: u32 = 218_103_808;
    /// `Icon`, without the icon table's `0x06000000`.
    pub const ICON: u32 = 218_103_809;
    /// `ItemSlots`: items a container holds.
    pub const ITEM_SLOTS: u32 = 218_103_812;
    /// `PackSlots`: packs a container holds.
    pub const PACK_SLOTS: u32 = 218_103_813;
    /// `StackCount`.
    pub const STACK_COUNT: u32 = 218_103_814;
    /// `StackMax`.
    pub const STACK_MAX: u32 = 218_103_815;
    /// `AssociatedSpell`: the spell it casts when used.
    pub const ASSOCIATED_SPELL: u32 = 218_103_816;
    /// `SlotLegacy`.
    pub const SLOT_LEGACY: u32 = 218_103_817;
    /// `Wielder`.
    pub const WIELDER: u32 = 218_103_818;
    /// `WieldingSlot`.
    pub const WIELDING_SLOT: u32 = 218_103_819;
    /// `Coverage`: the body parts a worn piece covers.
    pub const COVERAGE: u32 = 218_103_821;
    /// `EquipableSlots`: where it may be worn or wielded.
    pub const EQUIPABLE_SLOTS: u32 = 218_103_822;
    /// `EquipType`: its combat use.
    pub const EQUIP_TYPE: u32 = 218_103_823;
    /// `IconOutline`: the icon effect bits.
    pub const ICON_OUTLINE: u32 = 218_103_824;
    /// `MissileType`: the ammunition type.
    pub const MISSILE_TYPE: u32 = 218_103_825;
    /// `Model`: the physics setup.
    pub const MODEL: u32 = 218_103_830;
    /// `Category`: the item type bits.
    pub const CATEGORY: u32 = 218_103_834;
    /// `SpellCount`.
    pub const SPELL_COUNT: u32 = 218_103_838;
    /// `WeapSpeed`.
    pub const WEAP_SPEED: u32 = 218_103_839;
    /// `EquipSkill`.
    pub const EQUIP_SKILL: u32 = 218_103_840;
    /// `DamageType`.
    pub const DAMAGE_TYPE: u32 = 218_103_841;
    /// `MaxDamage`.
    pub const MAX_DAMAGE: u32 = 218_103_842;
    /// `IconOverlay`, without the icon table's `0x06000000`.
    pub const ICON_OVERLAY: u32 = 218_103_849;
    /// `IconUnderlay`, without the icon table's `0x06000000`.
    pub const ICON_UNDERLAY: u32 = 218_103_850;
    /// `Slot`: its place in its container, -1 for none.
    pub const SLOT: u32 = 231_735_296;

    // Floating-point table: AC property ids.
    /// `MeleeDefenseBonus`.
    pub const MELEE_DEFENSE_BONUS: u32 = 29;
    /// `ManaCBonus`: mana conversion.
    pub const MANA_C_BONUS: u32 = 144;
    /// `ElementalDamageVersusMonsters`.
    pub const ELEMENTAL_DAMAGE_VERSUS_MONSTERS: u32 = 152;

    // Floating-point table: Decal's own keys.
    /// `SlashProt`.
    pub const SLASH_PROT: u32 = 167_772_160;
    /// `PierceProt`.
    pub const PIERCE_PROT: u32 = 167_772_161;
    /// `BludgeonProt`.
    pub const BLUDGEON_PROT: u32 = 167_772_162;
    /// `AcidProt`.
    pub const ACID_PROT: u32 = 167_772_163;
    /// `LightningProt`.
    pub const LIGHTNING_PROT: u32 = 167_772_164;
    /// `FireProt`.
    pub const FIRE_PROT: u32 = 167_772_165;
    /// `ColdProt`.
    pub const COLD_PROT: u32 = 167_772_166;
    /// `ApproachDistance`: how close, metres, to use it.
    pub const APPROACH_DISTANCE: u32 = 167_772_168;
    /// `SalvageWorkmanship`.
    pub const SALVAGE_WORKMANSHIP: u32 = 167_772_169;
    /// `Variance`.
    pub const VARIANCE: u32 = 167_772_171;
    /// `AttackBonus`.
    pub const ATTACK_BONUS: u32 = 167_772_172;
    /// `Range`.
    pub const RANGE: u32 = 167_772_173;
    /// `DamageBonus`: the damage modifier.
    pub const DAMAGE_BONUS: u32 = 167_772_174;

    // String table.
    /// `Name`.
    pub const NAME: u32 = 1;
    /// `SecondaryName`: the plural name.
    pub const SECONDARY_NAME: u32 = 184_549_376;
}

/// An item as Decal's `WorldObject` answered mudsort. Absent values read as
/// Decal's do: 0, 0.0, an empty string, false.
pub trait ItemView {
    /// The object id.
    fn id(&self) -> u32;
    /// `ObjectClass`.
    fn object_class(&self) -> ObjectClass;
    /// `Container`: the pack holding it, or the creature wielding it.
    fn container(&self) -> u32;
    /// `Values(LongValueKey)`.
    fn long(&self, key: u32) -> i32;
    /// `Values(DoubleValueKey)`.
    fn double(&self, key: u32) -> f64;
    /// `Values(StringValueKey)`.
    fn string(&self, key: u32) -> &str;
    /// `Values(BoolValueKey)`.
    fn bool(&self, key: u32) -> bool;
    /// `Spell(0)` to `Spell(SpellCount - 1)`: the spells it carries, without
    /// its own running enchantments.
    fn spells(&self) -> &[u32];
}

impl<T: ItemView + ?Sized> ItemView for &T {
    fn id(&self) -> u32 {
        (**self).id()
    }
    fn object_class(&self) -> ObjectClass {
        (**self).object_class()
    }
    fn container(&self) -> u32 {
        (**self).container()
    }
    fn long(&self, key: u32) -> i32 {
        (**self).long(key)
    }
    fn double(&self, key: u32) -> f64 {
        (**self).double(key)
    }
    fn string(&self, key: u32) -> &str {
        (**self).string(key)
    }
    fn bool(&self, key: u32) -> bool {
        (**self).bool(key)
    }
    fn spells(&self) -> &[u32] {
        (**self).spells()
    }
}

/// An item's four Decal tables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecalItem {
    /// The object id.
    pub id: u32,
    /// Its object class.
    pub object_class: ObjectClass,
    /// The pack holding it, or the creature wielding it.
    pub container: u32,
    /// The integer table.
    pub longs: BTreeMap<u32, i32>,
    /// The floating-point table.
    pub doubles: BTreeMap<u32, f64>,
    /// The string table.
    pub strings: BTreeMap<u32, String>,
    /// The boolean table.
    pub bools: BTreeMap<u32, bool>,
    /// The spells it carries.
    pub spells: Vec<u32>,
}

impl ItemView for DecalItem {
    fn id(&self) -> u32 {
        self.id
    }
    fn object_class(&self) -> ObjectClass {
        self.object_class
    }
    fn container(&self) -> u32 {
        self.container
    }
    fn long(&self, key: u32) -> i32 {
        self.longs.get(&key).copied().unwrap_or(0)
    }
    fn double(&self, key: u32) -> f64 {
        self.doubles.get(&key).copied().unwrap_or(0.0)
    }
    fn string(&self, key: u32) -> &str {
        self.strings.get(&key).map_or("", String::as_str)
    }
    fn bool(&self, key: u32) -> bool {
        self.bools.get(&key).copied().unwrap_or(false)
    }
    fn spells(&self) -> &[u32] {
        &self.spells
    }
}

/// The bits of the description's first header word saying which optional
/// fields the server sent (Mosswart Massacre's `header_flags`).
mod header {
    pub const PLURAL_NAME: u32 = 0x0000_0001;
    pub const ITEMS_CAPACITY: u32 = 0x0000_0002;
    pub const CONTAINERS_CAPACITY: u32 = 0x0000_0004;
    pub const VALUE: u32 = 0x0000_0008;
    pub const USE_RADIUS: u32 = 0x0000_0020;
    pub const UI_EFFECTS: u32 = 0x0000_0080;
    pub const AMMO_TYPE: u32 = 0x0000_0100;
    pub const COMBAT_USE: u32 = 0x0000_0200;
    pub const STRUCTURE: u32 = 0x0000_0400;
    pub const MAX_STRUCTURE: u32 = 0x0000_0800;
    pub const STACK_SIZE: u32 = 0x0000_1000;
    pub const MAX_STACK_SIZE: u32 = 0x0000_2000;
    pub const VALID_LOCATIONS: u32 = 0x0001_0000;
    pub const CURRENTLY_WIELDED_LOCATION: u32 = 0x0002_0000;
    pub const PRIORITY: u32 = 0x0004_0000;
    pub const BURDEN: u32 = 0x0020_0000;
    pub const SPELL: u32 = 0x0040_0000;
    pub const WORKMANSHIP: u32 = 0x0100_0000;
    pub const ICON_OVERLAY: u32 = 0x4000_0000;
    pub const MATERIAL_TYPE: u32 = 0x8000_0000;
    /// The public-flags bit saying a second header word follows.
    pub const INCLUDES_SECOND_HEADER: u32 = 0x0400_0000;
}

/// An appraised spell with this bit is one of the item's own running
/// enchantments, not a spell it carries.
const ACTIVE_SPELL_BIT: u32 = 0x8000_0000;

const ICON_BASE: u32 = 0x0600_0000;

/// An icon id as Decal reported it: without the icon table's prefix.
#[must_use]
pub fn short_icon(icon_id: u32) -> i32 {
    (if icon_id >= ICON_BASE {
        icon_id - ICON_BASE
    } else {
        icon_id
    }) as i32
}

impl DecalItem {
    /// The tables Decal held for an item the client reports, with its
    /// properties when the client has them (Mosswart Massacre's
    /// `builder::create`, the keys mudsort reads).
    #[must_use]
    pub fn from_inventory(item: &InventoryItem, properties: Option<&ItemProperties>) -> Self {
        use header as hf;
        let mut r = Self {
            id: item.object_id,
            object_class: item.object_class,
            container: if item.container_id != 0 {
                item.container_id
            } else {
                item.wielder_id
            },
            ..Self::default()
        };
        if let Some(p) = properties {
            r.longs.extend(p.ints.iter().copied());
            r.doubles.extend(p.floats.iter().copied());
            r.bools.extend(p.bools.iter().copied());
            r.strings.extend(p.strings.iter().cloned());
        }

        // A header field counts as sent when the description's first word
        // says so; before a description arrived, when it is not zero.
        let flags = item.header.as_ref().and_then(|h| h.weenie_header_flags);
        let sent = |flag: u32, when_unknown: bool| flags.map_or(when_unknown, |f| f & flag != 0);
        let longs = &mut r.longs;
        let doubles = &mut r.doubles;

        r.strings.insert(key::NAME, item.name.clone());
        longs.insert(key::TYPE, item.weenie_class_id as i32);
        longs.insert(key::ICON, short_icon(item.icon_id));
        longs.insert(key::CATEGORY, item.item_type as i32);
        if sent(hf::PLURAL_NAME, !item.plural_name.is_empty()) {
            r.strings
                .insert(key::SECONDARY_NAME, item.plural_name.clone());
        }
        if sent(
            hf::ITEMS_CAPACITY | hf::CONTAINERS_CAPACITY,
            item.items_capacity != 0 || item.containers_capacity != 0,
        ) {
            longs.insert(key::ITEM_SLOTS, item.items_capacity);
            longs.insert(key::PACK_SLOTS, item.containers_capacity);
        }
        if sent(hf::VALUE, item.value != 0) {
            longs.insert(key::VALUE, item.value);
        } else {
            // Decal always held a value: -1 when the server sent none.
            longs.entry(key::VALUE).or_insert(-1);
        }
        if sent(hf::USE_RADIUS, item.use_radius != 0.0) {
            doubles.insert(key::APPROACH_DISTANCE, f64::from(item.use_radius));
        }
        if sent(hf::UI_EFFECTS, item.effects != 0) {
            longs.insert(key::ICON_OUTLINE, item.effects as i32);
        }
        if sent(hf::AMMO_TYPE, item.ammo_type != 0) {
            longs.insert(key::MISSILE_TYPE, item.ammo_type as i32);
        }
        if sent(hf::COMBAT_USE, item.combat_use != 0) {
            longs.insert(key::EQUIP_TYPE, item.combat_use);
        }
        if sent(
            hf::STRUCTURE | hf::MAX_STRUCTURE,
            item.maximum_structure != 0 || item.structure != 0,
        ) {
            longs.insert(key::USES_TOTAL, item.maximum_structure);
            longs.insert(key::USES_REMAINING, item.structure);
        }
        if sent(
            hf::STACK_SIZE | hf::MAX_STACK_SIZE,
            item.maximum_stack_size > 1,
        ) {
            longs.insert(key::STACK_COUNT, item.stack_size);
            longs.insert(key::STACK_MAX, item.maximum_stack_size);
        }
        if sent(hf::VALID_LOCATIONS, item.valid_locations != 0) {
            longs.insert(key::EQUIPABLE_SLOTS, item.valid_locations as i32);
        }
        if sent(hf::CURRENTLY_WIELDED_LOCATION, item.equipped_location != 0) {
            longs.insert(key::EQUIPPED_SLOTS, item.equipped_location as i32);
        }
        if sent(hf::PRIORITY, item.coverage_mask != 0) {
            longs.insert(key::COVERAGE, item.coverage_mask as i32);
        }
        if sent(hf::BURDEN, item.burden != 0) {
            longs.insert(key::BURDEN, item.burden);
        }
        if sent(hf::SPELL, item.spell_id != 0) {
            longs.insert(key::ASSOCIATED_SPELL, item.spell_id as i32);
        }
        if sent(hf::WORKMANSHIP, item.workmanship != 0.0) {
            doubles.insert(key::SALVAGE_WORKMANSHIP, f64::from(item.workmanship));
        }
        if sent(hf::ICON_OVERLAY, item.icon_overlay_id != 0) {
            longs.insert(key::ICON_OVERLAY, short_icon(item.icon_overlay_id));
        }
        if sent(hf::MATERIAL_TYPE, item.material_type != 0) {
            longs.insert(key::MATERIAL, item.material_type as i32);
        }
        if item.public_flags & hf::INCLUDES_SECOND_HEADER != 0 {
            longs.insert(key::ICON_UNDERLAY, short_icon(item.icon_underlay_id));
        }
        if let Some(header) = &item.header {
            if let Some(setup) = header.setup_id {
                longs.insert(key::MODEL, setup as i32);
            }
            if let Some(parent) = header.parent_id {
                longs.insert(key::WIELDER, parent as i32);
            }
            if let Some(location) = header.parent_location {
                longs.insert(key::WIELDING_SLOT, location as i32);
            }
        }
        // Decal's slot is outside its tables; it is the place in the
        // container (Mosswart Massacre's port inventory, the key mapping).
        longs.insert(key::SLOT, item.container_slot);
        // A wielded item's old-style slot reads -1 (MossTank's VTank key
        // reader, `mosstank-loot` `requirements.rs`); nothing otherwise.
        if item.wielder_id != 0 {
            longs.insert(key::SLOT_LEGACY, -1);
        }

        for entry in &item.appraised_spell_ids {
            if entry & ACTIVE_SPELL_BIT == 0 {
                r.spells.push(*entry);
            }
        }
        if !item.appraised_spell_ids.is_empty() {
            longs.insert(key::SPELL_COUNT, r.spells.len() as i32);
        }

        if let Some(weapon) = properties.and_then(|p| p.weapon.as_ref()) {
            longs.insert(key::WEAP_SPEED, weapon.weapon_time);
            longs.insert(key::EQUIP_SKILL, weapon.weapon_skill as i32);
            longs.insert(key::DAMAGE_TYPE, weapon.damage_type);
            longs.insert(key::MAX_DAMAGE, weapon.damage);
            doubles.insert(key::VARIANCE, weapon.damage_variance);
            doubles.insert(key::ATTACK_BONUS, weapon.weapon_offense);
            doubles.insert(key::RANGE, weapon.max_velocity);
            doubles.insert(key::DAMAGE_BONUS, weapon.damage_mod);
        }
        if let Some(armor) = properties.and_then(|p| p.armor.as_ref()) {
            doubles.insert(key::SLASH_PROT, f64::from(armor.slash_mod));
            doubles.insert(key::PIERCE_PROT, f64::from(armor.pierce_mod));
            doubles.insert(key::BLUDGEON_PROT, f64::from(armor.bludgeon_mod));
            doubles.insert(key::ACID_PROT, f64::from(armor.acid_mod));
            doubles.insert(key::LIGHTNING_PROT, f64::from(armor.electric_mod));
            doubles.insert(key::FIRE_PROT, f64::from(armor.fire_mod));
            doubles.insert(key::COLD_PROT, f64::from(armor.cold_mod));
        }
        r
    }
}
