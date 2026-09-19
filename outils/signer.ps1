<#
.SYNOPSIS
    Signe un executable ou un installeur Gescom avec un certificat
    auto-signe, cree a la premiere utilisation.

.DESCRIPTION
    Une signature auto-signee ne fait pas taire SmartScreen sur une
    machine inconnue : Windows verifie la CHAINE de confiance, pas la
    presence d'une signature (D5). Elle fait deux choses utiles quand
    meme :

      - sur la machine du SERVEUR, ou l'installeur enregistre le
        certificat comme editeur de confiance, les installations
        suivantes ne posent plus la question ;
      - un fichier signe qui a ete modifie ne s'installe plus : la
        signature ne colle plus, et Windows le dit.

    Le certificat vit dans le magasin de l'utilisateur qui construit
    (Cert:\CurrentUser\My). La CLE PRIVEE ne sort jamais de la machine et
    n'entre jamais dans le depot. Seule la partie publique (.cer) est
    exportee, pour que l'installeur puisse l'enregistrer.

.PARAMETER Fichier
    Le .exe a signer. Plusieurs fichiers acceptes.

.PARAMETER ExporterVers
    Exporte la partie publique (.cer) a cet emplacement. Sans fichier a
    signer, ne fait que ca.

.PARAMETER SansHorodatage
    Ne contacte pas de serveur d'horodatage. Sans horodatage, la
    signature expire avec le certificat (10 ans ici) ; avec, elle reste
    valable apres. Utile hors ligne.

.EXAMPLE
    .\outils\signer.ps1 -ExporterVers .\dist\gescom.cer
    .\outils\signer.ps1 .\dist\gescom-serveur.exe
#>

param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]] $Fichier,
    [string] $ExporterVers,
    [switch] $SansHorodatage
)

$ErrorActionPreference = "Stop"
$SUJET = "CN=Gescom (auto-signe), O=Gescom, C=ML"

# --- Le certificat : on le retrouve, ou on le cree une fois ---
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $SUJET -and $_.NotAfter -gt (Get-Date) } |
    Sort-Object NotAfter -Descending | Select-Object -First 1

if (-not $cert) {
    Write-Host "Aucun certificat Gescom : creation (10 ans, signature de code)." -ForegroundColor Cyan
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $SUJET `
        -KeyUsage DigitalSignature `
        -KeyAlgorithm RSA -KeyLength 3072 `
        -HashAlgorithm SHA256 `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears(10) `
        -FriendlyName "Gescom — signature de code (auto-signe)"
}
Write-Host "Certificat : $($cert.Subject) — empreinte $($cert.Thumbprint), expire le $($cert.NotAfter.ToString('yyyy-MM-dd'))"

# --- La partie publique, pour l'installeur ---
if ($ExporterVers) {
    $dossier = Split-Path -Parent $ExporterVers
    if ($dossier -and -not (Test-Path $dossier)) { New-Item -ItemType Directory -Force $dossier | Out-Null }
    Export-Certificate -Cert $cert -FilePath $ExporterVers -Type CERT -Force | Out-Null
    Write-Host "Partie publique exportee : $ExporterVers" -ForegroundColor Green
}

if (-not $Fichier) { exit 0 }

# --- signtool, dans le SDK Windows ---
$kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
$signtool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like "*\x64\*" } |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) {
    Write-Host "signtool.exe introuvable sous $kits — installer le SDK Windows 10/11." -ForegroundColor Red
    exit 1
}

foreach ($f in $Fichier) {
    if (-not (Test-Path $f)) { Write-Host "Introuvable : $f" -ForegroundColor Red; exit 1 }
    $args = @("sign", "/sha1", $cert.Thumbprint, "/fd", "SHA256", "/d", "Gescom")
    if (-not $SansHorodatage) { $args += @("/tr", "http://timestamp.digicert.com", "/td", "SHA256") }
    $args += $f
    & $signtool.FullName @args
    if ($LASTEXITCODE -ne 0) {
        if (-not $SansHorodatage) {
            Write-Host "Echec avec horodatage (pas d'Internet ?) — nouvel essai sans." -ForegroundColor Yellow
            & $signtool.FullName sign /sha1 $cert.Thumbprint /fd SHA256 /d "Gescom" $f
        }
        if ($LASTEXITCODE -ne 0) { Write-Host "Signature impossible : $f" -ForegroundColor Red; exit 1 }
    }
    Write-Host "Signe : $f" -ForegroundColor Green
}
