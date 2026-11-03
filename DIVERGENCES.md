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
| MS-7 | Taking an item out of the open chest | One `MoveItem(id, dest, slot, true)` from the chest straight to the destination slot. | Two requests: `pick_up(id, main pack)`, then, once that finished, `move_item` to the destination slot (or `give_item` to a player), always, the main pack too, since the slot matters. A stack the pick-up merges into one already in the main pack has no id left to move; that row fails. | The client has no slot-aware pick-up from an outside container. |
| MS-8 | Identifying | Every unidentified item was added to Decal's ID queue behind whatever was queued (the login identification), and the run waited until all had ID data, for ever if one never got it. | The sort's items go to the front of the queue, one request at a time (a second is answered `Busy` and asked again next tick, the client's one-in-flight rule). A request that ends without an appraisal is asked again up to 3 times (`identify::MAX_ATTEMPTS`); then the item is sorted without one (row note "could not be identified"); one the client refuses outright (`InvalidItem`, `Refused`) at once. | The player waits on the sort, not the login queue; a run must end. |
| MS-9 | Waiting between moves | The next `MoveItem` went out on the first frame Decal's `BusyState` was 0, without looking at how the last one ended. | The next request goes out once the last one's completion arrives (or the client stops being busy), as MossTank's `stack_cram.rs` does. A refusal (the client's `Refused`/`InvalidItem`/`InvalidTarget`, or the server's error in the completion) marks the row failed and the run goes on; a `Busy` answer is asked again next tick, at most 80 times per row (`run::MAX_BUSY_ANSWERS`, stack_cram's `AUTHENTIC_ABANDON_ATTEMPTS`). A put the client answers `Completed` (already in place) counts as moved. | Each row's state is shown, and a refused move must not hide behind the next. |
| MS-10 | Identifying at login | The loop queued each main-pack item and, for each pack, queued the pack itself once per item in it (`AddToQueue(obj.Id)` where `o.Id` was meant), so side-pack contents were never queued. | Everything the character owns is queued (main pack, packs and their contents, worn items), once each; one already appraised is skipped when its turn comes. | The option says "your inventory"; the loop's slip is plain. |
| MS-11 | An invalid source or destination | `activate` ran anyway (container 0: nothing to sort, or moves to 0). | `/ms start` refuses and says why ("cannot sort: ..."): the source must be the character, one of its packs or the open chest; the destination one of those or a player. | No moves to nowhere. |
| MS-12 | `/ms` error paths | `/ms set ocfilter` alone threw (`Substring` past the end) and did nothing; a source or destination that is not a number (`pack x`, `abc`) threw in `Int32.Parse`, was logged, and the line went on to the server; an unknown `/ms ...` went to the server too. | `/ms set ocfilter` alone clears the filter, as `/ms clear ocfilter`; a bad argument says "not a container: ..."; an unknown command points at `/ms help`; `/ms set flags` also lists the codes it skipped (MS-3). An id may also be written unsigned or as `0x` hexadecimal. | Error paths answer in chat; ids are shown unsigned in this client. |
| MS-13 | Where settings live | .NET user settings: one set per Windows user, shared by every character; three unnamed saved strings; the class filter and the destination slot were window fields, not saved. | JSON per character in the plugin's storage (`settings.json`), saved on every change: the sort string, the class filter, a name filter (new), the start slot, reverse, identify at login, announce when done, and named presets ("Everything" and mudsort's three saved strings as "Saved 1" to "Saved 3", kept as written: `MA` in the first decodes to nothing, as in mudsort). Source and destination are the character at every login, as mudsort set them. mudsort's `Debug` setting is not ported. | The client keeps plugin data per character. |
| MS-14 | Stopping a sort | `cancel` left the sort's items in Decal's ID queue. | The sort's items leave the identification queue, and the one in flight is not asked about again. | "Stop" stops asking the client for anything. |

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
- **Chat colour.** mudsort wrote its lines with Decal's `AddChatText(text,
  5)`; MudSort posts them with text class 5, taking Decal's colour number
  as the client's text class, as MossTank's port of UtilityBelt's chat does
  (`vendor_host.rs`).
