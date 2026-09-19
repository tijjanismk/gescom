<#
.SYNOPSIS
    Construit l'installeur de Gescom Serveur : compilation, signature,
    empaquetage NSIS, signature de l'installeur.

.DESCRIPTION
    Sortie : dist\Gescom-Serveur_<version>_x64-setup.exe, signe avec le
    certificat auto-signe de outils\signer.ps1. La version est celle de
    src-tauri\serveur\Cargo.toml.

    makensis vient de l'installation NSIS que Tauri a deja telechargee
    (%LOCALAPPDATA%\tauri\NSIS) ; a defaut, celle du PATH.

.PARAMETER SansSignature
    N'appelle pas signtool (pas de SDK Windows sur ce poste).

.EXAMPLE
    .\outils\construire_installeur_serveur.ps1
#>

param(
    [switch] $SansSignature
)

$ErrorActionPreference = "Continue"
$racine = Split-Path -Parent $PSScriptRoot
$srcTauri = Join-Path $racine "src-tauri"
$dist = Join-Path $racine "dist\serveur"

# --- Version ---
$cargo = Get-Content (Join-Path $srcTauri "serveur\Cargo.toml")
$version = ($cargo | Select-String '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
if (-not $version) { Write-Host "Version introuvable dans serveur\Cargo.toml" -ForegroundColor Red; exit 1 }
Write-Host "Gescom Serveur $version"

# --- Compilation en release (cargo-tenace : Smart App Control) ---
Push-Location $srcTauri
try {
    & (Join-Path $PSScriptRoot "cargo-tenace.ps1") -Arguments @("build", "--release", "-p", "gescom-serveur")
    if ($LASTEXITCODE -ne 0) { Write-Host "La compilation a echoue." -ForegroundColor Red; exit 1 }
} finally { Pop-Location }

New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item (Join-Path $srcTauri "target\release\gescom-serveur.exe") (Join-Path $dist "gescom-serveur.exe") -Force

# --- Certificat public + signature de l'executable ---
$signer = Join-Path $PSScriptRoot "signer.ps1"
& $signer -ExporterVers (Join-Path $dist "gescom.cer")
if ($LASTEXITCODE -ne 0) { exit 1 }
if (-not $SansSignature) {
    & $signer (Join-Path $dist "gescom-serveur.exe")
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

# --- makensis ---
$makensis = Join-Path $env:LOCALAPPDATA "tauri\NSIS\makensis.exe"
if (-not (Test-Path $makensis)) {
    $cmd = Get-Command makensis -ErrorAction SilentlyContinue
    if ($cmd) { $makensis = $cmd.Source } else {
        Write-Host "makensis introuvable : lancer une fois « npm run tauri build » (il installe NSIS), ou installer NSIS." -ForegroundColor Red
        exit 1
    }
}
$sortie = Join-Path $racine "dist\Gescom-Serveur_${version}_x64-setup.exe"
& $makensis "/DVERSION=$version" "/DSOURCE=$dist" "/DSORTIE=$sortie" (Join-Path $PSScriptRoot "installeur_serveur.nsi")
if ($LASTEXITCODE -ne 0) { Write-Host "makensis a echoue." -ForegroundColor Red; exit 1 }

if (-not $SansSignature) {
    & $signer $sortie
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

$taille = [math]::Round((Get-Item $sortie).Length / 1MB, 1)
Write-Host ""
Write-Host "Pret : $sortie ($taille Mo)" -ForegroundColor Green
Write-Host "A lancer EN ADMINISTRATEUR sur la machine qui tiendra la base."
