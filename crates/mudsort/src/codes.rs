//! The sort keys and their two-letter codes.
//!
//! mudsort does not store its codes: its `SortFlag` static constructor
//! derives them from the key names, in a fixed order, the first time the
//! plugin runs. Old sort strings only paste in unchanged if the same
//! algorithm runs over the same keys in the same order, so this module ports
//! it as written (`SortFlag.cs`, the static constructor) over mudsort's own
//! key lists (`Enums.cs`), and the tests pin the result to the table
//! mudsort's code produces.

use std::sync::OnceLock;

/// A key mudsort computes from several values rather than reading one
/// (`SortFlag.cs`, the ten `FLAG_CUSTOM` flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Computed {
    /// The object class (`OBJECT_CLASS`).
    ObjectClass,
    /// The sum of the eight gear ratings (`CALCED_TOTAL_RATINGS`).
    TotalRatings,
    /// Maximum damage plus the damage cantrips (`BUFFED_WEAPON_DAMAGE`).
    BuffedWeaponDamage,
    /// Elemental damage versus monsters plus the cantrips
    /// (`BUFFED_ELEMENTAL_DAMAGE`).
    BuffedElementalDamage,
    /// Melee defense bonus plus the cantrips (`BUFFED_MELEE_DEFENSE`).
    BuffedMeleeDefense,
    /// Attack bonus plus the cantrips (`BUFFED_ATTACK_BONUS`).
    BuffedAttackBonus,
    /// Armor level plus the cantrips (`BUFFED_ARMOR_LEVEL`).
    BuffedArmorLevel,
    /// Mana conversion bonus times the cantrips (`BUFFED_MANA_CONVERSION`).
    BuffedManaConversion,
    /// A missile weapon's damage modifier, elemental bonus and cantrips as
    /// one number (`TOTAL_MISSILE_DAMAGE`).
    TotalMissileDamage,
    /// A summoning essence's damage score from its ratings
    /// (`TOTAL_SUMMON_DAMAGE`).
    TotalSummonDamage,
}

/// What a key reads: a computed value, or one of Decal's value tables by
/// key number (the AC property id, or one of Decal's own keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A computed value.
    Computed(Computed),
    /// Decal's string table (`MSStringValueKey`).
    String(u32),
    /// Decal's integer table (`MSLongValueKey`).
    Long(u32),
    /// Decal's floating-point table (`MSDoubleValueKey`).
    Double(u32),
    /// Decal's boolean table (`MSBoolValueKey`).
    Bool(u32),
}

/// How a key is shown in the key picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Text.
    Text,
    /// A whole number.
    Number,
    /// A decimal number.
    Decimal,
    /// Yes or no.
    YesNo,
    /// Computed from several values.
    Computed,
}

impl Category {
    /// The category's label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Number => "Number",
            Self::Decimal => "Decimal",
            Self::YesNo => "Yes-No",
            Self::Computed => "Computed",
        }
    }
}

impl Key {
    /// The key's category.
    #[must_use]
    pub const fn category(self) -> Category {
        match self {
            Self::Computed(_) => Category::Computed,
            Self::String(_) => Category::Text,
            Self::Long(_) => Category::Number,
            Self::Double(_) => Category::Decimal,
            Self::Bool(_) => Category::YesNo,
        }
    }
}

/// One sort key with its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    code: [u8; 2],
    /// mudsort's name for it (the enum member's name).
    pub name: &'static str,
    /// What it reads.
    pub key: Key,
    /// Listed under "Commonly Used Keys" (`SortFlag.CommonFlags`).
    pub common: bool,
}

impl Flag {
    /// The two-letter code, upper case.
    #[must_use]
    pub fn code(&self) -> &str {
        // Codes are built from ASCII letters only.
        std::str::from_utf8(&self.code).unwrap_or("??")
    }

    /// The key's plain-English name for the key picker.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        display_name(self.name)
    }

    /// The key's category.
    #[must_use]
    pub fn category(&self) -> Category {
        self.key.category()
    }
}

/// The table: every flag with a code, and the keys that got none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The flags, in the order mudsort registered them.
    pub flags: Vec<Flag>,
    /// Keys whose every candidate code was taken: mudsort logs "duplicate
    /// code entry" and leaves them out.
    pub uncoded: Vec<(&'static str, Key)>,
}

