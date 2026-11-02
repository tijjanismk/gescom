<#
.SYNOPSIS
    Remplace Gescom sur ce poste : copie de secours des données, désinstallation
    des anciennes versions (les données restent), installation de la nouvelle.

.DESCRIPTION
    Dans l'ordre :
      1. une copie de secours des données de ce poste, avant de toucher à quoi
         que ce soit : la caisse (%APPDATA%\ml.gescom.app), Équipe
         (%APPDATA%\ml.gescom.equipe) et le serveur (%ProgramData%\Gescom, le
         service arrêté d'abord pour que la base soit copiée au repos) ;
      2. la désinstallation silencieuse des versions en place, lues dans
         Windows (Programmes et fonctionnalités). Une désinstallation
         silencieuse n'efface AUCUNE donnée : la case « supprimer les données »
         de l'ancienne caisse n'apparaît pas et reste décochée ; Gescom Serveur
         garde sa base, ses sauvegardes et son journal ;
      3. l'installation : Gescom Serveur AVEC ses fenêtres (sa page de
         configuration, et la reprise de la base d'un poste seul, D54), puis
         la caisse et Équipe sans fenêtres.
    Le serveur se fait en administrateur (une fenêtre Windows le demande) ;
    la caisse et Équipe s'installent pour l'utilisateur qui lance le script
    (installMode currentUser) : ne PAS lancer tout le script en administrateur
    sous un autre compte.

.PARAMETER Dossier
    Le dossier qui contient les trois installeurs (Gescom-Serveur_*,
    Gescom_*, Gescom Equipe_*). Défaut : le dossier de ce script.

.PARAMETER SansServeur
    Ne touche pas à Gescom Serveur (une caisse d'un autre poste).

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\mettre-a-jour-poste.ps1
#>

param(
    [string] $Dossier = $PSScriptRoot,
    [switch] $SansServeur,
    # Interne : la partie administrateur, relancée par le script lui-même.
    [string] $PartieAdmin = ""
)

$ErrorActionPreference = "Stop"

function Dire([string] $t, [string] $c = "Gray") { Write-Host $t -ForegroundColor $c }

# Les versions de Gescom inscrites dans Windows, machine et utilisateur.
function Installes {
    $cles = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
            'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    Get-ItemProperty $cles -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match '^Gescom' }
}

# Désinstalle en silence et attend la fin. Un désinstalleur NSIS se recopie
# dans %TEMP% et rend la main aussitôt ; `_?=` le fait travailler sur place.
function Desinstaller($p) {
    $chaine = if ($p.QuietUninstallString) { $p.QuietUninstallString } else { $p.UninstallString }
    if (-not $chaine) { Dire "  $($p.DisplayName) : pas de désinstalleur inscrit, laissé tel quel." Yellow; return }
    $exe = if ($chaine -match '^"([^"]+)"') { $Matches[1] } else { ($chaine -split ' ')[0] }
    if (-not (Test-Path $exe)) { Dire "  $($p.DisplayName) : désinstalleur introuvable ($exe), laissé tel quel." Yellow; return }
    $dossierProgramme = Split-Path -Parent $exe
    Dire "  $($p.DisplayName) $($p.DisplayVersion) : désinstallation silencieuse…"
    $proc = Start-Process -FilePath $exe -ArgumentList "/S", "_?=$dossierProgramme" -Wait -PassThru
    if ($proc.ExitCode -ne 0) { throw "La désinstallation de $($p.DisplayName) a rendu le code $($proc.ExitCode)." }
    # Avec `_?=`, le désinstalleur ne s'efface pas lui-même.
    Remove-Item -Force $exe -ErrorAction SilentlyContinue
    Remove-Item -Force -Recurse $dossierProgramme -ErrorAction SilentlyContinue
}

function Copier([string] $source, [string] $cible) {
    if (-not (Test-Path $source)) { Dire "  (rien à copier : $source)"; return }
    Copy-Item -Recurse -Force $source $cible
    $n = (Get-ChildItem -Recurse -File $cible | Measure-Object).Count
    Dire "  copié : $source  ->  $cible ($n fichiers)" Green
}

function Installeur([string] $motif) {
    $f = Get-ChildItem -Path $Dossier -Filter $motif -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $f) { throw "Installeur introuvable dans $Dossier : $motif" }
    $f.FullName
}

# ---------------------------------------------------------------------
#  La partie administrateur : le serveur (copie, désinstallation, installation).
# ---------------------------------------------------------------------
if ($PartieAdmin) {
    try {
        $secours = $PartieAdmin
        Dire "Gescom Serveur" Cyan
        $service = Get-Service GescomServeur -ErrorAction SilentlyContinue
        if ($service -and $service.Status -ne "Stopped") {
            Dire "  arrêt du service, pour copier la base au repos…"
            Stop-Service GescomServeur -Force
        }
        Copier (Join-Path $env:ProgramData "Gescom") (Join-Path $secours "serveur-ProgramData-Gescom")
        Installes | Where-Object { $_.DisplayName -eq "Gescom Serveur" } | ForEach-Object { Desinstaller $_ }
        $setup = Installeur "Gescom-Serveur_*_x64-setup.exe"
        Dire "  installation de $(Split-Path -Leaf $setup) : suivez ses fenêtres (port, reprise de la base)…"
        $proc = Start-Process -FilePath $setup -Wait -PassThru
        if ($proc.ExitCode -ne 0) { throw "L'installeur du serveur a rendu le code $($proc.ExitCode)." }
        Dire "  service : $((Get-Service GescomServeur -ErrorAction SilentlyContinue).Status)" Green
        exit 0
    } catch {
        Dire "ÉCHEC (serveur) : $($_.Exception.Message)" Red
        Read-Host "Entrée pour fermer"
        exit 1
    }
}

# ---------------------------------------------------------------------
#  La partie utilisateur.
# ---------------------------------------------------------------------
$Dossier = (Resolve-Path $Dossier).Path
Dire "Mise à jour de Gescom sur ce poste" Cyan
Dire "Installeurs : $Dossier"
$avant = Installes
if ($avant) { $avant | ForEach-Object { Dire "  en place : $($_.DisplayName) $($_.DisplayVersion)" } } else { Dire "  rien d'installé." }
# Les installeurs d'abord : on ne désinstalle rien si l'un manque.
$setupCaisse = Installeur "Gescom_*_x64-setup.exe"
$setupEquipe = Installeur "Gescom Equipe_*_x64-setup.exe"
if (-not $SansServeur) { $null = Installeur "Gescom-Serveur_*_x64-setup.exe" }

# 1. La copie de secours.
$secours = Join-Path ([Environment]::GetFolderPath("MyDocuments")) ("Gescom-secours-avant-mise-a-jour-" + (Get-Date -Format "yyyy-MM-dd_HH-mm"))
New-Item -ItemType Directory -Force $secours | Out-Null
Dire "`n1. Copie de secours dans $secours" Cyan
# Rien d'ouvert : une fenêtre ou le serveur qu'une caisse seule a lancé
# tiendraient leurs fichiers (et la base, pendant la copie).
$ouverts = Get-Process -Name gescom, gescom-equipe, gescom-serveur -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($env:LOCALAPPDATA, [StringComparison]::OrdinalIgnoreCase) }
if ($ouverts) {
    throw "Fermez d'abord Gescom et Gescom Équipe ($(($ouverts.ProcessName | Sort-Object -Unique) -join ', ') tourne encore), puis relancez le script."
}
foreach ($app in "ml.gescom.app", "ml.gescom.equipe") {
    Copier (Join-Path $env:APPDATA $app) (Join-Path $secours $app)
}

