# L'essai a la main du proprietaire : Gescom tel qu'il tourne sur le poste
# qui garde la base (build de debogage), sur SES donnees d'essai, dans SON
# dossier.
#
#   .\outils\banc\essai-main.ps1                 reprend l'essai : serveur, ecran, fenetre
#   .\outils\banc\essai-main.ps1 -Preparer       prepare le dossier (copie, marque,
#                                                poste.json) et ne lance rien
#   .\outils\banc\essai-main.ps1 -Arreter        arrete ce qui tourne sur CE dossier,
#                                                rien d'autre (pas le banc)
#   .\outils\banc\essai-main.ps1 -Neuf           base neuve + donnees d'essai ; l'ancien
#                                                dossier est MIS DE COTE (renomme), jamais efface
#   ... -SansConstruire                          ne reconstruit ni gescom-serveur ni gescom
#   ... -Port 40050                              port du serveur de l'essai (40001 a 40100)
#
# Pourquoi un dossier a lui (27/10/2026) : l'essai a la main vivait dans
# .travail\monoposte, le dossier du banc, et `lancer.ps1 monoposte -Neuf`
# l'a efface, sauvegardes comprises, sans corbeille (ALERTES, 27/10).
# Desormais :
#   - tout vit dans outils\banc\.travail\essai-main\ (ignore par git) : la
#     base serveur.db, son journal, ses sauvegardes, poste.json, le profil
#     WebView2 (webview\), les sorties des processus, pids.txt ;
#   - le dossier porte sa marque (.proprietaire = essai-main) : ce script ne
#     travaille que dans un dossier qui la porte, le banc (lancer.ps1) que
#     dans un dossier marque « banc » ; chacun refuse celui de l'autre ;
#   - la premiere fois (dossier absent, sans -Neuf), les donnees de
#     .travail\monoposte sont COPIEES ici ; l'original n'est pas touche ;
#   - le serveur de l'essai ecoute 127.0.0.1:40050 (D39 : 40000-40100) ;
#     le banc monoposte prend 40000. poste.json retient 40050 : la fenetre
#     de debogage le lit par GESCOM_DONNEES et se branche sur ce serveur,
#     que ce script demarre AVANT elle ;
#   - -Neuf ne detruit rien : l'ancien dossier devient
#     essai-main.mis-de-cote-<date>, a effacer a la main quand on veut.
#
# A savoir : l'ecran de debogage est servi par Vite sur :1420 (devUrl de
# tauri.conf.json, fige au build), le port du banc. Le banc arrete ce qui
# ecoute sur 1420 a chacun de ses lancements : la fenetre de l'essai perd
# alors son ecran, mais son serveur et ses donnees ne bougent pas. Ne pas
# jouer le banc monoposte pendant un essai a la main.

param(
    [switch] $Preparer,
    [switch] $Arreter,
    [switch] $Neuf,
    [switch] $SansConstruire,
    [int] $Port = 40050
)

# Pas « Stop » : cargo parle sur stderr sans echouer, et PowerShell 5.1 en
# ferait une erreur. Les codes de sortie sont lus.
$ErrorActionPreference = "Continue"
$Ici = Split-Path -Parent $MyInvocation.MyCommand.Path
$Racine = (Resolve-Path (Join-Path $Ici "..\..")).Path
$Travail = Join-Path $Ici ".travail"
$Dossier = Join-Path $Travail "essai-main"
$Ancien = Join-Path $Travail "monoposte"
$Marque = "essai-main"
$Base = Join-Path $Dossier "serveur.db"
$Pids = Join-Path $Dossier "pids.txt"
$PortEcran = 1420
$ExeServeur = Join-Path $Racine "src-tauri\target\debug\gescom-serveur.exe"
$ExeFenetre = Join-Path $Racine "src-tauri\target\debug\gescom.exe"