impl Table {
    /// The flag with this code (exact, upper case).
    #[must_use]
    pub fn by_code(&self, code: &str) -> Option<&Flag> {
        self.flags.iter().find(|flag| flag.code() == code)
    }

    /// The flag named `name` (mudsort's name).
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&Flag> {
        self.flags.iter().find(|flag| flag.name == name)
    }
}

/// The table, built once.
#[must_use]
pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(build)
}

/// The flag with this code, if any.
#[must_use]
pub fn by_code(code: &str) -> Option<&'static Flag> {
    table().by_code(code)
}

/// The computed flags, registered before the enums, with their fixed codes
/// (`SortFlag.cs`, the ten static fields).
const COMPUTED: [(&str, &str, Computed); 10] = [
    ("ObjectClass", "OC", Computed::ObjectClass),
    ("CalcedTotalRatings", "TR", Computed::TotalRatings),
    ("BuffedWeaponDamage", "BW", Computed::BuffedWeaponDamage),
    (
        "BuffedElementalDamage",
        "BE",
        Computed::BuffedElementalDamage,
    ),
    ("BuffedMeleeDefense", "BD", Computed::BuffedMeleeDefense),
    ("BuffedAttackBonus", "BA", Computed::BuffedAttackBonus),
    ("BuffedArmorLevel", "BP", Computed::BuffedArmorLevel),
    ("BuffedManaConversion", "BM", Computed::BuffedManaConversion),
    ("TotalMissileDamage", "TM", Computed::TotalMissileDamage),
    ("TotalSummonDamage", "TS", Computed::TotalSummonDamage),
];

/// `MSStringValueKey` (`Enums.cs`), in declaration order.
pub const STRING_KEYS: [(&str, u32); 10] = [
    ("SecondaryName", 184_549_376),
    ("Name", 1),
    ("Inscription", 7),
    ("InscribedBy", 8),
    ("SimpleDescription", 0xF),
    ("FullDescription", 0x10),
    ("OnlyActivatedBy", 25),
    ("PortalDestination", 38),
    ("LastTinkeredBy", 39),
    ("ImbuedBy", 40),
];

/// `MSLongValueKey` (`Enums.cs`), in declaration order.
pub const LONG_KEYS: [(&str, u32); 64] = [
    ("Type", 218_103_808),
    ("Icon", 218_103_809),
    ("ItemSlots", 218_103_812),
    ("PackSlots", 218_103_813),
    ("StackCount", 218_103_814),
    ("StackMax", 218_103_815),
    ("AssociatedSpell", 218_103_816),
    ("SlotLegacy", 218_103_817),
    ("WieldingSlot", 218_103_819),
    ("Coverage", 218_103_821),
    ("EquipableSlots", 218_103_822),
    ("EquipType", 218_103_823),
    ("IconOutline", 218_103_824),
    ("MissileType", 218_103_825),
    ("Model", 218_103_830),
    ("Category", 218_103_834),
    ("SpellCount", 218_103_838),
    ("WeapSpeed", 218_103_839),
    ("EquipSkill", 218_103_840),
    ("DamageType", 218_103_841),
    ("MaxDamage", 218_103_842),
    ("IconOverlay", 218_103_849),
    ("IconUnderlay", 218_103_850),
    ("Burden", 5),
    ("EquippedSlots", 10),
    ("RareId", 17),
    ("Value", 19),
    ("ArmorLevel", 28),
    ("Bonded", 33),
    ("Unenchantable", 36),
    ("WandElemDmgType", 45),
    ("MinLevelRestrict", 86),
    ("MaxLevelRestrict", 87),
    ("HealKitSkillBonus", 90),
    ("UsesTotal", 91),
    ("UsesRemaining", 92),
    ("Workmanship", 105),
    ("Spellcraft", 106),
    ("CurrentMana", 107),
    ("MaximumMana", 108),
    ("LoreRequirement", 109),
    ("RankRequirement", 110),
    ("Attuned", 114),
    ("SkillLevelReq", 115),
    ("Material", 131),
    ("WieldReqType", 158),
    ("WieldReqAttribute", 159),
    ("WieldReqValue", 160),
    ("SlayerSpecies", 166),
    ("NumberTimesTinkered", 171),
    ("ActivationReqSkillId", 176),
    ("Imbued", 179),
    ("ElementalDmgBonus", 204),
    ("ArmorSet", 265),
    ("Slot", 231_735_296),
    ("DamRating", 370),
    ("DamResistRating", 371),
    ("CritRating", 372),
    ("CritResist", 373),
    ("CritDamRating", 374),
    ("CritDamResistRating", 375),
    ("HealBoostRating", 376),
    ("VitalityRating", 379),
    ("SummoningMastery", 362),
];

