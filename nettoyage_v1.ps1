# nettoyage_v1.ps1
# Supprimer la couche morte persistance/ et les fichiers inutiles.
# Lancer depuis la racine du projet gescom.
# PowerShell : clic droit -> Executer avec PowerShell

Set-Location $PSScriptRoot

Write-Host "`n=== 1. Suppression de la couche persistance/ morte ===" -ForegroundColor Cyan

$morts = @(
    "src-tauri/src/persistance/ventes.rs",
    "src-tauri/src/persistance/paiements.rs",
    "src-tauri/src/persistance/retours.rs",
    "src-tauri/src/persistance/transferts.rs",
    "src-tauri/src/persistance/factures.rs",
    "src-tauri/src/persistance/mouvements_stock.rs",
    "src-tauri/src/persistance/sessions_caisse.rs",
    "src-tauri/src/persistance/articles.rs",
    "src-tauri/src/persistance/clients.rs",
    "src-tauri/src/persistance/depots.rs",
    "src-tauri/src/persistance/unites_vente.rs",
    "src-tauri/src/persistance/avoirs.rs",
    "src-tauri/src/persistance/fournisseurs.rs",
    "src-tauri/src/porte.rs"
)

foreach ($f in $morts) {
    if (Test-Path $f) {
        git rm $f
        Write-Host "  supprime : $f" -ForegroundColor Green
    } else {
        Write-Host "  absent   : $f (ok)" -ForegroundColor Gray
    }
}

Write-Host "`n=== 2. Generateur d'impression mort ===" -ForegroundColor Cyan

$usage1 = git grep -l "genererFactureHTML" -- "src/" 2>$null
$usage2 = git grep -l "lire_donnees_facture" -- "src/" 2>$null

if (-not $usage1 -and -not $usage2) {
    if (Test-Path "src/lib/genererFacture.ts") {
        git rm "src/lib/genererFacture.ts"
        Write-Host "  supprime : src/lib/genererFacture.ts" -ForegroundColor Green
    }
    Write-Host "  Pense a retirer manuellement dans lib.rs :" -ForegroundColor Yellow
    Write-Host "    commandes::societe::lire_donnees_facture," -ForegroundColor Yellow
    Write-Host "  Et la fonction lire_donnees_facture dans societe.rs" -ForegroundColor Yellow
} else {
    Write-Host "  genererFactureHTML ou lire_donnees_facture toujours utilises :" -ForegroundColor Yellow
    if ($usage1) { $usage1 | ForEach-Object { Write-Host "    $_" } }
    if ($usage2) { $usage2 | ForEach-Object { Write-Host "    $_" } }
    Write-Host "  Ne pas supprimer." -ForegroundColor Yellow
}

Write-Host "`n=== 3. Contenu de persistance/ apres nettoyage ===" -ForegroundColor Cyan
Get-ChildItem "src-tauri/src/persistance/" | ForEach-Object { Write-Host "  $($_.Name)" }
Write-Host "  Attendu : mod.rs, schema.sql, journal.rs" -ForegroundColor Gray

Write-Host "`n=== 4. Regenerer schema.sql ===" -ForegroundColor Cyan
$gescom_db = "$env:APPDATA\com.user.gescom\gescom.db"
if (Test-Path $gescom_db) {
    $sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
    if ($sqlite) {
        Write-Host "  Base trouvee. Ferme l'application Gescom maintenant" -ForegroundColor Yellow
        Read-Host "  Appuie sur Entree quand c'est fait"
        sqlite3 $gescom_db ".schema" | Out-File -Encoding UTF8 "schema_brut.sql"
        Write-Host "  schema_brut.sql cree a la racine du projet." -ForegroundColor Green
        Write-Host "  Envoie ce fichier pour le reformatage." -ForegroundColor Green
    } else {
        Write-Host "  sqlite3 non installe. Alternative :" -ForegroundColor Yellow
        Write-Host "  1. Ouvre DB Browser for SQLite"
        Write-Host "  2. Fichier > Ouvrir > $gescom_db"
        Write-Host "  3. Onglet 'Execute SQL'"
        Write-Host "  4. Colle et execute :"
        Write-Host "     SELECT sql FROM sqlite_master WHERE type='table';" -ForegroundColor Cyan
        Write-Host "  5. Copie le resultat et envoie-le."
    }
} else {
    Write-Host "  Base non trouvee a $gescom_db" -ForegroundColor Red
    Write-Host "  Lance d'abord l'application une fois pour creer la base."
}

Write-Host "`nTermine. Lance 'cargo build' pour verifier." -ForegroundColor Cyan
