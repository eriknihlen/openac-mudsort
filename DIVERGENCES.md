# Deliberate differences from mudsort

MudSort reproduces mudsort (`github.com/mudzereli/mudsort` at `3927e6c`).
Every deliberate difference is a row here; a commit that adds one adds its
row, a commit that removes one deletes it. The golden files under
`crates/mudsort/tests/golden` come from mudsort's own code
(`tools/golden/run.ps1`), so anything not listed matches it.

## Differences

| # | What | mudsort | MudSort | Why |
|---|---|---|---|---|
| MS-1 | The default sort string | `OC,ES,CV,AL-,AB-,DG-,ED-,ML,MA,NM` (`Properties/Settings.settings`). `DG` and `MA` match no key in today's code table and are dropped; `ES` is EquippedSlots, where an item is worn now, 0 for anything in a pack, so it orders nothing. What it decodes to: `OC,ES,CV,AL-,AB-,ED-,ML,NM` (test `mudsorts_default_loses_two_codes_and_sorts_by_equipped_slots`). | `OC,EQ,CV,AL-,AB-,DB-,ED-,ML,MT,NM`: EquipableSlots (where it can be worn), DamageBonus (the damage modifier) and Material in place of the dead and empty codes. An old string still decodes as mudsort decoded it. | The default should sort. Owner decision pending (plan, open item 3). |
| MS-2 | Case in a sort string | The edit box matched codes exactly (`oc` named nothing); `/ms set flags` upper-cased the string first. | Case never matters. | One rule for every entry point. |
| MS-3 | Parts that name no key | Logged to mudsort's error file and skipped. | Skipped and reported back (`SortString::unknown`) so the window can show them; an empty part (`OC,,NM`) is skipped without a report. | The player sees why a key is missing. |
| MS-4 | Writing a recipe that names one key twice | `AL-,AL-,` (`sortFlagListToString` finds the first copy's index, so every copy gets a comma). | `AL-,AL-`. Reading is unchanged: both copies take the last part's direction, as in mudsort. | The trailing comma is a slip; it only appears for a repeated key. |
| MS-5 | The culture of the text order | The player's Windows culture. | The en-US order, always. Checked against .NET Framework for every printable ASCII character and the Latin-1 letters (accented vowels, `ç`, `ñ`, `ø`, `æ`). Any other character (`ß`, Latin-1 symbols, other scripts) sorts after the letters by code point, which .NET does not. | A plugin has no culture; AC names are ASCII. |
| MS-6 | Digits outside ASCII, and digit runs longer than 19 digits | `char.IsDigit` took other scripts' digits, which `Convert.ToInt64` refused; a run longer than a 64-bit number overflowed. Either stopped the sort with an error. | Only ASCII digits are digits; a long run compares by value. | No error path to reproduce; the result is the order the numbers mean. |

## Keys

Decal's tables are reproduced from Mosswart Massacre's port
(`MosswartMassacre` `rust/crates/mosswartmassacre/src/inventory/builder.rs`,
`create`), checked there against 3,058 real Decal item dumps. Keys it does
not cover:

| Code | Key | Reads | Source |
|---|---|---|---|
| (none) | `Slot` (231735296) | The item's place in its container, -1 for none. | Decal held the slot outside its tables; Mosswart Massacre's port inventory maps it to the container slot (`docs/research/2026-09-25-mosswartmassacre-port-inventory.md`, the key mapping). |
| SG | `SlotLegacy` (218103817) | -1 for a wielded item, nothing (0) otherwise. | MossTank's reader of VTank's item keys (`mosstank-loot` `requirements.rs`), which follows VTank's mirror of Decal's object cache; not confirmed from Decal itself. |
| HD | `Heading` (167772167) | Always 0.0 (Decal's default). | No source maps it. |
| IL | `Inscribable` (201326593) | Always false (Decal's default). | No source maps it. |

`SpellCount`, `Slot` and `Scale` get no code in mudsort (every code they
could take is taken first) and none here.

Object classes are OpenAC-rust's, which carry Decal's numbers and names
(`Decal.Adapter.Wrappers.ObjectClass`, checked by `class_names_are_decals`
and the golden values). Decal has no combat pet class (its 43 is
`NumObjectClasses`); OpenAC-rust's `CombatPet` is named `CombatPet`. The
client classifies only monsters as combat pets, so none is ever in a pack.

## Assumptions

- **.NET's sort.** mudsort sorts the distinct values with
  `ArrayList.Sort`, which is not stable, and keeps the values from earlier
  passes in the list. That only shows for values the string order calls
  equal but that differ ("item 1" and "item 01"). MudSort reproduces .NET
  Framework's sort for a program without a target framework (the legacy
  depth-limited quicksort), checked against .NET Framework 4.8 on 200 lists
  and the golden sort cases. The game client is a native program, so Decal's
  plugins are assumed to get the same; if Decal's runtime chose the newer
  introsort instead, only the order of such equal values would differ.
- **Decimals.** `(int)(value * 10000)` is taken as .NET Framework 4.8's x86
  JIT computes it (IEEE doubles; NaN and out of range give -2147483648),
  checked against it. An older x86 JIT using the x87 unit could round
  differently in the last bit.
- **Which order a sort starts from.** mudsort sorted the items in the order
  Decal's `GetByContainer` listed them, which decides the order of items with
  equal values. That order is not known; `select` keeps the order it is
  given.
