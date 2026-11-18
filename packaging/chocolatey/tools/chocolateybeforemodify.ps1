$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$exe = Join-Path $toolsDir 'tabsmith.exe'

# Every open terminal tab runs tabsmith.exe, and Windows lets a running exe be renamed but not replaced or deleted. So
# it steps aside and an idle copy takes its name: Chocolatey backs that copy up (a failed upgrade rolls back to a
# working package), unshims it and deletes it like any file it installed. The renamed file goes at once when nothing
# runs it, best effort, and so do those earlier runs left. One still running stays until a later upgrade, or after an
# uninstall until lib\tabsmith is deleted by hand. install.ps1's Move-TabsmithIntoPlace does the same swap.
Get-ChildItem -LiteralPath $toolsDir -Filter 'tabsmith.exe.*.old' -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $exe) {
    $old = "$exe.$([DateTime]::UtcNow.Ticks).old"
    Move-Item -Force -LiteralPath $exe -Destination $old
    Copy-Item -LiteralPath $old -Destination $exe
    Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $old
}
