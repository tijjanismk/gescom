<#
.SYNOPSIS
    Refait les manuels PDF depuis leurs sources Markdown.

.DESCRIPTION
    Trois documents, une source chacun a la racine du depot :

        INSTALLATION.md   -> docs\manuels\Gescom_Installation.pdf
        MANUEL.md         -> docs\manuels\Gescom_Manuel.pdf
        MANUEL-EQUIPE.md  -> docs\manuels\Gescom_Equipe_Manuel.pdf

    Le PDF ne s'ecrit jamais a la main : on corrige le Markdown, puis on
    relance ce script. MANUEL.md est aussi celui que l'application
    imprime (src\lib\genererManuel.ts) ; la mise en page ici reprend la
    sienne : une couverture, un chapitre par page.

    Chaine : pandoc (Markdown -> HTML), puis Edge sans fenetre
    (HTML -> PDF, A4). pandoc vient d'Anaconda sur ce poste, ou du PATH.

.PARAMETER Seul
    Le nom d'une source ("MANUEL-EQUIPE.md") pour ne refaire qu'elle.

.EXAMPLE
    .\outils\manuels_pdf.ps1
#>

param(
    [string] $Seul
)

$ErrorActionPreference = "Stop"
$racine = Split-Path -Parent $PSScriptRoot
$sortie = Join-Path $racine "docs\manuels"
$travail = Join-Path $env:TEMP "gescom-manuels"
New-Item -ItemType Directory -Force $sortie, $travail | Out-Null

