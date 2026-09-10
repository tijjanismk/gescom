// lib/modeles/rendu.ts — un modèle + des données → du HTML imprimable.
//
// Le rendu est ici et pas en Rust pour une raison : l'aperçu de
// l'éditeur doit se redessiner à chaque frappe. Un rendu côté serveur
// imposerait un aller-retour par caractère, ou un second moteur en
// TypeScript pour l'aperçu — et deux moteurs finissent toujours par ne
// plus donner le même document, ce qui est précisément ce qu'on ne peut
// pas se permettre sur une facture.
//
// La sortie est un document HTML complet, autonome, prêt pour la
// fenêtre d'impression Tauri. Aucune ressource externe : le poste d'une
// boutique de Bamako n'a pas forcément Internet au moment d'imprimer.

import { SCRIPT_IMPRESSION } from "@/lib/genererPDF";
import { enLettres } from "@/lib/genererRecu";
import type {
  Alignement,
  Bloc,
  Champ,
  Colonne,
  FormatValeur,
  Modele,
} from "./types";

export interface ImagesDocument {
  logo?: string | null;
  entete?: string | null;
  pied?: string | null;
}

export interface OptionsRendu {
  /** Aperçu : pas de script d'impression, pas de @page. */
  apercu?: boolean;
  images?: ImagesDocument;
}

// =====================================================================
//  Résolution des chemins
// =====================================================================

/**
 * `piece.numero` → la valeur, ou `undefined`.
 *
 * Volontairement tolérant : un chemin qui ne mène nulle part rend une
 * cellule vide, jamais une exception. Un modèle mal saisi doit produire
 * un document incomplet qu'on corrige en le regardant — pas un écran
 * blanc au moment d'imprimer devant le client.
 */
export function valeurAuChemin(donnees: unknown, chemin: string): unknown {
  if (!chemin) return undefined;
  return chemin.split(".").reduce<unknown>((acc, cle) => {
    if (acc === null || acc === undefined) return undefined;
    return (acc as Record<string, unknown>)[cle];
  }, donnees);
}