# D39 : la fenetre ne joint que 40000-40100 (CSP). 40000 est au banc.
if ($Port -lt 40001 -or $Port -gt 40100) {
    Write-Output "REFUS : le port de l'essai doit etre entre 40001 et 40100 (plage D39) ; 40000 est celui du banc monoposte."
    exit 2
}

# ---------------------------------------------------------------------
#  Aides
# ---------------------------------------------------------------------

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

# Les gescom-serveur qui tiennent cette base : leur ligne de commande la
# nomme (--base), qu'ils viennent de ce script ou de la fenetre.
function Serveurs-Sur([string] $base) {
    $cible = $base.ToLowerInvariant()
    return @(Get-CimInstance Win32_Process -Filter "Name = 'gescom-serveur.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine.ToLowerInvariant().Contains($cible) })
}

function Qui-Ecoute([int] $port) {
    $c = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($c) { return [int]$c.OwningProcess }
    return 0
}

function Ligne-De([int] $id) {
    $p = Get-CimInstance Win32_Process -Filter "ProcessId = $id" -ErrorAction SilentlyContinue
    if ($p -and $p.CommandLine) { return [string]$p.CommandLine }
    return ""
}

function Repond([string] $url) {
    try { Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2 | Out-Null; return $true } catch { return $false }
}

function Attendre([string] $url, [string] $quoi, [int] $secondes) {
    for ($i = 0; $i -lt $secondes; $i++) {
        if (Repond $url) { return $true }
        Start-Sleep -Seconds 1
    }
    Write-Output "PAS PRET : $quoi ($url). Les sorties sont dans $Dossier (*.out, *.err)."
    return $false
}

# Ce que CE script a lance et qui tourne encore. pids.txt garde le nom avec
# le numero : un numero recycle par Windows n'est pas pris pour le notre.
function Nos-Processus {
    $vus = @()
    if (-not (Test-Path -LiteralPath $Pids)) { return $vus }
    foreach ($l in Get-Content -LiteralPath $Pids) {
        if ($l -match '^([\w-]+) (\d+)$') {
            $nom = $Matches[1]
            $p = Get-Process -Id ([int]$Matches[2]) -ErrorAction SilentlyContinue
            if ($p -and $p.ProcessName -eq $nom) { $vus += $p }
        }
    }
    return $vus
}

function Noter($p) {
    Add-Content -LiteralPath $Pids "$($p.ProcessName) $($p.Id)"
}

function Construire([string] $paquet) {
    Push-Location $Racine
    & (Join-Path $Racine "outils\cargo-tenace.ps1") build --package $paquet
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) {
        Write-Output "ECHEC : la construction de $paquet a echoue (code $code). Un gescom-serveur.exe ou gescom.exe de target\debug qui tourne (le banc, un test) verrouille son fichier : l'arreter, ou relancer avec -SansConstruire. Rien n'est lance."
        exit 1
    }
}

# ---------------------------------------------------------------------
#  Le dossier : le sien, ou rien
# ---------------------------------------------------------------------

if (Test-Path -LiteralPath $Dossier) {
    $vue = Lire-Marque $Dossier
    if ($vue -ne $Marque) {
        $dit = if ($vue) { $vue } else { "aucune" }
        Write-Output "REFUS : $Dossier ne porte pas la marque de l'essai a la main (.proprietaire attendu : $Marque ; trouve : $dit)."
        Write-Output "Ce script n'y touche pas. S'il n'a rien a garder : le deplacer ou le renommer soi-meme, puis relancer ; il sera recree."
        exit 2
    }
}

if ($Arreter) {
    if (-not (Test-Path -LiteralPath $Dossier)) { Write-Output "Rien a arreter : $Dossier n'existe pas."; exit 0 }
    $n = 0
    foreach ($p in (Nos-Processus)) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; $n++ }
    # Un serveur que la fenetre a relance elle-meme sur cette base n'est pas
    # dans pids.txt : il tient nos donnees, il est a nous.
    foreach ($s in (Serveurs-Sur $Base)) { Stop-Process -Id $s.ProcessId -Force -ErrorAction SilentlyContinue; $n++ }
    Remove-Item -LiteralPath $Pids -ErrorAction SilentlyContinue
    Write-Output "ARRETE : $n processus de l'essai a la main. Ceux du banc ne sont pas touches."
    exit 0
}

