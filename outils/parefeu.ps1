<#
.SYNOPSIS
    Ouvre ou ferme le port de Gescom Serveur dans le pare-feu Windows.

.DESCRIPTION
    Sans regle, le pare-feu refuse les connexions des postes caisse SANS
    RIEN ECRIRE : la caisse affiche « Serveur injoignable », le serveur
    ne voit rien passer, et il n'y a aucune trace a lire. C'est la panne
    la plus probable d'une installation multiposte, et la plus muette.

    Windows cree parfois une regle tout seul, au premier lancement,
    quand quelqu'un clique sur la boite de dialogue. Cette regle-la ne
    suffit pas :

      - elle ne vaut souvent que pour le profil PUBLIC, alors qu'un
        reseau de boutique est classe PRIVE ;
      - elle est attachee au CHEMIN de l'executable, donc perdue des
        que le serveur est deplace ou reinstalle ailleurs.

    Ce script pose une regle sur le PORT, pour les profils Domaine et
    Prive. Elle survit au deplacement de l'executable et couvre le cas
    reel : un reseau local de boutique.

    Il en pose une seconde, « Gescom serveur (recherche) » : UDP 40000
    entrant, memes profils. C'est par la que les caisses trouvent le
    serveur toutes seules (« Rechercher le serveur », 27/10/2026) ; sans
    elle, il faut saisir l'adresse a la main.

    Le profil Public est volontairement laisse de cote. C'est celui des
    reseaux ou l'on ne connait pas ses voisins — un hotel, un cybercafe.
    Y ouvrir la base d'un commerce serait un mauvais service.

.PARAMETER Ouvrir
    Cree les deux regles (TCP de la plage, UDP de la recherche).

.PARAMETER Fermer
    Supprime les deux regles.

.PARAMETER Etat
    N'affiche que la situation. Ne demande aucun droit particulier.

.PARAMETER Port
    Defaut 40000, celui de `protocole::PORT_DEFAUT` : l'adresse a saisir
    sur les caisses. La regle, elle, ouvre toute la plage 40000-40100
    (D39) : si 40000 est pris, le poste principal se rabat sur le
    suivant libre, et les caisses doivent encore le joindre.

.EXAMPLE
    .\outils\parefeu.ps1 -Etat
    .\outils\parefeu.ps1 -Ouvrir      # PowerShell en administrateur
#>

param(
    [switch] $Ouvrir,
    [switch] $Fermer,
    [switch] $Etat,
    [int] $Port = 40000
)

$NOM = "Gescom serveur"
# La plage du serveur (D39, `protocole::PORT_DEFAUT..=PORT_MAX`).
$PLAGE = "40000-40100"
# La recherche du serveur par les caisses (`noyau::decouverte::PORT`) :
# toujours UDP 40000, quel que soit le port TCP. Le nom est celui que
# lit le serveur (`reseau_local::REGLE_UDP`).
$NOM_RECHERCHE = "Gescom serveur (recherche)"
$PORT_RECHERCHE = 40000

function Test-Administrateur {
    $identite = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identite)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Afficher-Regle([string] $nom, [string] $siAbsente) {
    $regles = Get-NetFirewallRule -DisplayName $nom -ErrorAction SilentlyContinue
    if (-not $regles) {
        Write-Host "Regle « $nom » : ABSENTE" -ForegroundColor Yellow
        Write-Host "  $siAbsente"
    } else {
        foreach ($r in $regles) {
            $filtre = $r | Get-NetFirewallPortFilter
            Write-Host "Regle « $($r.DisplayName) » : PRESENTE" -ForegroundColor Green
            Write-Host "  direction $($r.Direction), action $($r.Action)"
            Write-Host "  profils   $($r.Profile)"
            Write-Host "  port      $($filtre.Protocol) $($filtre.LocalPort)"
        }
    }
}