function esc(s: unknown): string {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// =====================================================================
//  Mise en forme des valeurs
// =====================================================================

function fmtMontant(n: number, devise: string): string {
  // Espace insécable avant la devise : sinon « 12 000 » se coupe en fin
  // de ligne et le montant se lit sur deux lignes.
  const corps = Math.round(n)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return `${corps} ${devise}`;
}

function fmtNombre(n: number): string {
  // Les quantités sont des flottants (2,5 sacs). On n'imprime les
  // décimales que si elles existent — « 3,00 kg » sur un ticket de
  // quincaillerie n'apporte rien et allonge la ligne.
  return Number.isInteger(n) ? String(n) : n.toFixed(2).replace(".", ",");
}

function fmtDate(v: unknown): string {
  const s = String(v ?? "");
  if (!s) return "";
  const d = new Date(s);
  if (Number.isNaN(d.getTime())) return s;
  const p = (x: number) => String(x).padStart(2, "0");
  return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${d.getFullYear()}`;
}

function fmtDateHeure(v: unknown): string {
  const s = String(v ?? "");
  if (!s) return "";
  const d = new Date(s);
  if (Number.isNaN(d.getTime())) return s;
  const p = (x: number) => String(x).padStart(2, "0");
  return `${fmtDate(v)} à ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function formater(
  valeur: unknown,
  format: FormatValeur,
  devise: string,
): string {
  if (valeur === null || valeur === undefined || valeur === "") return "";
  switch (format) {
    case "montant":
      return fmtMontant(Number(valeur) || 0, devise);
    case "nombre":
      return fmtNombre(Number(valeur) || 0);
    case "date":
      return fmtDate(valeur);
    case "date_heure":
      return fmtDateHeure(valeur);
    default:
      return String(valeur);
  }
}

/** `Merci {{tiers.nom}}` → `Merci Amadou`. */
function interpoler(texte: string, donnees: unknown, devise: string): string {
  return texte.replace(/\{\{\s*([\w.]+)\s*\}\}/g, (_, chemin: string) => {
    const v = valeurAuChemin(donnees, chemin);
    if (v === null || v === undefined) return "";
    return typeof v === "number" && chemin.startsWith("totaux.")
      ? fmtMontant(v, devise)
      : String(v);
  });
}

const ALIGN: Record<Alignement, string> = {
  gauche: "left",
  centre: "center",
  droite: "right",
};

// =====================================================================
//  Rendu des blocs
// =====================================================================

function rendreChamps(items: Champ[], donnees: unknown, devise: string): string {
  return items
    .map((c) => {
      const brute = valeurAuChemin(donnees, c.chemin);
      const v = formater(brute, c.format, devise);
      if (c.masquerSiVide && !v) return "";
      return `<div class="ch"><span class="ch-l">${esc(c.libelle)}</span><span class="ch-v">${esc(v)}</span></div>`;
    })
    .join("");
}

function rendreBloc(
  bloc: Bloc,
  donnees: unknown,
  devise: string,
  images: ImagesDocument,
): string {
  if (!bloc.visible) return "";

  switch (bloc.type) {
    case "entete": {
      const src =
        bloc.image === "logo"
          ? images.logo
          : bloc.image === "entete"
            ? images.entete
            : null;
      const img = src
        ? `<img class="logo" style="max-height:${bloc.hauteurMm}mm" src="${src}" alt="">`
        : "";
      const soc = bloc.afficherSociete
        ? `<div class="soc">
             <div class="soc-nom">${esc(valeurAuChemin(donnees, "societe.nom"))}</div>
             ${["adresse", "telephone", "email", "nif", "rccm"]
               .map((k) => {
                 const v = valeurAuChemin(donnees, `societe.${k}`);
                 if (!v) return "";
                 const prefixe = k === "nif" ? "NIF : " : k === "rccm" ? "RCCM : " : "";
                 return `<div class="soc-l">${esc(prefixe + String(v))}</div>`;
               })
               .join("")}
           </div>`
        : "";
      return `<header class="bloc entete" style="text-align:${ALIGN[bloc.alignement]}">${img}${soc}</header>`;
    }

    case "titre":
      return `<div class="bloc titre${bloc.trait ? " titre-trait" : ""}"
                   style="text-align:${ALIGN[bloc.alignement]};font-size:${bloc.taillePt}pt">
                ${esc(interpoler(bloc.texte, donnees, devise))}
              </div>`;

    case "champs": {
      const corps = rendreChamps(bloc.items, donnees, devise);
      if (!corps) return "";
      return `<section class="bloc champs cols-${bloc.colonnes}">
                ${bloc.titre ? `<div class="champs-t">${esc(bloc.titre)}</div>` : ""}
                <div class="champs-g">${corps}</div>
              </section>`;
    }

    case "tableau": {
      const brutes = valeurAuChemin(donnees, bloc.source);
      const lignes = Array.isArray(brutes) ? brutes : [];
      if (lignes.length === 0) {
        return bloc.siVide
          ? `<div class="bloc vide">${esc(bloc.siVide)}</div>`
          : "";
      }
      const entete = bloc.colonnes
        .map(
          (c: Colonne) =>
            `<th style="width:${c.largeur}%;text-align:${ALIGN[c.alignement]}">${esc(c.libelle)}</th>`,
        )
        .join("");
      const corps = lignes
        .map((ligne, i) => {
          const cellules = bloc.colonnes
            .map((c) => {
              const v = formater(
                valeurAuChemin(ligne, c.chemin),
                c.format,
                devise,
              );
              return `<td style="text-align:${ALIGN[c.alignement]}">${esc(v)}</td>`;
            })
            .join("");
          const cls = bloc.zebre && i % 2 === 1 ? ' class="z"' : "";
          return `<tr${cls}>${cellules}</tr>`;
        })
        .join("");
      return `<table class="bloc tab"><thead><tr>${entete}</tr></thead><tbody>${corps}</tbody></table>`;
    }

    case "totaux": {
      const n = bloc.items.length;
      const rangs = bloc.items
        .map((c, i) => {
          const brute = valeurAuChemin(donnees, c.chemin);
          const v = formater(brute, c.format, devise);
          if (c.masquerSiVide && (!v || Number(brute) === 0)) return "";
          const fort = bloc.accentuerDernier && i === n - 1 ? " fort" : "";
          return `<div class="tot-l${fort}"><span>${esc(c.libelle)}</span><span>${esc(v)}</span></div>`;
        })
        .join("");
      if (!rangs) return "";
      return `<section class="bloc totaux">${rangs}</section>`;
    }

    case "texte": {
      const t = interpoler(bloc.contenu, donnees, devise).trim();
      if (!t) return "";
      const style = [
        `text-align:${ALIGN[bloc.alignement]}`,
        `font-size:${bloc.taillePt}pt`,
        bloc.italique ? "font-style:italic" : "",
      ]
        .filter(Boolean)
        .join(";");
      return `<div class="bloc txt${bloc.cadre ? " cadre" : ""}" style="${style}">${esc(t).replace(/\n/g, "<br>")}</div>`;
    }

    case "signatures": {
      // Les deux noms vides : le bloc disparaît. C'est ainsi qu'on
      // retire les signatures d'un document, sans réglage on/off en
      // plus.
      if (!bloc.gauche && !bloc.droite) return "";
      const c = (nom: string) =>
        `<div class="sig-c">${nom ? `<div class="sig-t"></div><div class="sig-n">${esc(nom)}</div>` : ""}</div>`;
      return `<section class="bloc sigs">${c(bloc.gauche)}${c(bloc.droite)}</section>`;
    }

    case "trait":
      return `<hr class="bloc sep">`;

    case "espace":
      return `<div class="bloc" style="height:${bloc.hauteurMm}mm"></div>`;

    case "saut_page":
      return `<div class="saut"></div>`;

    default:
      return "";
  }
}

// =====================================================================
//  Feuille de style
// =====================================================================

const LARGEUR_MM: Record<string, number> = {
  a4: 210,
  a5: 148,
  thermique_80: 80,
  thermique_58: 58,
};

function styles(modele: Modele, apercu: boolean): string {
  const { page } = modele.contenu;
  const thermique = modele.format.startsWith("thermique");
  const largeur = LARGEUR_MM[modele.format] ?? 210;
  // Sur un rouleau, la hauteur est libre : `auto`. Une hauteur fixe
  // couperait le ticket au milieu d'une ligne ou cracherait du papier
  // blanc, selon le sens de l'erreur.
  const taillePage = thermique
    ? `${largeur}mm auto`
    : modele.format === "a5"
      ? "A5"
      : "A4";
  const marge = thermique ? Math.min(page.margeMm, 4) : page.margeMm;

  return `
  ${apercu ? "" : `@page { size: ${taillePage}; margin: ${marge}mm; }`}
  * { box-sizing: border-box; }
  body {
    margin: 0;
    font-family: ${page.police};
    font-size: ${page.taillePt}pt;
    color: #000;
    line-height: 1.4;
    ${apercu ? `width:${largeur}mm;padding:${marge}mm;background:#fff;margin:0 auto;` : ""}
  }
  .bloc { margin-bottom: 3mm; }
  .entete .logo { display: block; margin-bottom: 2mm; }
  .entete[style*="center"] .logo { margin-left: auto; margin-right: auto; }
  .entete[style*="right"] .logo { margin-left: auto; }
  .soc-nom { font-weight: 700; font-size: 1.25em; }
  .soc-l { font-size: .88em; }
  .titre { font-weight: 700; letter-spacing: .04em; }
  .titre-trait { border-bottom: 2px solid ${page.couleurAccent}; padding-bottom: 1.5mm; }
  .champs-t { font-weight: 700; font-size: .9em; margin-bottom: 1mm;
              color: ${page.couleurAccent}; }
  .champs-g { display: grid; gap: 0 6mm; }
  .cols-1 .champs-g { grid-template-columns: 1fr; }
  .cols-2 .champs-g { grid-template-columns: 1fr 1fr; }
  .cols-3 .champs-g { grid-template-columns: 1fr 1fr 1fr; }
  .ch { display: flex; justify-content: space-between; gap: 3mm;
        border-bottom: .2mm dotted #bbb; padding: .6mm 0; }
  .ch-l { color: #444; }
  .ch-v { font-weight: 600; text-align: right; }
  table.tab { width: 100%; border-collapse: collapse; }
  .tab th { border-bottom: .5mm solid ${page.couleurAccent};
            padding: 1.2mm 1mm; font-size: .85em; text-transform: uppercase;
            letter-spacing: .03em; }
  .tab td { border-bottom: .2mm solid #ddd; padding: 1.2mm 1mm; }
  .tab tr.z td { background: #f5f5f5; }
  .totaux { margin-left: auto; width: 62%; }
  .tot-l { display: flex; justify-content: space-between; padding: .8mm 0;
           border-bottom: .2mm solid #eee; }
  .tot-l.fort { font-weight: 700; font-size: 1.15em; border-top: .5mm solid #000;
                border-bottom: none; padding-top: 1.5mm; }
  .txt { white-space: pre-line; }
  .cadre { border: .3mm solid #999; padding: 2mm; border-radius: 1mm; }
  .sigs { display: flex; justify-content: space-between; gap: 10mm;
          margin-top: 10mm; }
  .sig-c { flex: 1; }
  .sig-t { border-top: .3mm solid #000; margin-bottom: 1mm; }
  .sig-n { font-size: .85em; text-align: center; }
  .sep { border: none; border-top: .3mm solid #999; margin: 3mm 0; }
  .vide { font-style: italic; color: #666; text-align: center; padding: 4mm 0; }
  .saut { page-break-after: always; }
  ${
    thermique
      ? `.totaux { width: 100%; } .champs-g { grid-template-columns: 1fr !important; }
         .tab th, .tab td { padding: .8mm .4mm; font-size: .95em; }`
      : ""
  }
  `;
}

// =====================================================================
//  Entrée publique
// =====================================================================

/** Le corps du document, sans `<html>` — c'est ce que l'aperçu injecte. */
export function rendreCorps(
  modele: Modele,
  donnees: unknown,
  options: OptionsRendu = {},
): string {
  const devise =
    (valeurAuChemin(donnees, "societe.devise") as string) || "FCFA";
  const images = options.images ?? {};
  return modele.contenu.blocs
    .map((b) => rendreBloc(b, donnees, devise, images))
    .join("\n");
}

/** Le document complet, prêt à imprimer. */
export function rendreModele(
  modele: Modele,
  donnees: unknown,
  options: OptionsRendu = {},
): string {
  const apercu = options.apercu ?? false;
  const titre =
    (valeurAuChemin(donnees, "piece.numero") as string) || modele.nom;
  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="utf-8">
<title>${esc(titre)}</title>
<style>${styles(modele, apercu)}</style>
</head><body>
${rendreCorps(modele, donnees, options)}
${apercu ? "" : SCRIPT_IMPRESSION}
</body></html>`;
}

/** Le montant total écrit en toutes lettres, pour le contexte. */
export function montantEnLettres(n: number, devise: string): string {
  return `${enLettres(Math.round(n))} ${devise}`;
}