# -Neuf : l'ancien essai est mis de cote, jamais efface.
if ($Neuf -and (Test-Path -LiteralPath $Dossier)) {
    $vivants = @(Nos-Processus | ForEach-Object { $_.Id }) + @(Serveurs-Sur $Base | ForEach-Object { $_.ProcessId })
    if ($vivants.Count -gt 0) {
        Write-Output "REFUS : l'essai a la main tourne encore (processus $($vivants -join ', ')). D'abord : .\outils\banc\essai-main.ps1 -Arreter"
        exit 2
    }
    $cote = "essai-main.mis-de-cote-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
    try {
        Rename-Item -LiteralPath $Dossier -NewName $cote -ErrorAction Stop
    } catch {
        Write-Output "ECHEC : impossible de mettre l'ancien essai de cote ($($_.Exception.Message)). Un fichier est peut-etre tenu ouvert. Rien n'a ete efface."
        exit 1
    }
    Write-Output "L'ancien essai est mis de cote, intact : $(Join-Path $Travail $cote)"
}

# Premiere fois : un dossier neuf, avec la copie de l'essai d'avant s'il y
# en a un. .travail\monoposte marque « banc » n'est pas un essai a la main.
if (-not (Test-Path -LiteralPath $Dossier)) {
    $baseAncienne = Join-Path $Ancien "serveur.db"
    $aCopier = (-not $Neuf) -and (Test-Path -LiteralPath $baseAncienne) -and ((Lire-Marque $Ancien) -ne "banc")
    if ($aCopier) {
        # Une copie prise pendant qu'un serveur ecrit ne vaut rien : la base
        # vit surtout dans son -wal.
        $occupes = @(Serveurs-Sur $baseAncienne | ForEach-Object { $_.ProcessId })
        if ($occupes.Count -gt 0) {
            Write-Output "REFUS : un serveur tourne encore sur $baseAncienne (processus $($occupes -join ', ')) : une copie prise pendant qu'il ecrit ne serait pas fiable. Fermer cet essai (ou .\outils\banc\lancer.ps1 arreter s'il vient du banc), puis relancer. Rien n'a ete copie."
            exit 2
        }
    }
    New-Item -ItemType Directory -Force $Dossier | Out-Null
    if ($aCopier) {
        $source = (Get-Item -LiteralPath $Ancien).FullName.TrimEnd('\')
        Get-ChildItem -LiteralPath $source -Force | Copy-Item -Destination $Dossier -Recurse -Force
        # Chaque fichier de la source doit etre la, a la meme taille.
        $manque = @()
        $n = 0
        $octets = 0
        foreach ($f in Get-ChildItem -LiteralPath $source -Recurse -File -Force) {
            $relatif = $f.FullName.Substring($source.Length).TrimStart('\')
            $copie = Join-Path $Dossier $relatif
            if ((Test-Path -LiteralPath $copie) -and ((Get-Item -LiteralPath $copie -Force).Length -eq $f.Length)) {
                $n++
                $octets += $f.Length
            } else {
                $manque += $relatif
            }
        }
        if ($manque.Count -gt 0) {
            Write-Output "ECHEC de la copie ($($manque -join ', ')). L'original $Ancien n'a pas ete touche. $Dossier reste SANS marque : ce script le refusera ; le supprimer soi-meme, puis relancer."
            exit 1
        }
        $trace = "Copie de $source faite le $(Get-Date -Format 'dd/MM/yyyy HH:mm:ss') par essai-main.ps1 : $n fichiers, $octets octets. L'original n'a pas ete touche."
        Set-Content -LiteralPath (Join-Path $Dossier "copie-de-monoposte.txt") -Value $trace -Encoding utf8
        Write-Output "COPIE : $n fichiers ($([math]::Round($octets / 1MB, 1)) Mo) de $source vers $Dossier. L'original n'est pas touche."
    }
    Poser-Marque $Dossier $Marque
}

# poste.json : ce poste garde la base, son serveur est sur $Port. Le nom et
# l'empreinte sont gardes (la base copiee connait ce poste par elle).
$fPoste = Join-Path $Dossier "poste.json"
$nomPoste = $env:COMPUTERNAME
$empreinte = [guid]::NewGuid().ToString()
$actuel = "(aucun)"
if (Test-Path -LiteralPath $fPoste) {
    try {
        $texte = ([string](Get-Content -LiteralPath $fPoste -Raw -Encoding UTF8)).TrimStart([char]0xFEFF)
        $j = $texte | ConvertFrom-Json
        if ($j.poste_nom) { $nomPoste = [string]$j.poste_nom }
        if ($j.poste_empreinte) { $empreinte = [string]$j.poste_empreinte }
        $actuel = "$($j.mode) $($j.serveur)"
    } catch {
        $actuel = "(illisible)"
    }
}
$voulu = "monoposte 127.0.0.1:$Port"
if ($actuel -ne $voulu) {
    $json = [ordered]@{ mode = "monoposte"; serveur = "127.0.0.1:$Port"; poste_nom = $nomPoste; poste_empreinte = $empreinte } | ConvertTo-Json
    # Sans BOM : c'est ce que la fenetre ecrit elle-meme.
    [System.IO.File]::WriteAllText($fPoste, $json, (New-Object System.Text.UTF8Encoding $false))
    Write-Output "poste.json : ce poste garde la base, serveur 127.0.0.1:$Port (etait : $actuel)."
}

if ($Preparer) {
    Write-Output "PREPARE : $Dossier (marque $Marque). Rien n'est lance."
    exit 0
}

# ---------------------------------------------------------------------
#  Le serveur, l'ecran, la fenetre
# ---------------------------------------------------------------------

# Herites par le serveur et la fenetre. GESCOM_DONNEES n'est lu qu'en build
# de debogage : la fenetre y lit poste.json et, si elle doit relancer le
# serveur elle-meme, le lance sur ce dossier.
$env:GESCOM_DEMO = "1"
$env:GESCOM_DONNEES = $Dossier
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $Dossier "webview"
# Ce que le banc pose pour lui-meme, pas pour un essai a la main.
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
Remove-Item Env:GESCOM_SAUVEGARDE_DELAI_S -ErrorAction SilentlyContinue

$fenetreOuverte = @(Nos-Processus | Where-Object { $_.ProcessName -eq "gescom" }).Count -gt 0

# Le serveur de l'essai : celui qui tient deja le port, s'il est a nous.
$serveurLa = $false
$ecoute = Qui-Ecoute $Port
if ($ecoute -gt 0) {
    if ((Ligne-De $ecoute).ToLowerInvariant().Contains($Base.ToLowerInvariant())) {
        $serveurLa = $true
        Write-Output "Le serveur de l'essai tourne deja (127.0.0.1:$Port, processus $ecoute) : repris."
    } else {
        Write-Output "REFUS : le port $Port est tenu par un autre programme (processus $ecoute), pas par le serveur de l'essai. Choisir un autre port de la plage : -Port 40051 (par exemple)."
        exit 2
    }
} else {
    $autres = @(Serveurs-Sur $Base | ForEach-Object { $_.ProcessId })
    if ($autres.Count -gt 0) {
        Write-Output "REFUS : un serveur tient deja la base de l'essai sur un autre port (processus $($autres -join ', ')). Jamais deux serveurs sur une base : d'abord .\outils\banc\essai-main.ps1 -Arreter"
        exit 2
    }
}

if ($serveurLa -and $fenetreOuverte) {
    Write-Output "L'essai a la main est deja ouvert (serveur 127.0.0.1:$Port, fenetre lancee par ce script)."
    exit 0
}

# La fenetre d'abord : quand son script de construction repasse
# (tauri.conf.json modifie), tauri-build recopie le vieux
# binaires\gescom-serveur-*.exe sur target\debug\gescom-serveur.exe.
# Construire le serveur ensuite remet le bon (verifie le 27/10/2026).
if (-not $SansConstruire) {
    if (-not $fenetreOuverte) { Construire "gescom" }
    if (-not $serveurLa) { Construire "gescom-serveur" }
}
foreach ($exe in $ExeServeur, $ExeFenetre) {
    if (-not (Test-Path -LiteralPath $exe)) {
        Write-Output "ECHEC : $exe est introuvable. Construire d'abord (sans -SansConstruire)."
        exit 1
    }
}

if (-not $serveurLa) {
    $neuve = -not (Test-Path -LiteralPath $Base)
    $s = Start-Process -FilePath $ExeServeur -WorkingDirectory $Racine -WindowStyle Hidden -PassThru `
        -ArgumentList @("--hote", "127.0.0.1", "--port", "$Port",
            "--base", "`"$Base`"",
            "--journal", "`"$(Join-Path $Dossier 'serveur.log')`"",
            "--sauvegardes", "`"$(Join-Path $Dossier 'sauvegardes')`"") `
        -RedirectStandardOutput (Join-Path $Dossier "serveur.out") -RedirectStandardError (Join-Path $Dossier "serveur.err")
    Noter $s
    if (-not (Attendre "http://127.0.0.1:$Port/sante" "serveur de l'essai" 90)) { exit 1 }
    Write-Output "Serveur de l'essai : 127.0.0.1:$Port, base $Base."
    # Base neuve : le jeu de donnees d'essai, pose par le serveur (ses
    # commandes, ses regles), comme le 26/10.
    if ($neuve) {
        $journal = Join-Path $Dossier "donnees-essai.log"
        & node (Join-Path $Ici "donnees-essai.mjs") "127.0.0.1:$Port" 2>&1 | Out-File -Encoding utf8 $journal
        if ($LASTEXITCODE -ne 0) {
            Write-Output "ECHEC : les donnees d'essai n'ont pas pu etre posees (voir $journal). Le serveur tourne ; la fenetre n'est pas ouverte."
            exit 1
        }
        Write-Output "Donnees d'essai posees (detail : $journal)."
    }
}

if ($fenetreOuverte) {
    Write-Output "La fenetre de l'essai est deja ouverte."
    exit 0
}

# L'ecran : celui qui repond deja sur :1420 (le meme code), sinon le notre.
$urlEcran = "http://localhost:$PortEcran"
if (Repond $urlEcran) {
    Write-Output "L'ecran (Vite :$PortEcran) repond deja : repris."
} else {
    if ((Qui-Ecoute $PortEcran) -gt 0) {
        Write-Output "REFUS : le port $PortEcran est tenu par un programme qui ne sert pas l'ecran. La fenetre de debogage ne sait charger que celui-la (devUrl)."
        exit 2
    }
    $v = Start-Process -FilePath "node" -WorkingDirectory $Racine -WindowStyle Hidden -PassThru `
        -ArgumentList @("node_modules\vite\bin\vite.js", "--config", "outils\banc\vite.banc.config.ts",
            "--port", "$PortEcran", "--strictPort", "--host", "localhost") `
        -RedirectStandardOutput (Join-Path $Dossier "vite.out") -RedirectStandardError (Join-Path $Dossier "vite.err")
    Noter $v
    if (-not (Attendre $urlEcran "ecran (Vite)" 90)) { exit 1 }
}

$f = Start-Process -FilePath $ExeFenetre -WorkingDirectory $Racine -PassThru `
    -RedirectStandardOutput (Join-Path $Dossier "fenetre.out") -RedirectStandardError (Join-Path $Dossier "fenetre.err")
Noter $f
Write-Output "PRET : essai a la main, serveur 127.0.0.1:$Port, dossier $Dossier. Pour arreter : .\outils\banc\essai-main.ps1 -Arreter"
