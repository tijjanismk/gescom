# migrer_identifiant.ps1
#
# Changer l'identifiant Tauri change le dossier de données.
# Sans migration, l'application repart sur une base VIDE et
# l'ancienne reste orpheline dans %APPDATA%\com.user.gescom.
#
# Ce script copie la base de l'ancien dossier vers le nouveau.
# À lancer UNE FOIS, après avoir modifié tauri.conf.json et
# AVANT de relancer l'application.

$ancien = "$env:APPDATA\com.user.gescom"
$nouveau = "$env:APPDATA\ml.gescom.app"

Write-Host "`n=== Migration du dossier de donnees ===" -ForegroundColor Cyan
Write-Host "  Ancien  : $ancien"
Write-Host "  Nouveau : $nouveau`n"

if (-not (Test-Path $ancien)) {
    Write-Host "Ancien dossier introuvable — rien a migrer." -ForegroundColor Yellow
    Write-Host "C'est normal si tu n'as jamais lance l'application."
    exit
}

# L'application doit etre fermee : sinon le WAL n'est pas bascule
# et on copierait une base incomplete.
$proc = Get-Process -Name "gescom", "Gescom" -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "L'application est OUVERTE. Ferme-la avant de continuer." -ForegroundColor Red
    Write-Host "Sans cela, les ecritures recentes restent dans gescom.db-wal"
    Write-Host "et la copie serait incomplete."
    exit 1
}

if (Test-Path $nouveau) {
    Write-Host "Le nouveau dossier existe deja." -ForegroundColor Yellow
    $rep = Read-Host "Ecraser son contenu ? (o/N)"
    if ($rep -ne "o") { Write-Host "Annule."; exit }
} else {
    New-Item -ItemType Directory -Path $nouveau -Force | Out-Null
}

# Sauvegarde horodatee avant toute chose.
$backup = "$env:USERPROFILE\Desktop\gescom_backup_$(Get-Date -Format 'yyyyMMdd_HHmm')"
Copy-Item -Path $ancien -Destination $backup -Recurse -Force
Write-Host "Sauvegarde creee : $backup" -ForegroundColor Green

# Copier TOUS les fichiers, y compris -wal et -shm : une copie de
# gescom.db seul donnerait une base vide.
Copy-Item -Path "$ancien\*" -Destination $nouveau -Recurse -Force
Write-Host "Donnees copiees vers $nouveau" -ForegroundColor Green

Write-Host "`nContenu du nouveau dossier :" -ForegroundColor Cyan
Get-ChildItem $nouveau | ForEach-Object {
    Write-Host ("  {0,-24} {1,10:N0} octets" -f $_.Name, $_.Length)
}

Write-Host "`nRelance l'application et verifie que tes donnees sont la." -ForegroundColor Cyan
Write-Host "Si tout va bien, tu peux supprimer :" -ForegroundColor Gray
Write-Host "  $ancien" -ForegroundColor Gray