/// `MSBoolValueKey` (`Enums.cs`), in declaration order.
pub const BOOL_KEYS: [(&str, u32); 7] = [
    ("Inscribable", 201_326_593),
    ("HookVisibility", 24),
    ("UnlimitedUses", 0x3F),
    ("CanBeSold", 69),
    ("Retained", 91),
    ("Ivoryable", 99),
    ("Dyeable", 100),
];

/// `MSDoubleValueKey` (`Enums.cs`), in declaration order.
pub const DOUBLE_KEYS: [(&str, u32); 25] = [
    ("SlashProt", 167_772_160),
    ("PierceProt", 167_772_161),
    ("BludgeonProt", 167_772_162),
    ("AcidProt", 167_772_163),
    ("LightningProt", 167_772_164),
    ("FireProt", 167_772_165),
    ("ColdProt", 167_772_166),
    ("Heading", 167_772_167),
    ("ApproachDistance", 167_772_168),
    ("SalvageWorkmanship", 167_772_169),
    ("Scale", 167_772_170),
    ("Variance", 167_772_171),
    ("AttackBonus", 167_772_172),
    ("Range", 167_772_173),
    ("DamageBonus", 167_772_174),
    ("ManaRateOfChange", 5),
    ("MeleeDefenseBonus", 29),
    ("ManaTransferEfficiency", 87),
    ("HealingKitRestoreBonus", 100),
    ("ManaStoneChanceDestruct", 137),
    ("ManaCBonus", 144),
    ("BitingStrikeChance", 147),
    ("MissileDBonus", 149),
    ("MagicDBonus", 150),
    ("ElementalDamageVersusMonsters", 152),
];

/// `SortFlag.CommonFlags`, as written. "SkilLevelReq" matches no key (the
/// key is `SkillLevelReq`), so that key is not common in mudsort either.
pub const COMMON_FLAGS: [&str; 31] = [
    "ObjectClass",
    "EquipableSlots",
    "Coverage",
    "ArmorLevel",
    "AttackBonus",
    "DamageBonus",
    "ElementalDmgBonus",
    "MinLevelRestrict",
    "Material",
    "Name",
    "ActivationReqSkillId",
    "ArmorSet",
    "Attuned",
    "Bonded",
    "Burden",
    "CurrentMana",
    "DamageType",
    "Heritage",
    "Icon",
    "MaxDamage",
    "RareId",
    "SkilLevelReq",
    "SlayerSpecies",
    "StackCount",
    "Value",
    "WieldReqAttribute",
    "WieldReqType",
    "WieldReqValue",
    "Workmanship",
    "MeleeDefenseBonus",
    "Variance",
];

