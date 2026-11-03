# Regenerates the golden files in crates/mudsort/tests/golden from mudsort's
# own code: compiles SortFlag.cs, Enums.cs and AlphanumComparator.cs from a
# mudsort checkout, unchanged, with Harness.cs (stand-ins for Decal's types,
# and PluginCore's sort loop and sort-string parser), for x86 on .NET
# Framework, and runs it under en-US as a Decal player did.
#
#   tools/golden/run.ps1 [-Mudsort <path to a mudsort checkout>]
#
# The checkout used for the committed files: github.com/mudzereli/mudsort at
# 3927e6c.
param(
    [string]$Mudsort = (Join-Path $PSScriptRoot '..\..\..\references\mudsort')
)
$ErrorActionPreference = 'Stop'
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319\csc.exe'
$golden = Join-Path $PSScriptRoot '..\..\crates\mudsort\tests\golden'
$out = Join-Path ([System.IO.Path]::GetTempPath()) 'mudsort-golden'
New-Item -ItemType Directory -Force $out | Out-Null
$exe = Join-Path $out 'harness.exe'
$sources = @(
    (Join-Path $PSScriptRoot 'Harness.cs'),
    (Join-Path $Mudsort 'SortFlag.cs'),
    (Join-Path $Mudsort 'Enums.cs'),
    (Join-Path $Mudsort 'AlphanumComparator.cs')
)
& $csc /nologo /platform:x86 /target:exe "/out:$exe" @sources
if ($LASTEXITCODE -ne 0) { throw 'csc failed' }
& $exe (Resolve-Path $golden) $PSScriptRoot
if ($LASTEXITCODE -ne 0) { throw 'harness failed' }
Write-Output "golden files written to $golden"
