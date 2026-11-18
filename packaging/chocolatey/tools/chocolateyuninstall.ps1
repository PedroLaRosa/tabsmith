$ErrorActionPreference = 'Stop'

# Chocolatey unshims the exes it still finds in the package, after chocolateybeforemodify.ps1 has moved tabsmith.exe
# aside, so it leaves the tabsmith shim behind: remove it by name.
Uninstall-BinFile -Name 'tabsmith'