function Afficher-Etat {
    Afficher-Regle $NOM "Les postes caisse ne pourront pas joindre ce serveur."
    Afficher-Regle $NOM_RECHERCHE "« Rechercher le serveur » ne trouvera pas ce poste : les caisses saisiront l'adresse."

    # Les regles posees automatiquement par Windows, souvent limitees au
    # profil Public : les montrer evite de croire que tout est en ordre.
    $auto = Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue |
        Where-Object { $_.Program -like "*gescom-serveur*" }
    if ($auto) {
        Write-Host ""
        Write-Host "Regles posees par Windows sur l'executable :" -ForegroundColor DarkGray
        foreach ($a in $auto) {
            $r = $a | Get-NetFirewallRule
            Write-Host "  $($r.Direction) $($r.Action) profils=$($r.Profile)" -ForegroundColor DarkGray
            Write-Host "    $($a.Program)" -ForegroundColor DarkGray
        }
        Write-Host "  ^ attachees a un chemin : perdues si le serveur bouge." -ForegroundColor DarkGray
    }

    Write-Host ""
    $ip = (Get-NetIPConfiguration |
        Where-Object { $_.IPv4DefaultGateway -ne $null } |
        Select-Object -First 1).IPv4Address.IPAddress
    if ($ip) {
        # Le nom du poste d'abord : il ne change pas quand la box
        # redistribue les adresses (l'IP, elle, peut changer).
        $nomPoste = if ($Port -eq 40000) { $env:COMPUTERNAME } else { "$($env:COMPUTERNAME):$Port" }
        Write-Host "Adresse a saisir sur les caisses : $nomPoste (ou $ip`:$Port)"
        Write-Host "  ou, sur la caisse : Parametres > Reseau > « Rechercher le serveur »"
    } else {
        Write-Host "Aucune passerelle : ce poste n'a pas de reseau local." -ForegroundColor Yellow
    }
}

if ($Etat -or (-not $Ouvrir -and -not $Fermer)) {
    Afficher-Etat
    exit 0
}

if (-not (Test-Administrateur)) {
    Write-Host "Droits administrateur requis." -ForegroundColor Red
    Write-Host "  Ouvrir PowerShell par un clic droit > « Executer en tant"
    Write-Host "  qu'administrateur », puis relancer cette commande."
    exit 1
}

if ($Fermer) {
    $fermees = 0
    foreach ($n in @($NOM, $NOM_RECHERCHE)) {
        if (Get-NetFirewallRule -DisplayName $n -ErrorAction SilentlyContinue) {
            Remove-NetFirewallRule -DisplayName $n
            Write-Host "Regle « $n » supprimee." -ForegroundColor Green
            $fermees++
        }
    }
    if ($fermees -eq 0) {
        Write-Host "Rien a fermer : aucune regle n'existe."
    } else {
        Write-Host "  Les postes caisse ne joindront plus ce serveur."
    }
    exit 0
}

# --- Ouvrir ---
# On supprime d'abord : relancer le script avec un autre port doit
# remplacer la regle, pas en empiler une seconde qui laisserait
# l'ancien port ouvert sans que personne ne le sache.
foreach ($n in @($NOM, $NOM_RECHERCHE)) {
    Get-NetFirewallRule -DisplayName $n -ErrorAction SilentlyContinue |
        Remove-NetFirewallRule -ErrorAction SilentlyContinue
}

New-NetFirewallRule `
    -DisplayName $NOM `
    -Description "Laisse les postes caisse joindre Gescom Serveur sur le reseau local de la boutique." `
    -Direction Inbound `
    -Action Allow `
    -Protocol TCP `
    -LocalPort $PLAGE `
    -Profile Domain,Private `
    -Enabled True | Out-Null

Write-Host "Regle « $NOM » creee : TCP $PLAGE entrant, profils Domaine et Prive." -ForegroundColor Green

# La recherche du serveur : les caisses posent leur question en diffusion
# UDP sur 40000. Memes profils : pas Public.
New-NetFirewallRule `
    -DisplayName $NOM_RECHERCHE `
    -Description "Laisse les postes caisse trouver Gescom Serveur sur le reseau local (Rechercher le serveur)." `
    -Direction Inbound `
    -Action Allow `
    -Protocol UDP `
    -LocalPort $PORT_RECHERCHE `
    -Profile Domain,Private `
    -Enabled True | Out-Null

Write-Host "Regle « $NOM_RECHERCHE » creee : UDP $PORT_RECHERCHE entrant, profils Domaine et Prive." -ForegroundColor Green
Write-Host ""
Afficher-Etat
Write-Host ""
Write-Host "Verifier depuis une caisse : http://<adresse>:$Port/sante" -ForegroundColor Cyan
