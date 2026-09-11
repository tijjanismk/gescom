<#
.SYNOPSIS
    Compile gescom-serveur et le place la ou l'empaqueteur l'attend.

.DESCRIPTION
    Le serveur est un SECOND executable. L'installeur ne le prenait pas :
    un commercant qui installait Gescom obtenait la fenetre, et rien
    pour faire tourner plusieurs caisses. Le serveur se copiait a la
    main, sans raccourci, sans desinstallation, sans rien.

    Tauri embarque un binaire supplementaire par `externalBin`. Il exige
    que le fichier porte le TRIPLET de la cible en suffixe :

        gescom-serveur-x86_64-pc-windows-msvc.exe

    Le suffixe disparait a l'installation : le commercant trouve
    « gescom-serveur.exe » a cote de « Gescom.exe ».

    Ce script fait les deux : la compilation en release, et la copie
    sous le bon nom. Il tourne avant `tauri build`.

.PARAMETER Triplet
    Defaut : celui de la machine, lu dans `rustc -vV`.

.EXAMPLE
    .\outils\preparer_serveur.ps1
    npm run tauri build
#>

param(
    [string] $Triplet
)

# PAS "Stop" : en PowerShell 5.1, la sortie d'erreur d'un executable
# natif arrive enveloppee dans un ErrorRecord (NativeCommandError). Avec
# "Stop", la premiere ligne que cargo ecrit sur stderr — « Compiling
# tauri » en fait partie — interrompt le script alors que rien n'a
# echoue. On verifie donc $LASTEXITCODE, qui dit la verite.
$ErrorActionPreference = "Continue"

$racine = Split-Path -Parent $PSScriptRoot
$srcTauri = Join-Path $racine "src-tauri"
$destination = Join-Path $srcTauri "binaires"

if (-not $Triplet) {
    $ligne = (& rustc -vV) | Where-Object { $_ -like "host:*" }
    if (-not $ligne) {
        Write-Host "Impossible de lire le triplet depuis rustc." -ForegroundColor Red
        exit 1
    }
    $Triplet = $ligne.Split(":")[1].Trim()
}
Write-Host "Cible : $Triplet"

# --- Compilation ---
# `cargo-tenace` et non `cargo` : Smart App Control bloque parfois un
# binaire fraichement lie, et l'erreur ne ressemble pas a ce qu'elle est
# (voir AI_CONTEXT/modules/environnement-windows.md).
Push-Location $srcTauri
try {
    # `-Arguments @(...)` et non des mots libres : PowerShell verrait
    # « -p » comme un nom de parametre a lier, et cargo recevrait
    # « build --release » seul — donc TOUT l'espace de travail, dont le
    # crate applicatif, qui exige justement le binaire qu'on est en
    # train de fabriquer.
    & (Join-Path $PSScriptRoot "cargo-tenace.ps1") `
        -Arguments @("build", "--release", "-p", "gescom-serveur")
    if ($LASTEXITCODE -ne 0) {
        Write-Host "La compilation du serveur a echoue." -ForegroundColor Red
        exit 1
    }
} finally {
    Pop-Location
}

$source = Join-Path $srcTauri "target\release\gescom-serveur.exe"
if (-not (Test-Path $source)) {
    Write-Host "Binaire introuvable : $source" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $destination)) {
    New-Item -ItemType Directory -Path $destination | Out-Null
}

$cible = Join-Path $destination "gescom-serveur-$Triplet.exe"
Copy-Item -Path $source -Destination $cible -Force

$taille = [math]::Round((Get-Item $cible).Length / 1MB, 1)
Write-Host "Pret : $cible ($taille Mo)" -ForegroundColor Green
Write-Host ""
Write-Host "Construire l'installeur :" -ForegroundColor Cyan
Write-Host "  npm run tauri build"
