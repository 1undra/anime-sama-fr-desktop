$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$src = Join-Path $root "src-tauri"
$config = Get-Content (Join-Path $src "tauri.conf.json") -Raw | ConvertFrom-Json
$version = $config.version

Push-Location $src
try {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        cargo tauri build 2>&1 | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "Build échoué (code $LASTEXITCODE)" }
    } finally {
        $ErrorActionPreference = $previous
    }
} finally {
    Pop-Location
}

$setup = Get-ChildItem (Join-Path $src "target\release\bundle\nsis") -Filter "*_${version}_x64-setup.exe" | Select-Object -First 1
if (-not $setup) {
    throw "Installateur introuvable pour la version $version"
}

$out = Join-Path $root "release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$installer = Join-Path $out "Anime-Sama.exe"
Copy-Item $setup.FullName $installer -Force

$hex = (Get-FileHash $installer -Algorithm SHA512).Hash
$bytes = New-Object byte[] 64
for ($i = 0; $i -lt 64; $i++) {
    $bytes[$i] = [Convert]::ToByte($hex.Substring($i * 2, 2), 16)
}
$hash = [Convert]::ToBase64String($bytes)
$date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
$yml = "version: $version`nfiles:`n  - url: Anime-Sama.exe`n    sha512: $hash`npath: Anime-Sama.exe`nsha512: $hash`nreleaseDate: '$date'`n"
$yamlPath = Join-Path $out "latest.yml"
Set-Content -Path $yamlPath -Value $yml -NoNewline -Encoding UTF8

$previous = $ErrorActionPreference
$ErrorActionPreference = "Continue"
gh release view $version --repo traxcs/anime-sama-fr-desktop *> $null
$exists = $LASTEXITCODE -eq 0
$ErrorActionPreference = $previous
if ($exists) {
    gh release delete $version --repo traxcs/anime-sama-fr-desktop --yes
}
gh release create $version $installer $yamlPath --repo traxcs/anime-sama-fr-desktop --title $version --generate-notes