# 2 et 3. Le serveur, en administrateur (sa propre fenêtre).
if (-not $SansServeur) {
    Dire "`n2. Gescom Serveur (une fenêtre Windows demande l'autorisation)" Cyan
    $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"", "-Dossier", "`"$Dossier`"", "-PartieAdmin", "`"$secours`"")
    $proc = Start-Process powershell -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    if ($proc.ExitCode -ne 0) { throw "La partie serveur a échoué (code $($proc.ExitCode)) : rien d'autre n'a été touché." }
}

# La caisse et Équipe : désinstallées puis réinstallées pour cet utilisateur.
Dire "`n3. La caisse et Gescom Équipe" Cyan
Installes | Where-Object { $_.DisplayName -in "Gescom", "Gescom Equipe" } | ForEach-Object { Desinstaller $_ }
foreach ($setup in $setupCaisse, $setupEquipe) {
    Dire "  installation de $(Split-Path -Leaf $setup)…"
    $proc = Start-Process -FilePath $setup -ArgumentList "/S" -Wait -PassThru
    if ($proc.ExitCode -ne 0) { throw "L'installeur $(Split-Path -Leaf $setup) a rendu le code $($proc.ExitCode)." }
}

Dire "`nTerminé. Installé maintenant :" Green
Installes | ForEach-Object { Dire "  $($_.DisplayName) $($_.DisplayVersion)" Green }
Dire "Copie de secours gardée : $secours (à effacer quand tout marche)."
