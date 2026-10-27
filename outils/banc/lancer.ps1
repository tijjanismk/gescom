# Banc d'essai de l'ecran, sous Windows : les trois installations de la
# v3, chacune sur une base jetable, pour les parcours Playwright de ce
# dossier.
#
#   .\outils\banc\lancer.ps1 sqlite      [-Neuf]   serveur sur SQLite (comme lancer.sh)
#   .\outils\banc\lancer.ps1 multiposte  [-Neuf]   serveur sur PostgreSQL gescom_test
#   .\outils\banc\lancer.ps1 monoposte   [-Neuf]   la vraie fenetre Tauri + son serveur SQLite
#   .\outils\banc\lancer.ps1 arreter                arrete ce que le banc a lance
#
# Ce que chaque mode laisse tourner :
#   sqlite, multiposte : serveur :40000 + ecran (Vite) :1420, les parcours
#                        ouvrent Edge sur l'ecran, chaque navigateur est un poste ;
#   monoposte          : la boutique a une caisse telle qu'elle s'installe :
#                        gescom.exe seul, pilote par Playwright a travers
#                        WebView2 (:9222). Au premier demarrage il demande
#                        le role du poste ; « garde la base » : il lance
#                        lui-meme son serveur SQLite (127.0.0.1:40000) et
#                        s'y branche (D22/D24 revises le 25/10/2026).
#
# Garde-fous :
#   - multiposte REFUSE toute base qui n'est pas `gescom_test` : -Neuf y
#     fait DROP SCHEMA, comme les scenarios Rust. La base du serveur
#     (`gescom`) n'est jamais touchee.
#   - monoposte pose GESCOM_DONNEES (lu en build de debogage seulement)
#     et un dossier WebView2 a part : ni la base ni la session de la
#     fenetre du proprietaire (%APPDATA%\ml.gescom.app) ne sont ouvertes.
#   - monoposte ne travaille QUE dans un dossier marque « banc »
#     (.travail\monoposte\.proprietaire), -Neuf ou non, et refuse avant
#     d'arreter quoi que ce soit : le 27/10/2026, -Neuf a efface l'essai a
#     la main du proprietaire, qui vivait la, sauvegardes comprises.
#     L'essai a la main a son dossier et son script : essai-main.ps1.
#   - on arrete ce qui ecoute sur NOS ports, et le serveur qui tient la base
#     du banc (dossier marque), jamais un processus par son nom (le serveur
#     de l'essai a la main ecoute 40050, sur sa base : jamais touche).
#
# Tout vit dans outils/banc/.travail/ (ignore par git).

param(
    [Parameter(Position = 0)]
    [ValidateSet("sqlite", "multiposte", "monoposte", "arreter")]
    [string] $Mode = "sqlite",
    [switch] $Neuf
)

# Pas « Stop » : cargo et psql parlent sur stderr sans echouer, et
# PowerShell 5.1 en ferait une erreur. Les codes de sortie sont lus.
$ErrorActionPreference = "Continue"
$Ici = Split-Path -Parent $MyInvocation.MyCommand.Path
$Racine = (Resolve-Path (Join-Path $Ici "..\..")).Path
$Travail = Join-Path $Ici ".travail"
New-Item -ItemType Directory -Force (Join-Path $Travail "captures") | Out-Null

$PortServeur = if ($env:PORT_SERVEUR) { [int]$env:PORT_SERVEUR } else { 40000 }
# La fenetre de debogage charge devUrl (tauri.conf.json) : 1420, fixe.
$PortEcran = 1420
$PortCdp = if ($env:PORT_CDP) { [int]$env:PORT_CDP } else { 9222 }
$UrlPg = if ($env:GESCOM_PG_BANC) { $env:GESCOM_PG_BANC } else { "postgresql://postgres:user@127.0.0.1:5432/gescom_test" }
$Psql = "C:\Program Files\PostgreSQL\18\bin\psql.exe"

