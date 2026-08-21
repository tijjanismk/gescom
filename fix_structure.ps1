# Script PowerShell — déplacer les fichiers dans les bons dossiers
# Lancer depuis la racine du projet gescom

$src = "src"

# Pages
$pages = @(
    "Dashboard.tsx",
    "Ventes.tsx", 
    "Clients.tsx",
    "FicheClient.tsx",
    "FicheFournisseur.tsx",
    "Relances.tsx",
    "Rapports.tsx",
    "Pieces.tsx",
    "Parametres.tsx"
)

foreach ($f in $pages) {
    $from = Join-Path $src $f
    $to   = Join-Path $src "pages\$f"
    if (Test-Path $from) {
        Move-Item -Path $from -Destination $to -Force
        Write-Host "pages/$f OK"
    }
}

# Components
$components = @(
    "Layout.tsx",
    "FiltresAvances.tsx",
    "ModalNouvellePiece.tsx",
    "OngletChantiers.tsx"
)

foreach ($f in $components) {
    $from = Join-Path $src $f
    $to   = Join-Path $src "components\$f"
    if (Test-Path $from) {
        Move-Item -Path $from -Destination $to -Force
        Write-Host "components/$f OK"
    }
}

# Lib
$libs = @("genererPiece.ts")

foreach ($f in $libs) {
    $from = Join-Path $src $f
    $to   = Join-Path $src "lib\$f"
    if (Test-Path $from) {
        Move-Item -Path $from -Destination $to -Force
        Write-Host "lib/$f OK"
    }
}

Write-Host ""
Write-Host "Structure corrigee !"
