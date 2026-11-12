# tabsmith installer for Windows (Windows PowerShell 5.1 and PowerShell 7): installs the release binary as
# %USERPROFILE%\.local\bin\tabsmith.exe, where it then updates itself in the background.
#
#   irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1))) 0.2.0
#
# `irm | iex` runs this in the user's own session: everything happens inside Install-Tabsmith, called on the last
# line, so nothing leaks into the session and a download cut short runs nothing. Failures are thrown, never `exit`:
# through iex, exit would close the user's PowerShell window.
param([string]$Version = 'latest')

function Install-Tabsmith([string]$Version) {
    $ErrorActionPreference = 'Stop'
    $releases = 'https://github.com/PedroLaRosa/tabsmith/releases'
    # Invoke-WebRequest's progress bar makes Windows PowerShell 5.1 downloads many times slower.
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 may still default to TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $arch = Get-TabsmithArch
    $asset = "tabsmith-win32-$arch.exe"
    $binDir = Join-Path $env:USERPROFILE '.local\bin'
    $exe = Join-Path $binDir 'tabsmith.exe'
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    # Next to the final file, so the last step is a rename on one volume.
    $staged = Join-Path $binDir ".tabsmith-install-$PID.exe"
    $sums = "$staged.sums"
    $versionFile = "$staged.version"
    try {
        if ($Version -eq 'latest') {
            Save-TabsmithFile "$releases/latest/download/VERSION" $versionFile
            $Version = (Get-Content -Raw -LiteralPath $versionFile).Trim()
        }
        $Version = $Version.TrimStart('v')

        Write-Host "Downloading tabsmith $Version for win32-$arch..."
        Save-TabsmithFile "$releases/download/v$Version/$asset" $staged
        Save-TabsmithFile "$releases/download/v$Version/SHA256SUMS" $sums
        $expected = Get-TabsmithListedDigest $sums $asset
        if (-not $expected) {
            throw "the SHA256SUMS of tabsmith $Version does not list $asset"
        }
        $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $staged).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            throw "$asset does not match its checksum (expected $expected, got $actual)"
        }

        # Every new terminal runs this file: one that does not start never takes the old one's place.
        $installedVersion = (& $staged --version | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or -not $installedVersion) {
            throw 'the downloaded tabsmith does not run on this machine'
        }
        $firstInstall = -not (Test-Path -LiteralPath $exe)
        Move-TabsmithIntoPlace $staged $exe
        Write-Host "Installed $installedVersion at $exe"
        Add-TabsmithToPath $binDir

        if ($firstInstall) {
            Write-Host ''
            if ([Console]::IsInputRedirected) {
                Write-Host 'Run: tabsmith setup'
            } else {
                & $exe setup
            }
        }
    } finally {
        Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $staged, $sums, $versionFile
    }
}

# x64 or arm64. Any sign of ARM64 wins: x64 PowerShell emulated on ARM64 reports AMD64 in its environment, and an
# older .NET reports the process's architecture, but Win32_Processor names the real one (12 is ARM64).
function Get-TabsmithArch {
    $names = @($env:PROCESSOR_ARCHITECTURE, $env:PROCESSOR_ARCHITEW6432)
    try {
        $names += [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    } catch {
    }
    try {
        $processor = Get-CimInstance -ClassName Win32_Processor | Select-Object -First 1
        if ($processor.Architecture -eq 12) {
            $names += 'ARM64'
        }
    } catch {
    }
    if ($names -contains 'ARM64') {
        return 'arm64'
    }
    if ($names -contains 'AMD64' -or $names -contains 'X64') {
        return 'x64'
    }
    throw "this architecture ($env:PROCESSOR_ARCHITECTURE) has no tabsmith release"
}

function Save-TabsmithFile([string]$Url, [string]$Path) {
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Path
    } catch {
        throw "could not download ${Url}: $($_.Exception.Message)"
    }
}

# The digest SHA256SUMS lists for the asset, in text (`<hex>  <name>`) or binary (`<hex> *<name>`) mode.
function Get-TabsmithListedDigest([string]$SumsFile, [string]$Asset) {
    foreach ($line in Get-Content -LiteralPath $SumsFile) {
        $fields = $line.Trim() -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq $Asset) {
            return $fields[0].ToLowerInvariant()
        }
    }
    return $null
}

# A running tabsmith.exe can be renamed but not replaced or deleted: it steps aside as
# tabsmith.exe.<pid>.<ticks>.old, the new file takes its name, and it comes back if that fails. Copies left by earlier
# updates are deleted first, best effort (one still running stays until a later run).
function Move-TabsmithIntoPlace([string]$Staged, [string]$Exe) {
    Get-ChildItem -LiteralPath (Split-Path -Parent $Exe) -Filter 'tabsmith.exe.*.old' -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
    if (-not (Test-Path -LiteralPath $Exe)) {
        Move-Item -Force -LiteralPath $Staged -Destination $Exe
        return
    }
    # Unique per call: `irm | iex` runs in the user's session, so a second run in the same window has the same $PID
    # while the first run's copy may still be running (that window's own tabsmith session).
    $old = "$Exe.$PID.$([DateTime]::UtcNow.Ticks).old"
    Move-Item -Force -LiteralPath $Exe -Destination $old
    try {
        Move-Item -Force -LiteralPath $Staged -Destination $Exe
    } catch {
        Move-Item -Force -LiteralPath $old -Destination $Exe
        throw
    }
    Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $old
}

# The user PATH (HKCU), for new terminals, and this session's PATH, so tabsmith runs by name right away.
# ponytail: [Environment] reads a REG_EXPAND_SZ user PATH expanded and writes it back as REG_SZ, which still works;
# keep the value kind through the registry API if anyone relies on %VAR% entries there.
function Add-TabsmithToPath([string]$BinDir) {
    $entries = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ })
    if ($entries -notcontains $BinDir) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $BinDir) -join ';'), 'User')
        Write-Host "Added $BinDir to your PATH"
    }
    if (@($env:Path -split ';') -notcontains $BinDir) {
        $env:Path = "$env:Path;$BinDir"
    }
}

Install-Tabsmith -Version $Version
