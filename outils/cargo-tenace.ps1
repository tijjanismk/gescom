<#
.SYNOPSIS
    Relance une commande cargo quand Smart App Control bloque un binaire
    fraîchement lié.

.DESCRIPTION
    Smart App Control refuse d'exécuter un fichier qu'il ne connaît pas.
    Un binaire de test que le compilateur vient de produire n'a, par
    construction, aucune réputation : il est parfois bloqué, et cargo
    s'arrête sur « Une stratégie de contrôle d'application a bloqué ce
    fichier » (erreur 4551).

    Le blocage n'est PAS un échec de test — mais le message ne le dit
    pas, et on perd du temps à chercher une régression qui n'existe pas.
    Vingt-trois blocages en vingt-quatre heures sur ce poste.

    Relier le fichier produit un binaire différent, qui passe presque
    toujours. Ce script supprime celui que cargo a nommé et recommence.

    Il ne touche à AUCUN réglage du système. C'est un contournement,
    pas une solution : la vraie décision — garder ou non Smart App
    Control sur un poste de développement — appartient au propriétaire
    de la machine.

.EXAMPLE
    .\outils\cargo-tenace.ps1 test --workspace
    .\outils\cargo-tenace.ps1 build --release
#>

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Arguments
)

# Au-delà, c'est autre chose qu'un blocage passager : on s'arrête plutôt
# que de boucler en effaçant des binaires.
#
# Volontairement une variable et non un paramètre : PowerShell donnerait
# à un second paramètre la première position, et « cargo-tenace test »
# se ferait interpréter comme un nombre d'essais valant « test ».
$Essais = if ($env:GESCOM_ESSAIS) { [int] $env:GESCOM_ESSAIS } else { 4 }

if (-not $Arguments -or $Arguments.Count -eq 0) {
    Write-Host "Usage : .\outils\cargo-tenace.ps1 test --workspace" -ForegroundColor Yellow
    exit 1
}

# Le script vit dans outils/ ; cargo doit tourner dans src-tauri/.
$racine = Split-Path -Parent $PSScriptRoot
$manifeste = Join-Path $racine "src-tauri"
if (-not (Test-Path (Join-Path $manifeste "Cargo.toml"))) {
    Write-Host "Cargo.toml introuvable dans $manifeste" -ForegroundColor Red
    exit 1
}

# 4551 en français comme en anglais : on reconnaît les deux, plus le
# code brut, pour ne pas dépendre de la langue de l'installation.
$signes = @(
    'os error 4551',
    "strat.gie de contr.le d'application",
    'application control policy'
)

for ($i = 1; $i -le $Essais; $i++) {
    $sortie = & cargo @Arguments --manifest-path (Join-Path $manifeste "Cargo.toml") 2>&1
    $code = $LASTEXITCODE
    $texte = $sortie -join "`n"
    $sortie | ForEach-Object { Write-Host $_ }

    if ($code -eq 0) { exit 0 }

    $bloque = $false
    foreach ($s in $signes) { if ($texte -match $s) { $bloque = $true; break } }
    if (-not $bloque) {
        # Un vrai échec : test rouge, erreur de compilation. On rend la
        # main sans rien effacer.
        exit $code
    }

    # Cargo nomme le fichier entre accents graves.
    $m = [regex]::Match($texte, 'could not execute process `([^`]+)`')
    if (-not $m.Success) {
        Write-Host "Blocage détecté, mais le fichier n'est pas nommé — abandon." -ForegroundColor Yellow
        exit $code
    }

    $fichier = $m.Groups[1].Value
    Write-Host ""
    Write-Host "[tenace] Smart App Control a bloqué :" -ForegroundColor Yellow
    Write-Host "         $fichier"
    Write-Host "[tenace] Suppression et nouvelle liaison (essai $i/$Essais)…" -ForegroundColor Yellow
    Write-Host ""
    Remove-Item -LiteralPath $fichier -Force -ErrorAction SilentlyContinue
    # Le .pdb voisin n'est pas bloquant, mais le laisser orphelin fait
    # râler le lieur au coup suivant.
    Remove-Item -LiteralPath ([System.IO.Path]::ChangeExtension($fichier, ".pdb")) `
        -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "[tenace] Toujours bloqué après $Essais essais." -ForegroundColor Red
Write-Host "         Ce n'est plus un aléa de réputation. Voir" -ForegroundColor Red
Write-Host "         AI_CONTEXT/modules/environnement-windows.md" -ForegroundColor Red
exit 1
