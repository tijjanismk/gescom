// lib/genererManuel.ts — Le manuel d'utilisation, imprimable
//
// La source est MANUEL.md, importé tel quel. Recopier son texte ici en
// aurait fait une seconde version : celle du dépôt et celle imprimée
// auraient divergé au premier ajout, et c'est le document qu'un
// commerçant garde près de sa caisse.
//
// Le convertisseur ci-dessous ne traite QUE le sous-ensemble Markdown
// employé par ce fichier : titres, gras, listes, tableaux, blocs de
// code, citations, séparateurs. Ce n'est pas un moteur Markdown, et il
// ne doit pas le devenir — si le manuel a besoin d'autre chose, mieux
// vaut l'écrire autrement que d'élargir ce code.

import manuelMd from "../../MANUEL.md?raw";

function esc(s: string): string {
  return s.replace(/[&<>]/g, c => (
    { "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]!
  ));
}

/** Gras et italique, appliqués APRÈS l'échappement. */
function enrichir(s: string): string {
  return esc(s)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*])\*([^*]+?)\*/g, "$1<em>$2</em>")
    .replace(/`(.+?)`/g, "<code>$1</code>");
}

function convertir(md: string): string {
  const lignes = md.split(/\r?\n/);
  const sortie: string[] = [];

  let dansCode = false;
  let dansListe = false;
  let lignesTableau: string[][] = [];

  // Le manuel est écrit avec des lignes coupées à 70 colonnes. Sans
  // accumulation, chaque ligne deviendrait son propre paragraphe : le
  // texte sortirait haché et une citation de trois lignes donnerait
  // trois encadrés empilés. On regroupe donc jusqu'à la ligne vide.
  let paragraphe: string[] = [];
  let citation: string[] = [];

  const viderParagraphe = () => {
    if (paragraphe.length) {
      sortie.push(`<p>${enrichir(paragraphe.join(" "))}</p>`);
      paragraphe = [];
    }
  };
  const viderCitation = () => {
    if (citation.length) {
      sortie.push(`<blockquote>${enrichir(citation.join(" "))}</blockquote>`);
      citation = [];
    }
  };

  const fermerListe = () => {
    viderParagraphe(); viderCitation();
    if (dansListe) { sortie.push("</ul>"); dansListe = false; }
  };

  // Un tableau Markdown se reconnaît à sa ligne de séparation ; on
  // accumule donc les lignes avant de décider quoi en faire.
  const viderTableau = () => {
    if (lignesTableau.length === 0) return;
    const [entete, ...corps] = lignesTableau;
    sortie.push('<table><thead><tr>'
      + entete.map(c => `<th>${enrichir(c)}</th>`).join("")
      + "</tr></thead><tbody>"
      + corps.map(l => "<tr>"
          + l.map(c => `<td>${enrichir(c)}</td>`).join("") + "</tr>").join("")
      + "</tbody></table>");
    lignesTableau = [];
  };

  for (const ligne of lignes) {
    if (ligne.startsWith("```")) {
      fermerListe(); viderTableau();
      sortie.push(dansCode ? "</pre>" : "<pre>");
      dansCode = !dansCode;
      continue;
    }
    if (dansCode) { sortie.push(esc(ligne)); continue; }

    const t = ligne.trim();

    // Tableau : | a | b |  — la ligne de tirets se jette.
    if (t.startsWith("|") && t.endsWith("|")) {
      const cases = t.slice(1, -1).split("|").map(c => c.trim());
      if (!cases.every(c => /^:?-{2,}:?$/.test(c))) {
        fermerListe();
        lignesTableau.push(cases);
      }
      continue;
    }
    viderTableau();

    if (t === "") { fermerListe(); continue; }
    if (t === "---") { fermerListe(); sortie.push('<hr>'); continue; }

    if (t.startsWith("## ")) {
      fermerListe();
      sortie.push(`<h3>${enrichir(t.slice(3))}</h3>`);
      continue;
    }
    if (t.startsWith("# ")) {
      fermerListe();
      // Chaque chapitre démarre une page : un manuel se feuillette,
      // il ne se lit pas d'un trait.
      sortie.push(`<h2>${enrichir(t.slice(2))}</h2>`);
      continue;
    }
    if (t.startsWith("> ")) {
      viderParagraphe();
      if (dansListe) { sortie.push("</ul>"); dansListe = false; }
      citation.push(t.slice(2));
      continue;
    }
    if (t.startsWith("- ") || t.startsWith("* ")) {
      viderParagraphe(); viderCitation();
      if (!dansListe) { sortie.push("<ul>"); dansListe = true; }
      sortie.push(`<li>${enrichir(t.slice(2))}</li>`);
      continue;
    }

    viderCitation();
    if (dansListe) { sortie.push("</ul>"); dansListe = false; }
    paragraphe.push(t);
  }

  fermerListe();
  viderTableau();
  if (dansCode) sortie.push("</pre>");
  return sortie.join("\n");
}

