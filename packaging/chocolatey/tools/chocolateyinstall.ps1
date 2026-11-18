$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

# Chocolatey takes an ARM64 host for 32-bit and x64 PowerShell there reports AMD64 (chocolatey/choco#3907), but
# Win32_Processor names the real architecture (12 is ARM64), as install.ps1's Get-TabsmithArch does.
$arch = if ((Get-CimInstance Win32_Processor | Select-Object -First 1).Architecture -eq 12) { 'arm64' } else { 'x64' }
$other = if ($arch -eq 'arm64') { 'x64' } else { 'arm64' }

# Chocolatey shims every exe in the package once this script is done: one left, tabsmith.exe, makes one command.
Move-Item -Force -LiteralPath (Join-Path $toolsDir "tabsmith-win32-$arch.exe") -Destination (Join-Path $toolsDir 'tabsmith.exe')
Remove-Item -Force -LiteralPath (Join-Path $toolsDir "tabsmith-win32-$other.exe")
Write-Host "Installed tabsmith for Windows $arch"

# Not run here: Chocolatey runs elevated, so setup would edit the administrator's profile instead of yours.
Write-Host 'Run: tabsmith setup (in your own terminal, not an elevated one)'
