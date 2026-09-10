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

    Le profil Public est volontairement laisse de cote. C'est celui des
    reseaux ou l'on ne connait pas ses voisins — un hotel, un cybercafe.
    Y ouvrir la base d'un commerce serait un mauvais service.

.PARAMETER Ouvrir
    Cree la regle.

.PARAMETER Fermer
    Supprime la regle.

.PARAMETER Etat
    N'affiche que la situation. Ne demande aucun droit particulier.

.PARAMETER Port
    Defaut 7300, celui de `protocole::PORT_DEFAUT`.

.EXAMPLE
    .\outils\parefeu.ps1 -Etat
    .\outils\parefeu.ps1 -Ouvrir      # PowerShell en administrateur
#>

param(
    [switch] $Ouvrir,
    [switch] $Fermer,
    [switch] $Etat,
    [int] $Port = 7300
)

$NOM = "Gescom serveur"

function Test-Administrateur {
    $identite = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identite)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Afficher-Etat {
    $regles = Get-NetFirewallRule -DisplayName $NOM -ErrorAction SilentlyContinue
    if (-not $regles) {
        Write-Host "Regle « $NOM » : ABSENTE" -ForegroundColor Yellow
        Write-Host "  Les postes caisse ne pourront pas joindre ce serveur."
    } else {
        foreach ($r in $regles) {
            $ports = ($r | Get-NetFirewallPortFilter).LocalPort
            Write-Host "Regle « $($r.DisplayName) » : PRESENTE" -ForegroundColor Green
            Write-Host "  direction $($r.Direction), action $($r.Action)"
            Write-Host "  profils   $($r.Profile)"
            Write-Host "  port      $ports"
        }
    }

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
        Write-Host "Adresse a saisir sur les caisses : $ip`:$Port"
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
    $regles = Get-NetFirewallRule -DisplayName $NOM -ErrorAction SilentlyContinue
    if (-not $regles) {
        Write-Host "Rien a fermer : la regle n'existe pas."
        exit 0
    }
    Remove-NetFirewallRule -DisplayName $NOM
    Write-Host "Regle « $NOM » supprimee." -ForegroundColor Green
    Write-Host "  Les postes caisse ne joindront plus ce serveur."
    exit 0
}

# --- Ouvrir ---
# On supprime d'abord : relancer le script avec un autre port doit
# remplacer la regle, pas en empiler une seconde qui laisserait
# l'ancien port ouvert sans que personne ne le sache.
Get-NetFirewallRule -DisplayName $NOM -ErrorAction SilentlyContinue |
    Remove-NetFirewallRule -ErrorAction SilentlyContinue

New-NetFirewallRule `
    -DisplayName $NOM `
    -Description "Laisse les postes caisse joindre Gescom Serveur sur le reseau local de la boutique." `
    -Direction Inbound `
    -Action Allow `
    -Protocol TCP `
    -LocalPort $Port `
    -Profile Domain,Private `
    -Enabled True | Out-Null

Write-Host "Regle « $NOM » creee : TCP $Port entrant, profils Domaine et Prive." -ForegroundColor Green
Write-Host ""
Afficher-Etat
Write-Host ""
Write-Host "Verifier depuis une caisse : http://<adresse>:$Port/sante" -ForegroundColor Cyan