export function genererManuelHTML(
  societe?: { nom?: string } | null,
  logoBase64?: string | null,
): string {
  // Le premier titre du fichier sert de couverture, pas de chapitre.
  const corps = convertir(
    manuelMd.replace(/^# .*\n/, "").replace(/^\*Version.*\*\n/m, ""),
  );
  const versionLigne = manuelMd.match(/^\*(Version[^*]*)\*/m)?.[1] ?? "";
  const maintenant = new Date();

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>Gescom — Manuel d'utilisation</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Georgia,'Times New Roman',serif; font-size:11.5pt;
         line-height:1.55; color:#1a1a1a; }

  .couverture { height:100vh; display:flex; flex-direction:column;
                align-items:center; justify-content:center; text-align:center;
                page-break-after:always; padding:20mm; }
  .couverture img { max-height:90px; margin-bottom:20px; }
  .couverture h1 { font-size:34pt; letter-spacing:1px; margin-bottom:6px; }
  .couverture .soc { font-size:15pt; color:#444; margin-bottom:28px; }
  .couverture .ver { font-size:10pt; color:#777; font-style:italic; }
  .couverture .note { margin-top:40px; font-size:10pt; color:#666;
                      max-width:110mm; line-height:1.5; }

  /* Un chapitre par page : le manuel se feuillette. */
  h2 { font-size:19pt; margin:0 0 14px; padding-bottom:6px;
       border-bottom:2px solid #1a1a1a; page-break-before:always;
       page-break-after:avoid; }
  /* La couverture ferme déjà sa page ; sans cette exception le premier
     chapitre en ouvrirait une seconde et laisserait une page blanche. */
  h2:first-of-type { page-break-before:avoid; }
  h3 { font-size:12.5pt; margin:16px 0 5px; page-break-after:avoid; }
  p  { margin:0 0 8px; text-align:justify; }
  ul { margin:0 0 10px 18px; }
  li { margin-bottom:3px; }
  hr { border:0; height:0; margin:0; }

  strong { font-weight:bold; }
  code { font-family:'Courier New',monospace; font-size:10pt;
         background:#f0f0f0; padding:1px 3px; }

  pre { font-family:'Courier New',monospace; font-size:9.5pt;
        background:#f5f5f3; border-left:3px solid #999;
        padding:9px 12px; margin:0 0 12px; line-height:1.45;
        white-space:pre-wrap; page-break-inside:avoid; }

  blockquote { border-left:3px solid #c00; background:#fff6f6;
               padding:9px 12px; margin:0 0 12px; font-size:10.5pt;
               page-break-inside:avoid; }

  table { width:100%; border-collapse:collapse; margin:0 0 12px;
          font-size:10pt; page-break-inside:avoid; }
  th { background:#eee; text-align:left; padding:5px 7px;
       border-bottom:1.5px solid #333; }
  td { padding:5px 7px; border-bottom:1px solid #ddd;
       vertical-align:top; }

  @media print {
    @page { size:A4; margin:18mm 16mm; }
    .couverture { height:auto; padding-top:60mm; }
  }
  @media screen {
    body { max-width:190mm; margin:0 auto; padding:16mm; background:#fff; }
  }
</style></head>
<body>

<div class="couverture">
  ${logoBase64 ? `<img src="${logoBase64}" alt="">` : ""}
  <h1>Manuel d'utilisation</h1>
  <div class="soc">${esc(societe?.nom || "Gescom")}</div>
  <div class="ver">${esc(versionLigne)}</div>
  <p class="note">
    Ce manuel décrit les gestes de la journée : ouvrir la caisse, vendre,
    encaisser, clôturer. Gardez-le près du comptoir — il répond plus vite
    qu'un appel.
  </p>
  <p class="note" style="margin-top:14px">
    Imprimé le ${maintenant.toLocaleDateString("fr-ML")}
  </p>
</div>

${corps}

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}