/// One enum's members as .NET's `Enum.GetValues` returns them: sorted by
/// value as an unsigned number.
fn get_values(members: &[(&'static str, u32)]) -> Vec<(&'static str, u32)> {
    let mut values = members.to_vec();
    values.sort_by_key(|(_, value)| *value);
    values
}

/// Every key in the order mudsort registers them: the String, Long, Double
/// and Bool enums, each by `Enum.GetValues` (`SortFlag.cs`, `enums.AddRange`).
#[must_use]
pub fn enum_keys() -> Vec<(&'static str, Key)> {
    let mut keys = Vec::new();
    keys.extend(
        get_values(&STRING_KEYS)
            .into_iter()
            .map(|(n, v)| (n, Key::String(v))),
    );
    keys.extend(
        get_values(&LONG_KEYS)
            .into_iter()
            .map(|(n, v)| (n, Key::Long(v))),
    );
    keys.extend(
        get_values(&DOUBLE_KEYS)
            .into_iter()
            .map(|(n, v)| (n, Key::Double(v))),
    );
    keys.extend(
        get_values(&BOOL_KEYS)
            .into_iter()
            .map(|(n, v)| (n, Key::Bool(v))),
    );
    keys
}

const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const CONSONANTS: &str = "bcdfghjklmnpqrstvwxyz";

/// The code mudsort tries for `name` given the codes already taken: the
/// first two capitals not already taken as a pair; failing that, the
/// consonants after the first capital; failing that, the first two letters.
/// `None` when that last one is taken too.
fn code_for(name: &str, taken: &[String]) -> Option<String> {
    let mut code = String::new();
    for c in name.chars() {
        if UPPER.contains(c) {
            code.push(c);
            if code.len() == 2 {
                if taken.contains(&code) {
                    code.truncate(1);
                } else {
                    break;
                }
            }
        }
    }
    if code.len() < 2 {
        for c in name.chars() {
            if CONSONANTS.contains(c) {
                code.push(c);
            }
            if code.len() == 2 {
                if taken.contains(&code.to_uppercase()) {
                    code.truncate(1);
                } else {
                    break;
                }
            }
        }
        code = code.to_uppercase();
    }
    if code.len() < 2 {
        code = name.chars().take(2).collect::<String>().to_uppercase();
    }
    (!taken.contains(&code)).then_some(code)
}

fn flag(name: &'static str, code: &str, key: Key) -> Flag {
    let bytes = code.as_bytes();
    Flag {
        code: [bytes[0], bytes[1]],
        name,
        key,
        common: COMMON_FLAGS.contains(&name),
    }
}

fn build() -> Table {
    let mut taken: Vec<String> = COMPUTED.iter().map(|(_, c, _)| (*c).to_owned()).collect();
    let mut flags: Vec<Flag> = COMPUTED
        .iter()
        .map(|(name, code, computed)| flag(name, code, Key::Computed(*computed)))
        .collect();
    let mut uncoded = Vec::new();
    for (name, key) in enum_keys() {
        // mudsort also refuses a second flag of the same kind and name; the
        // enums have no repeated names, so the code is the only test.
        match code_for(name, &taken) {
            Some(code) => {
                flags.push(flag(name, &code, key));
                taken.push(code);
            }
            None => uncoded.push((name, key)),
        }
    }
    Table { flags, uncoded }
}

/// A key's plain-English name for the key picker.
#[must_use]
pub fn display_name(name: &str) -> &'static str {
    match name {
        // Computed.
        "ObjectClass" => "Object class",
        "CalcedTotalRatings" => "Total ratings",
        "BuffedWeaponDamage" => "Buffed maximum damage",
        "BuffedElementalDamage" => "Buffed elemental damage vs. monsters",
        "BuffedMeleeDefense" => "Buffed melee defense",
        "BuffedAttackBonus" => "Buffed attack bonus",
        "BuffedArmorLevel" => "Buffed armor level",
        "BuffedManaConversion" => "Buffed mana conversion",
        "TotalMissileDamage" => "Total missile damage",
        "TotalSummonDamage" => "Summon damage score",
        // Text.
        "SecondaryName" => "Plural name",
        "Name" => "Name",
        "Inscription" => "Inscription",
        "InscribedBy" => "Inscribed by",
        "SimpleDescription" => "Short description",
        "FullDescription" => "Full description",
        "OnlyActivatedBy" => "Only usable by",
        "PortalDestination" => "Portal destination",
        "LastTinkeredBy" => "Last tinkered by",
        "ImbuedBy" => "Imbued by",
        // Whole numbers.
        "Type" => "Weenie class",
        "Icon" => "Icon",
        "ItemSlots" => "Item slots",
        "PackSlots" => "Pack slots",
        "StackCount" => "Stack size",
        "StackMax" => "Largest stack",
        "AssociatedSpell" => "Spell it casts",
        "SlotLegacy" => "Old slot",
        "WieldingSlot" => "Wielded at",
        "Coverage" => "Body coverage",
        "EquipableSlots" => "Where it can be worn",
        "EquipType" => "Combat use",
        "IconOutline" => "Icon effects",
        "MissileType" => "Ammunition type",
        "Model" => "Model",
        "Category" => "Item type",
        "SpellCount" => "Spell count",
        "WeapSpeed" => "Weapon speed",
        "EquipSkill" => "Weapon skill",
        "DamageType" => "Damage type",
        "MaxDamage" => "Maximum damage",
        "IconOverlay" => "Icon overlay",
        "IconUnderlay" => "Icon underlay",
        "Burden" => "Burden",
        "EquippedSlots" => "Where it is worn now",
        "RareId" => "Rare number",
        "Value" => "Value",
        "ArmorLevel" => "Armor level",
        "Bonded" => "Bonded",
        "Unenchantable" => "Unenchantable",
        "WandElemDmgType" => "Wand element",
        "MinLevelRestrict" => "Minimum level",
        "MaxLevelRestrict" => "Maximum level",
        "HealKitSkillBonus" => "Healing kit skill bonus",
        "UsesTotal" => "Total uses",
        "UsesRemaining" => "Uses left",
        "Workmanship" => "Workmanship",
        "Spellcraft" => "Spellcraft",
        "CurrentMana" => "Mana",
        "MaximumMana" => "Maximum mana",
        "LoreRequirement" => "Lore requirement",
        "RankRequirement" => "Rank requirement",
        "Attuned" => "Attuned",
        "SkillLevelReq" => "Skill level required",
        "Material" => "Material",
        "WieldReqType" => "Wield requirement kind",
        "WieldReqAttribute" => "Wield requirement skill",
        "WieldReqValue" => "Wield requirement level",
        "SlayerSpecies" => "Slayer species",
        "NumberTimesTinkered" => "Times tinkered",
        "ActivationReqSkillId" => "Activation skill",
        "Imbued" => "Imbue",
        "ElementalDmgBonus" => "Elemental damage bonus",
        "ArmorSet" => "Armor set",
        "Slot" => "Slot",
        "DamRating" => "Damage rating",
        "DamResistRating" => "Damage resistance rating",
        "CritRating" => "Critical rating",
        "CritResist" => "Critical resistance rating",
        "CritDamRating" => "Critical damage rating",
        "CritDamResistRating" => "Critical damage resistance rating",
        "HealBoostRating" => "Healing boost rating",
        "VitalityRating" => "Vitality rating",
        "SummoningMastery" => "Summoning mastery",
        // Decimals.
        "SlashProt" => "Slashing protection",
        "PierceProt" => "Piercing protection",
        "BludgeonProt" => "Bludgeoning protection",
        "AcidProt" => "Acid protection",
        "LightningProt" => "Lightning protection",
        "FireProt" => "Fire protection",
        "ColdProt" => "Cold protection",
        "Heading" => "Heading",
        "ApproachDistance" => "Use distance",
        "SalvageWorkmanship" => "Salvage workmanship",
        "Scale" => "Scale",
        "Variance" => "Damage variance",
        "AttackBonus" => "Attack bonus",
        "Range" => "Range",
        "DamageBonus" => "Damage modifier",
        "ManaRateOfChange" => "Mana drain rate",
        "MeleeDefenseBonus" => "Melee defense bonus",
        "ManaTransferEfficiency" => "Mana transfer efficiency",
        "HealingKitRestoreBonus" => "Healing kit restore bonus",
        "ManaStoneChanceDestruct" => "Mana stone destroy chance",
        "ManaCBonus" => "Mana conversion bonus",
        "BitingStrikeChance" => "Biting strike chance",
        "MissileDBonus" => "Missile defense bonus",
        "MagicDBonus" => "Magic defense bonus",
        "ElementalDamageVersusMonsters" => "Elemental damage vs. monsters",
        // Yes or no.
        "Inscribable" => "Inscribable",
        "HookVisibility" => "Shows on a hook",
        "UnlimitedUses" => "Unlimited uses",
        "CanBeSold" => "Can be sold",
        "Retained" => "Retained",
        "Ivoryable" => "Ivoryable",
        "Dyeable" => "Dyeable",
        _ => "",
    }
}
