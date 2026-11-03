# MudSort

Inventory sorting for the openAC-rust Asheron's Call client: a WebAssembly
port of mudzereli's Decal plugin [mudsort](https://github.com/mudzereli/mudsort).
Pick a source and a destination, give a recipe of item properties
(`OC,AL-,NM`: object class, then armor level largest first, then name), and
MudSort moves the items over in that order.

Status: the sort core (keys and codes, sort strings, mudsort's string order,
the sort, the computed keys, item selection) is ported and pinned to
mudsort's own behaviour. The run (identify, then move one item at a time
into a pack, the open chest or another player), the `/ms` commands and the
per-character settings and presets work; the window is next.

```powershell
scripts/check.ps1   # format, lint, test, build target/wasm32-unknown-unknown/release/mudsort.wasm
```

`tools/golden/run.ps1` regenerates the golden files the tests compare
against by running mudsort's own code on .NET Framework. Deliberate
differences from mudsort are in `DIVERGENCES.md`. MIT (`LICENSE`), except
`crates/mudsort/src/alphanum.rs`, the port of mudsort's LGPL
`AlphanumComparator.cs`, which is LGPL-2.1-or-later (`LICENSE-LGPL-2.1`);
see `NOTICE.md`.
