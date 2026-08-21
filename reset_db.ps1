# reset_db.ps1 — Réinitialiser la base de données Gescom
# Lancer depuis la racine du projet gescom

$appData = $env:APPDATA
$dbPath = Join-Path $appData "com.gescom.app\gescom.db"
$dbPathAlt = Join-Path $appData "gescom\gescom.db"

# Trouver la base
$db = $null
if (Test-Path $dbPath) {
    $db = $dbPath
} elseif (Test-Path $dbPathAlt) {
    $db = $dbPathAlt
} else {
    # Chercher dans AppData
    $found = Get-ChildItem -Path $appData -Recurse -Filter "gescom.db" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { $db = $found.FullName }
}

if ($null -eq $db) {
    Write-Host "Base de données introuvable dans $appData" -ForegroundColor Red
    Write-Host "Cherche manuellement dans : $appData" -ForegroundColor Yellow
    exit 1
}

Write-Host "Base trouvée : $db" -ForegroundColor Cyan

# Confirmation
$confirm = Read-Host "Supprimer la base et recommencer à zéro ? (O/N)"
if ($confirm -ne "O" -and $confirm -ne "o") {
    Write-Host "Annulé." -ForegroundColor Yellow
    exit 0
}

# Arrêter l'app si elle tourne
Get-Process -Name "gescom" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

# Sauvegarder avant suppression
$backup = $db + ".backup_" + (Get-Date -Format "yyyyMMdd_HHmmss")
Copy-Item $db $backup
Write-Host "Sauvegarde : $backup" -ForegroundColor Green

# Supprimer
Remove-Item $db -Force
if (Test-Path ($db + "-wal")) { Remove-Item ($db + "-wal") -Force }
if (Test-Path ($db + "-shm")) { Remove-Item ($db + "-shm") -Force }

Write-Host ""
Write-Host "Base supprimée. Lance 'npm run tauri dev' — la base sera recréée." -ForegroundColor Green
