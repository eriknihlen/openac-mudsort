# MudSort

Inventory sorting for the openAC-rust Asheron's Call client: a WebAssembly
port of mudzereli's Decal plugin [mudsort](https://github.com/mudzereli/mudsort).
Pick a source and a destination, give a recipe of item properties
(`OC,AL-,NM`: object class, then armor level largest first, then name), and
MudSort moves the items over in that order.

Status: the sort core (keys and codes, sort strings, mudsort's string order,
the sort, the computed keys, item selection) is ported and pinned to
mudsort's own behaviour. The run, the commands and the window are next.

```powershell
scripts/check.ps1   # format, lint, test, build target/wasm32-unknown-unknown/release/mudsort.wasm
```

`tools/golden/run.ps1` regenerates the golden files the tests compare
against by running mudsort's own code on .NET Framework. Deliberate
differences from mudsort are in `DIVERGENCES.md`. MIT; see `LICENSE` and
`NOTICE.md`.
