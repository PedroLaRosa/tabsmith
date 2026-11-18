# Packs tabsmith.<Version>.nupkg into the current directory from the release's two Windows binaries in -BinDir. The
# release workflow runs it; so can a maintainer pushing a version by hand (README "Releasing").
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinDir
)
$ErrorActionPreference = 'Stop'

# A copy of the package folder, so the binaries and filled-in files never land in the source tree.
$package = Join-Path ([IO.Path]::GetTempPath()) "tabsmith-chocolatey-$([Guid]::NewGuid())"
New-Item -ItemType Directory -Path $package | Out-Null
try {
    foreach ($item in 'tabsmith.nuspec', 'legal', 'tools') {
        Copy-Item -Recurse -LiteralPath (Join-Path $PSScriptRoot $item) -Destination $package
    }
    $root = Join-Path $PSScriptRoot '..\..'
    $licence = @("tabsmith is licensed under MIT OR Apache-2.0, at your option. From:",
        "https://github.com/PedroLaRosa/tabsmith/blob/main/LICENSE-MIT",
        "https://github.com/PedroLaRosa/tabsmith/blob/main/LICENSE-APACHE", '')
    foreach ($file in 'LICENSE-MIT', 'LICENSE-APACHE') {
        $licence += Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $root $file)
    }
    Set-Content -LiteralPath (Join-Path $package 'legal\LICENSE.txt') -Value ($licence -join "`n") -NoNewline -Encoding utf8

    $fills = @{ '@VERSION@' = $Version }
    foreach ($arch in 'x64', 'arm64') {
        $exe = Join-Path $BinDir "tabsmith-win32-$arch.exe"
        Copy-Item -LiteralPath $exe -Destination (Join-Path $package 'tools')
        $fills["@WIN32_$($arch.ToUpperInvariant())_SHA256@"] = (Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash.ToLowerInvariant()
    }
    foreach ($file in 'tabsmith.nuspec', 'legal\VERIFICATION.txt') {
        $path = Join-Path $package $file
        $text = Get-Content -Raw -Encoding UTF8 -LiteralPath $path
        foreach ($placeholder in $fills.Keys) {
            $text = $text.Replace($placeholder, $fills[$placeholder])
        }
        if ($text -match '@[A-Z0-9_]+@') {
            throw "$file has a placeholder pack.ps1 does not fill: $($Matches[0])"
        }
        Set-Content -LiteralPath $path -Value $text -NoNewline -Encoding utf8
    }

    choco pack (Join-Path $package 'tabsmith.nuspec') --outputdirectory (Get-Location).Path
    if ($LASTEXITCODE -ne 0) {
        throw "choco pack failed (exit code $LASTEXITCODE)"
    }
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue -LiteralPath $package
}