# --- Les outils ---
$pandoc = Get-Command pandoc -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $pandoc) { $pandoc = Join-Path $env:USERPROFILE "anaconda3\Library\bin\pandoc.exe" }
if (-not (Test-Path $pandoc)) { Write-Host "pandoc introuvable (PATH ou Anaconda)." -ForegroundColor Red; exit 1 }
$edge = @(
    "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
    "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { Write-Host "Microsoft Edge introuvable." -ForegroundColor Red; exit 1 }

# Textes de couverture en entites HTML : le script reste en ASCII
# (PowerShell 5.1 lit mal un fichier UTF-8 sans BOM).
$documents = @(
    @{ Source = "INSTALLATION.md";  Pdf = "Gescom_Installation.pdf";  Titre = "Guide d'installation";
       Note = "Les trois programmes, tous les cas : un seul poste, plusieurs caisses, le serveur, Gescom &Eacute;quipe, la mise &agrave; jour et les probl&egrave;mes courants." },
    @{ Source = "MANUEL.md";        Pdf = "Gescom_Manuel.pdf";        Titre = "Manuel d'utilisation";
       Note = "Les gestes de la journ&eacute;e : ouvrir la caisse, vendre, encaisser, cl&ocirc;turer. Gardez-le pr&egrave;s du comptoir &mdash; il r&eacute;pond plus vite qu'un appel." },
    @{ Source = "MANUEL-EQUIPE.md"; Pdf = "Gescom_Equipe_Manuel.pdf"; Titre = "Gescom &Eacute;quipe &mdash; Manuel";
       Note = "Le personnel, les jours travaill&eacute;s, les avances, la paie et le suivi des clients." }
)

# La mise en page du manuel de l'application : # = chapitre (une page),
# ## = section. Les tableaux et les encadres ne se coupent pas.
$style = @"
* { margin:0; padding:0; box-sizing:border-box; }
body { font-family:Georgia,'Times New Roman',serif; font-size:11.5pt; line-height:1.55; color:#1a1a1a; }
.couverture { height:250mm; display:flex; flex-direction:column; align-items:center; justify-content:center;
              text-align:center; padding:20mm; }
.couverture h1 { font-size:34pt; letter-spacing:1px; margin-bottom:6px; border:0; page-break-before:avoid; }
.couverture .soc { font-size:15pt; color:#444; margin-bottom:28px; }
.couverture .ver { font-size:10pt; color:#777; font-style:italic; }
.couverture .note { margin-top:40px; font-size:10pt; color:#666; max-width:110mm; line-height:1.5; text-align:center; }
h1 { font-size:19pt; margin:0 0 14px; padding-bottom:6px; border-bottom:2px solid #1a1a1a;
     page-break-before:always; page-break-after:avoid; }
h2 { font-size:12.5pt; margin:16px 0 5px; page-break-after:avoid; }
h3 { font-size:11.5pt; margin:12px 0 4px; page-break-after:avoid; }
p  { margin:0 0 8px; text-align:justify; }
ul, ol { margin:0 0 10px 20px; }
li { margin-bottom:3px; }
li > p { margin:0; }
hr { border:0; height:0; margin:0; }
code { font-family:'Courier New',monospace; font-size:10pt; background:#f0f0f0; padding:1px 3px; }
pre { font-family:'Courier New',monospace; font-size:9.5pt; background:#f5f5f3; border-left:3px solid #999;
      padding:9px 12px; margin:0 0 12px; line-height:1.45; white-space:pre-wrap; page-break-inside:avoid; }
pre code { background:none; padding:0; }
blockquote { border-left:3px solid #c00; background:#fff6f6; padding:9px 12px; margin:0 0 12px;
             font-size:10.5pt; page-break-inside:avoid; }
blockquote p:last-child { margin:0; }
table { width:100%; border-collapse:collapse; margin:0 0 12px; font-size:10pt; page-break-inside:avoid; }
th { background:#eee; text-align:left; padding:5px 7px; border-bottom:1.5px solid #333; }
td { padding:5px 7px; border-bottom:1px solid #ddd; vertical-align:top; }
@page { size:A4; margin:18mm 16mm; }
"@

function Echapper([string] $s) { $s.Replace("&", "&amp;").Replace("<", "&lt;").Replace(">", "&gt;") }

$imprime = (Get-Date).ToString("dd/MM/yyyy")
foreach ($d in $documents) {
    if ($Seul -and $d.Source -ne $Seul) { continue }
    $source = Join-Path $racine $d.Source
    if (-not (Test-Path $source)) { Write-Host "Absent : $($d.Source)" -ForegroundColor Yellow; continue }

    # Le premier titre sert de couverture, la ligne *Version...* aussi,
    # comme dans genererManuel.ts.
    $md = [IO.File]::ReadAllText($source, [Text.Encoding]::UTF8)
    $version = [regex]::Match($md, '(?m)^\*(Version[^*]*)\*').Groups[1].Value
    $corpsMd = [regex]::Replace($md, '\A# .*\r?\n', '')
    $corpsMd = [regex]::Replace($corpsMd, '(?m)^\*Version.*\*\r?\n', '')
    $fichierMd = Join-Path $travail ($d.Source)
    [IO.File]::WriteAllText($fichierMd, $corpsMd, (New-Object Text.UTF8Encoding $false))

    $corps = & $pandoc -f gfm -t html5 $fichierMd
    if ($LASTEXITCODE -ne 0) { Write-Host "pandoc a echoue : $($d.Source)" -ForegroundColor Red; exit 1 }
    $corps = $corps -join "`n"

    $html = @"
<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8"><title>Gescom &mdash; $($d.Titre)</title>
<style>$style</style></head><body>
<div class="couverture">
  <h1>$($d.Titre)</h1>
  <div class="soc">Gescom</div>
  <div class="ver">$(Echapper $version)</div>
  <p class="note">$($d.Note)</p>
  <p class="note" style="margin-top:14px">Imprim&eacute; le $imprime</p>
</div>
$corps
</body></html>
"@
    $fichierHtml = Join-Path $travail ([IO.Path]::ChangeExtension($d.Source, ".html"))
    [IO.File]::WriteAllText($fichierHtml, $html, (New-Object Text.UTF8Encoding $false))

    $pdf = Join-Path $sortie $d.Pdf
    if (Test-Path $pdf) { Remove-Item -LiteralPath $pdf -Force }
    $url = "file:///" + ($fichierHtml -replace '\\', '/')
    $p = Start-Process -FilePath $edge -PassThru -Wait -WindowStyle Hidden -ArgumentList @(
        "--headless=new", "--disable-gpu", "--no-first-run", "--no-pdf-header-footer",
        "--user-data-dir=`"$travail\edge`"", "--print-to-pdf=`"$pdf`"", "`"$url`""
    )
    if (-not (Test-Path $pdf)) { Write-Host "Edge n'a pas produit $($d.Pdf) (code $($p.ExitCode))." -ForegroundColor Red; exit 1 }
    $taille = [math]::Round((Get-Item $pdf).Length / 1KB)
    Write-Host "Pret : docs\manuels\$($d.Pdf) ($taille Ko)" -ForegroundColor Green
}
