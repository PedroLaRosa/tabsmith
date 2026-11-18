$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$exe = Join-Path $toolsDir 'tabsmith.exe'

# Every open terminal tab runs tabsmith.exe, and Windows lets a running exe be renamed but not replaced or deleted. It
# steps aside so the upgrade or uninstall can go on: Chocolatey only removes the files it installed, so it leaves the
# renamed copy alone. That copy, and those earlier runs renamed, are deleted at once when nothing runs them (best
# effort); one still running stays until a later upgrade. install.ps1's Move-TabsmithIntoPlace does the same.
Get-ChildItem -LiteralPath $toolsDir -Filter 'tabsmith.exe.*.old' -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $exe) {
    $old = "$exe.$([DateTime]::UtcNow.Ticks).old"
    Move-Item -Force -LiteralPath $exe -Destination $old
    Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $old
}