function Arreter-Port([int] $port) {
    $c = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue
    foreach ($p in ($c | Select-Object -ExpandProperty OwningProcess -Unique)) {
        if ($p -gt 0) { Stop-Process -Id $p -Force -ErrorAction SilentlyContinue }
    }
}

function Arreter-Tout {
    # La fenetre d'abord : elle tient le port CDP par son WebView2.
    $pids = Join-Path $Travail "pids.txt"
    if (Test-Path $pids) {
        foreach ($l in Get-Content $pids) {
            if ($l -match '^\d+$') { Stop-Process -Id ([int]$l) -Force -ErrorAction SilentlyContinue }
        }
        Remove-Item $pids
    }
    foreach ($p in $PortServeur, $PortEcran, $PortCdp) { Arreter-Port $p }
    # Le serveur que la fenetre du banc a lance sur un autre port de la
    # plage (p5 le laisse sur 40001) tient encore serveur.db, et -Neuf ne
    # pourrait pas l'effacer : on l'arrete par sa base — seulement si le
    # dossier porte la marque du banc.
    if ((Lire-Marque $DonneesMonoposte) -eq $MarqueBanc) {
        $base = (Join-Path $DonneesMonoposte "serveur.db").ToLowerInvariant()
        Get-CimInstance Win32_Process -Filter "Name = 'gescom-serveur.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.CommandLine -and $_.CommandLine.ToLowerInvariant().Contains($base) } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    Start-Sleep -Seconds 1
}

function Construire([string] $paquet) {
    Push-Location $Racine
    try {
        & (Join-Path $Racine "outils\cargo-tenace.ps1") build --package $paquet
        if ($LASTEXITCODE -ne 0) { throw "La construction de $paquet a echoue." }
    } finally { Pop-Location }
}

function Lancer([string] $exe, [string[]] $arguments, [string] $sortie, [string] $fenetre = "Hidden") {
    $o = @{
        FilePath = $exe; WorkingDirectory = $Racine; WindowStyle = $fenetre; PassThru = $true
        RedirectStandardOutput = "$sortie.out"; RedirectStandardError = "$sortie.err"
    }
    # Start-Process refuse une liste d'arguments vide.
    if ($arguments.Count -gt 0) { $o.ArgumentList = $arguments }
    $p = Start-Process @o
    Add-Content (Join-Path $Travail "pids.txt") $p.Id
    return $p
}

# Ou Vite ecoute : 127.0.0.1 pour les navigateurs des parcours (pw.mjs) ;
# localhost pour la fenetre, qui charge devUrl (http://localhost:1420) —
# Windows le resout en ::1, et un Vite sur 127.0.0.1 lui refusait la
# connexion (page blanche). Jamais 0.0.0.0 : l'ecran n'a rien a faire
# sur le reseau de la boutique.
$HoteEcran = if ($Mode -eq "monoposte") { "localhost" } else { "127.0.0.1" }

function Ecran {
    Lancer "node" @("node_modules\vite\bin\vite.js", "--config", "outils\banc\vite.banc.config.ts",
        "--port", "$PortEcran", "--strictPort", "--host", $HoteEcran) `
        (Join-Path $Travail "vite") | Out-Null
}

function Attendre([string] $url, [string] $quoi) {
    for ($i = 0; $i -lt 90; $i++) {
        try {
            Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2 | Out-Null
            return
        } catch { Start-Sleep -Seconds 1 }
    }
    Write-Output "PAS PRET : $quoi ($url)"
    Get-ChildItem $Travail -Filter "*.err" | ForEach-Object { Write-Output "--- $($_.Name)"; Get-Content $_.FullName -Tail 8 }
    exit 1
}

# Chaque dossier de .travail porte sa marque : .proprietaire = banc (ce
# script) ou essai-main (essai-main.ps1). Le banc ne travaille que dans le
# sien ; un dossier deja la sans sa marque est peut-etre un essai a la main.
$MarqueBanc = "banc"
$DonneesMonoposte = Join-Path $Travail "monoposte"

function Lire-Marque([string] $dossier) {
    $f = Join-Path $dossier ".proprietaire"
    if (-not (Test-Path -LiteralPath $f)) { return "" }
    $t = Get-Content -LiteralPath $f -Raw -ErrorAction SilentlyContinue
    if ($null -eq $t) { return "" }
    return ([string]$t).Trim()
}

function Poser-Marque([string] $dossier, [string] $marque) {
    Set-Content -LiteralPath (Join-Path $dossier ".proprietaire") -Value $marque -Encoding ascii
}

function Exiger-Le-Sien([string] $dossier, [string] $marque) {
    if (-not (Test-Path -LiteralPath $dossier)) { return }
    $vue = Lire-Marque $dossier
    if ($vue -eq $marque) { return }
    $dit = if ($vue) { $vue } else { "aucune" }
    Write-Output "REFUS : $dossier n'est pas le dossier du banc (.proprietaire attendu : $marque ; trouve : $dit)."
    Write-Output "Le banc n'y touche pas, -Neuf ou non, et n'a rien arrete : c'est peut-etre un essai a la main."
    Write-Output "  - L'essai a la main a son propre dossier : .\outils\banc\essai-main.ps1 (.travail\essai-main)."
    Write-Output "    A sa premiere ouverture, il COPIE les donnees de .travail\monoposte ; il ne les efface pas."
    Write-Output "  - Une fois la copie verifiee, ou si ce dossier n'a rien a garder : le deplacer ou l'effacer"
    Write-Output "    soi-meme, puis relancer. Le banc le recree, marque $marque."
    exit 2
}

# Avant d'arreter quoi que ce soit : un refus ne coupe rien.
if ($Mode -eq "monoposte") { Exiger-Le-Sien $DonneesMonoposte $MarqueBanc }

Arreter-Tout
if ($Mode -eq "arreter") { Write-Output "ARRETE"; exit 0 }

$env:GESCOM_DEMO = "1"

switch ($Mode) {
    "sqlite" {
        $base = Join-Path $Travail "essai.db"
        if ($Neuf) { Remove-Item "$base", "$base-wal", "$base-shm" -ErrorAction SilentlyContinue }
        Construire "gescom-serveur"
        Lancer (Join-Path $Racine "src-tauri\target\debug\gescom-serveur.exe") `
            @("--hote", "127.0.0.1", "--port", "$PortServeur", "--base", "`"$base`"", "--journal", "`"$(Join-Path $Travail 'essai.log')`"", "--sauvegardes", "`"$(Join-Path $Travail 'sauvegardes')`"") `
            (Join-Path $Travail "serveur") | Out-Null
        Ecran
        Attendre "http://127.0.0.1:$PortServeur/sante" "serveur"
        Set-Content -Encoding ascii (Join-Path $Travail "moteur.txt") $base
    }
    "multiposte" {
        # Le nom de la base, lu dans l'URL : rien d'autre que gescom_test.
        $nom = ($UrlPg -split "/")[-1].Split("?")[0]
        if ($nom -ne "gescom_test") {
            Write-Output "REFUS : le banc multiposte ne tourne que sur gescom_test (pas « $nom »)."
            exit 2
        }
        if ($Neuf) {
            & $Psql -q -d $UrlPg -c "SET client_min_messages TO warning; DROP SCHEMA public CASCADE; CREATE SCHEMA public;" 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Remise a zero de gescom_test impossible." }
            # Sur PostgreSQL, le serveur n'ecrit pas la demo (GESCOM_DEMO ne
            # joue que sur SQLite) : l'outil de mise en route la pose, avant.
            Push-Location $Racine
            & (Join-Path $Racine "outils\cargo-tenace.ps1") build --package gescom-noyau --example amorcer
            Pop-Location
            & (Join-Path $Racine "src-tauri\target\debug\examples\amorcer.exe") $UrlPg --demo
            if ($LASTEXITCODE -ne 0) { throw "Amorcage de la demo sur gescom_test impossible." }
        }
        Construire "gescom-serveur"
        Lancer (Join-Path $Racine "src-tauri\target\debug\gescom-serveur.exe") `
            @("--hote", "127.0.0.1", "--port", "$PortServeur", "--base", $UrlPg, "--journal", "`"$(Join-Path $Travail 'essai.log')`"", "--sauvegardes", "`"$(Join-Path $Travail 'sauvegardes')`"") `
            (Join-Path $Travail "serveur") | Out-Null
        Ecran
        Attendre "http://127.0.0.1:$PortServeur/sante" "serveur"
        Set-Content -Encoding ascii (Join-Path $Travail "moteur.txt") $UrlPg
    }
    "monoposte" {
        $donnees = $DonneesMonoposte
        # Le profil WebView2 vit DANS le dossier : une seule marque couvre
        # tout ce que -Neuf efface. (L'ancien .travail\monoposte-webview,
        # d'avant le 27/10/2026, n'est plus touche par personne.)
        $webview = Join-Path $donnees "webview"
        # Deja verifie avant Arreter-Tout : absent, ou marque « banc ».
        if ($Neuf -and (Test-Path -LiteralPath $donnees)) {
            Remove-Item -LiteralPath $donnees -Recurse -Force -ErrorAction SilentlyContinue
            if (Test-Path -LiteralPath $donnees) {
                # Il reste le sien : la marque revient avec ce qui n'est pas parti.
                Poser-Marque $donnees $MarqueBanc
                Write-Output "ECHEC : $donnees n'a pas pu etre efface en entier (un processus tient encore un fichier ?). Rien n'est lance."
                exit 1
            }
        }
        if (-not (Test-Path -LiteralPath $donnees)) {
            New-Item -ItemType Directory -Force $donnees | Out-Null
            Poser-Marque $donnees $MarqueBanc
        }
        # La fenetre d'abord : quand son script de construction repasse
        # (tauri.conf.json modifie), tauri-build recopie le vieux
        # binaires\gescom-serveur-*.exe sur target\debug\gescom-serveur.exe ;
        # construire le serveur ensuite remet le bon (verifie le 27/10/2026).
        Construire "gescom"
        Construire "gescom-serveur"
        # Le serveur, la fenetre le lance elle-meme quand le poste garde la
        # base (reseau.rs, demarrer_serveur_local) — comme chez le
        # commercant : rien ne le demarre ici. Avec GESCOM_DONNEES, il
        # tourne sur 127.0.0.1:40000, base serveur.db de ce dossier.
        $base = Join-Path $donnees "serveur.db"
        # La copie automatique regarde 5 min apres le demarrage : 20 s ici,
        # pour que p4 la voie partir sans attendre. Herite par la fenetre,
        # puis par le serveur qu'elle lance (comme GESCOM_DEMO).
        $env:GESCOM_SAUVEGARDE_DELAI_S = "20"
        Set-Content -Encoding ascii (Join-Path $Travail "moteur.txt") $base
        Ecran
        Attendre "http://$($HoteEcran):$PortEcran" "ecran"
        $env:GESCOM_DONNEES = $donnees
        $env:WEBVIEW2_USER_DATA_FOLDER = $webview
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$PortCdp"
        # Fenetre visible : cachee, WebView2 ralentit ses minuteries et ne peint plus.
        Lancer (Join-Path $Racine "src-tauri\target\debug\gescom.exe") @() (Join-Path $Travail "fenetre") "Normal" | Out-Null
        Attendre "http://127.0.0.1:$PortCdp/json/version" "fenetre (WebView2)"
    }
}
Attendre "http://$($HoteEcran):$PortEcran" "ecran"
Write-Output "PRET ($Mode)"
