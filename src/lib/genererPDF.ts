// lib/genererPDF.ts — Génération PDF via impression HTML → PDF navigateur
// Pas de dépendance externe — utilise window.print() avec CSS @page

/** Formats d'impression. Un seul generateur pour toutes les pieces. */
// =====================================================================
//  Script d'impression — commun a tous les formats
// =====================================================================
//
// `onafterprint` se declenche a la fermeture de la boite de dialogue,
// que l'utilisateur ait imprime OU annule. Dans les deux cas la fenetre
// n'a plus d'objet : on la ferme.
//
// `window.close()` est refuse par certains moteurs sur une fenetre que
// le script n'a pas ouverte. D'ou le bouton de repli, qui n'apparait
// qu'apres 2,5 s et seulement si la fenetre est encore la. Il est
// masque a l'impression : il ne doit jamais finir sur le papier.
const SCRIPT_IMPRESSION = `
<style>@media print { #gescom-fermer { display:none !important; } }</style>
<script>
(function () {
  function fermer() { try { window.close(); } catch (e) {} }
  window.onload = function () {
    window.focus();
    window.onafterprint = function () { setTimeout(fermer, 300); };
    window.print();
    setTimeout(function () {
      if (window.closed || document.getElementById('gescom-fermer')) return;
      var b = document.createElement('button');
      b.id = 'gescom-fermer';
      b.textContent = 'Fermer';
      b.onclick = fermer;
      b.setAttribute('style',
        'position:fixed;top:12px;right:12px;z-index:99999;padding:9px 20px;' +
        'font:14px Arial,sans-serif;background:#111;color:#fff;border:0;' +
        'border-radius:6px;cursor:pointer;box-shadow:0 2px 8px rgba(0,0,0,.3)');
      document.body.appendChild(b);
    }, 2500);
  };
})();
<\/script>`;

export type FormatImpression =
  | "a4" | "a5" | "thermique_58" | "thermique_80"
  // Bon de sortie : le meme document SANS AUCUN montant, remis au
  // client pour le magasinier. Dans une quincaillerie, celui qui
  // encaisse n'est pas celui qui delivre la marchandise ; le bon est
  // la seule piece qui circule entre les deux.
  | "bon_sortie"
  // Facture ET bon dans UN document : une seule boite de dialogue.
  | "a4_et_bon"
  | "a5_et_bon";

export interface DonneesPiece {
  piece: any;
  lignes: any[];
  societe: any;
  totaux: any;
}

function fmt(n: number, devise = "FCFA"): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " " + devise;
}
function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

/**
 * Date ET heure.
 *
 * Deux ventes du meme jour au meme client se distinguent mal sur une
 * facture qui ne porte que la date. L'heure permet de retrouver
 * l'operation dans le journal et de trancher une contestation.
 */
function fmtDateHeure(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  }) + " à " + d.toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}

const TITRES: Record<string, string> = {
  devis:                    "DEVIS",
  proforma:                 "FACTURE PROFORMA",
  commande_client:          "BON DE COMMANDE",
  bon_livraison:            "BON DE LIVRAISON",
  facture:                  "FACTURE",
  facture_acompte:          "FACTURE D'ACOMPTE",
  avoir_client:             "AVOIR",
  bon_commande_fournisseur: "BON DE COMMANDE",
  bon_reception:            "BON DE RÉCEPTION",
  facture_fournisseur:      "FACTURE FOURNISSEUR",
  avoir_fournisseur:        "AVOIR FOURNISSEUR",
};

// =====================================================================
//  Générer HTML A4 — avec support nb_copies (factures en 5 exemplaires)
// =====================================================================

export function genererPieceHTML(
  donnees: DonneesPiece,
  logoBase64?: string | null,
  format: "a4" | "a5" = "a4",
  // Bandeau pleine largeur. Present, il REMPLACE le logo et le bloc de
  // coordonnees : c'est le papier a en-tete du commercant, qui porte
  // deja son nom, son adresse et son telephone. Les repeter dessous
  // ferait doublon sur le document imprime.
  enteteBase64?: string | null,
  // Bandeau de bas de page. Present, il remplace la ligne de texte
  // `pied_facture`, qu'il porte deja en general.
  piedBase64?: string | null,
): string {
  const { piece, lignes, societe, totaux } = donnees;
  const devise = societe.devise ?? "FCFA";
  const titre = TITRES[piece.type_piece] ?? "PIÈCE COMMERCIALE";
  const isA5 = format === "a5";

  const logoHtml = logoBase64
    ? `<img src="${logoBase64}" alt="Logo"
           style="max-height:55px;max-width:180px;object-fit:contain;display:block"/>`
    : "";

  const avecEntete = !!enteteBase64;
  const enteteHtml = avecEntete
    ? `<div style="margin-bottom:12px">
         <img src="${enteteBase64}" alt=""
              style="width:100%;height:auto;display:block"/>
       </div>`
    : "";

  const hasTVA = lignes.some((l: any) => l.taux_tva && l.taux_tva > 0);

  function lignesTableau(): string {
    return lignes.map((l: any, i: number) => {
      const tva_pct = l.taux_tva ? (l.taux_tva * 100).toFixed(0) + "%" : "—";
      return `
        <tr style="background:${i % 2 === 0 ? "#fff" : "#f9f9f9"};border-bottom:1px solid #eee">
          <td style="padding:5px 6px">${l.article_nom}
            ${l.remise_pct > 0
              ? `<br><small style="color:#888">Remise ${l.remise_pct}%</small>`
              : ""}
          </td>
          <td style="text-align:center;padding:5px 4px">
            ${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)} ${l.unite_libelle}
          </td>
          <td style="text-align:right;padding:5px 4px">${fmt(l.prix_unitaire, devise)}</td>
          ${hasTVA ? `<td style="text-align:center;padding:5px 4px;color:#555">${tva_pct}</td>` : ""}
          <td style="text-align:right;padding:5px 4px;font-weight:600">
            ${fmt(l.montant_ht, devise)}
          </td>
        </tr>`;
    }).join("");
  }

  function totauxHTML(): string {
    const rows = [];
    // HT seulement si TVA présente
    if (hasTVA && totaux.total_ht > 0) {
      rows.push(`<tr>
        <td style="padding:3px 8px;color:#555">Total HT</td>
        <td style="padding:3px 8px;text-align:right">${fmt(totaux.total_ht, devise)}</td>
      </tr>`);
    }
    // Remise
    if (totaux.remise_montant > 0) {
      rows.push(`<tr>
        <td style="padding:3px 8px;color:#e65c00">Remise (${piece.remise_globale}%)</td>
        <td style="padding:3px 8px;text-align:right;color:#e65c00">
          −${fmt(totaux.remise_montant, devise)}
        </td>
      </tr>`);
    }
    // TVA seulement si active
    if (hasTVA && totaux.total_tva > 0) {
      rows.push(`<tr>
        <td style="padding:3px 8px;color:#1a56db">TVA</td>
        <td style="padding:3px 8px;text-align:right;color:#1a56db">
          ${fmt(totaux.total_tva, devise)}
        </td>
      </tr>`);
    }
    // Total
    rows.push(`<tr style="border-top:2px solid #000;background:#f5f5f5">
      <td style="padding:7px 8px;font-weight:bold;font-size:13px">
        ${hasTVA && totaux.total_tva > 0 ? "TOTAL TTC" : "TOTAL"}
      </td>
      <td style="padding:7px 8px;text-align:right;font-weight:bold;font-size:14px">
        ${fmt(hasTVA ? totaux.total_ttc : totaux.total_net, devise)}
      </td>
    </tr>`);

    // Acompte et reste du : affiches des qu'il reste quelque chose a
    // payer, meme sans acompte. Le client doit lire sur son document ce
    // qu'il doit encore — c'est la premiere source de litige.
    if (totaux.reste_du > 0 && piece.type_piece === "facture") {
      if (totaux.total_paye > 0) {
        rows.push(`<tr>
          <td style="padding:3px 8px;color:#555">Acompte reçu</td>
          <td style="padding:3px 8px;text-align:right">
            −${fmt(totaux.total_paye, devise)}
          </td>
        </tr>`);
      }
      rows.push(`<tr style="border-top:1px solid #000;background:#fff5f5">
        <td style="padding:7px 8px;font-weight:bold;color:#c00">RESTE DÛ</td>
        <td style="padding:7px 8px;text-align:right;font-weight:bold;color:#c00">
          ${fmt(totaux.reste_du, devise)}
        </td>
      </tr>`);
    }
    return rows.join("");
  }

  function uneFacture(): string {
    return `
    <div class="page">
      ${enteteHtml}
      <div style="display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:14px">
        <div>
          ${avecEntete ? "" : `
          ${logoHtml}
          <div style="font-size:${isA5 ? "14" : "17"}px;font-weight:bold;margin-top:4px">${societe.nom}</div>
          ${societe.adresse ? `<div style="font-size:10px;color:#555">${societe.adresse}</div>` : ""}
          ${societe.telephone ? `<div style="font-size:10px;color:#555">Tél: ${societe.telephone}${societe.telephone2 ? " / " + societe.telephone2 : ""}</div>` : ""}
          ${societe.nif ? `<div style="font-size:10px;color:#555">NIF: ${societe.nif}</div>` : ""}
          ${societe.rccm ? `<div style="font-size:10px;color:#555">RCCM: ${societe.rccm}</div>` : ""}`}
        </div>
        <div style="text-align:right">
          <div style="font-size:15px;font-weight:bold;color:#333">${titre}</div>

          <div style="margin-top:4px">N° <strong>${piece.numero}</strong></div>
          <div style="font-size:10px;color:#555">Date : ${fmtDateHeure(piece.date_piece)}</div>
          ${piece.date_echeance
            ? `<div style="font-size:10px;color:#e65c00">Échéance : ${fmtDate(piece.date_echeance)}</div>`
            : ""}
        </div>
      </div>

      <div style="margin-bottom:12px;padding:8px;border:1px solid #ddd;
                  border-radius:4px;display:inline-block;min-width:210px;
                  align-self:flex-start">
        <div style="font-size:9px;color:#777;text-transform:uppercase;margin-bottom:2px">
          ${piece.tiers_type === "fournisseur" ? "Fournisseur" : "Client"}
        </div>
        <div style="font-weight:bold">${piece.client_nom}</div>
        ${piece.client_code ? `<div style="font-size:10px;color:#555">${piece.client_code}</div>` : ""}
        ${piece.client_telephone ? `<div style="font-size:10px;color:#555">Tél: ${piece.client_telephone}</div>` : ""}
        ${piece.client_adresse ? `<div style="font-size:10px;color:#555">${piece.client_adresse}</div>` : ""}
        ${piece.client_nif ? `<div style="font-size:10px;color:#555">NIF: ${piece.client_nif}</div>` : ""}
      </div>

      <table style="width:100%;border-collapse:collapse;margin:10px 0">
        <thead>
          <tr style="background:#f0f0f0;border-bottom:2px solid #000">
            <th style="text-align:left;padding:5px 6px;font-size:11px">Désignation</th>
            <th style="text-align:center;padding:5px 4px;font-size:11px">Qté</th>
            <th style="text-align:right;padding:5px 4px;font-size:11px">P.U.</th>
            ${hasTVA ? '<th style="text-align:center;padding:5px 4px;font-size:11px">TVA</th>' : ""}
            <th style="text-align:right;padding:5px 4px;font-size:11px">${hasTVA ? "Montant HT" : "Montant"}</th>
          </tr>
        </thead>
        <tbody>${lignesTableau()}</tbody>
      </table>

      <div style="display:flex;justify-content:flex-end;margin-bottom:10px">
        <table style="border-collapse:collapse;min-width:230px">
          ${totauxHTML()}
        </table>
      </div>

      ${piece.note ? `
        <div style="border-top:1px solid #ddd;padding-top:6px;margin-top:6px;
                    font-size:10px;color:#555;font-style:italic">
          ${piece.note}
        </div>` : ""}

      ${piedBase64
        ? `<div class="pied-page" style="padding-top:14px">
             <img src="${piedBase64}" alt="" style="width:100%;height:auto;display:block"/>
             <div style="font-size:9px;color:#aaa;margin-top:2px;text-align:center">
               Imprimé le ${fmtDateHeure(new Date().toISOString())}
             </div>
           </div>`
        : `<div class="pied-page" style="border-top:1px solid #ddd;
                    padding-top:6px;text-align:center">
          <div style="font-size:10px;color:#555">
            ${/* Interpole BRUT : le commercant peut y mettre du HTML
                  (gras, retours a la ligne, petit tableau). Une balise
                  non fermee casse la mise en page — c'est le prix. */
              societe.pied_facture ?? "Merci de votre confiance"}
          </div>
          <div style="font-size:9px;color:#aaa;margin-top:2px">
            Imprimé le ${fmtDateHeure(new Date().toISOString())}
          </div>
        </div>`}
    </div>`;
  }

  const pageSize = isA5 ? "148mm 210mm" : "210mm 297mm";
  const padding = isA5 ? "10mm" : "14mm 14mm 10mm 14mm";

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${titre} ${piece.numero}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:11px; color:#000; }
    .page {
      padding: ${padding};
      min-height: ${isA5 ? "190mm" : "277mm"};
      page-break-after: always;
      /* Colonne flex : le pied se colle en bas via margin-top:auto,
         au lieu de flotter au milieu d'une facture courte. */
      display: flex;
      flex-direction: column;
    }
    .pied-page { margin-top: auto; }
    .page:last-child { page-break-after: avoid; }
    @media print {
      body { margin:0; }
      @page { size: ${pageSize}; margin:0; }
    }
  </style>
</head>
<body>
  ${uneFacture()}
  ${SCRIPT_IMPRESSION}
</body>
</html>`;
}

// =====================================================================
//  Point d'entree unique
// =====================================================================

/**
 * Genere le HTML d'une piece dans le format demande.
 *
 * Remplace l'ancien couple genererFactureHTML (POS) / genererPieceHTML
 * (Pieces) : les deux ecrans lisent desormais lire_donnees_piece, donc
 * une seule mise en page a maintenir. Un client ne recoit plus deux
 * presentations differentes pour le meme document.
 */
export function genererImpression(
  donnees: DonneesPiece,
  format: FormatImpression,
  logoBase64?: string | null,
  // Bandeau d'en-tete, ignore sur thermique : 58 ou 80 mm en noir et
  // blanc ne rendent qu'une tache grise, et le papier a en-tete n'a
  // pas de sens sur un ticket de caisse.
  enteteBase64?: string | null,
  piedBase64?: string | null,
): string {
  switch (format) {
    case "thermique_58": return genererTicketThermique(donnees, logoBase64, 58);
    case "thermique_80": return genererTicketThermique(donnees, logoBase64, 80);
    case "bon_sortie":   return genererBonSortieHTML(donnees, logoBase64, enteteBase64);
    case "a4_et_bon":
      return genererFactureEtBonHTML(donnees, "a4", logoBase64, enteteBase64, piedBase64);
    case "a5_et_bon":
      return genererFactureEtBonHTML(donnees, "a5", logoBase64, enteteBase64, piedBase64);
    case "a5":
      return genererPieceHTML(donnees, logoBase64, "a5", enteteBase64, piedBase64);
    default:
      return genererPieceHTML(donnees, logoBase64, "a4", enteteBase64, piedBase64);
  }
}

// =====================================================================
//  Ticket thermique 58 / 80mm
// =====================================================================

export function genererTicketThermique(
  donnees: DonneesPiece,
  logoBase64?: string | null,
  largeurMm: 58 | 80 = 80,
): string {
  const { piece, lignes, societe, totaux } = donnees;
  const devise = societe.devise ?? "F";
  const titre = TITRES[piece.type_piece] ?? "FACTURE";

  const lignesHTML = lignes.map((l: any) => {
    const montant = new Intl.NumberFormat("fr-ML").format(l.montant_ht);
    const qte = l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2);
    const tva_pct = l.taux_tva ? `(TVA ${(l.taux_tva * 100).toFixed(0)}%)` : "";
    return `
      <div style="margin:3px 0">
        <div style="font-weight:bold">${l.article_nom} ${tva_pct}</div>
        <div style="display:flex;justify-content:space-between">
          <span>${qte} ${l.unite_libelle} × ${new Intl.NumberFormat("fr-ML").format(l.prix_unitaire)}</span>
          <span>${montant} ${devise}</span>
        </div>
        ${l.remise_pct > 0
          ? `<div style="color:#888;font-size:10px">Remise ${l.remise_pct}%</div>`
          : ""}
      </div>
      <div style="border-top:1px dashed #ccc;margin:2px 0"></div>`;
  }).join("");

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${titre} ${piece.numero}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body {
      font-family: 'Courier New', monospace;
      font-size: 11px;
      color: #000;
      width: ${largeurMm === 58 ? "52mm" : "72mm"};
      padding: ${largeurMm === 58 ? "3mm 2mm" : "4mm 3mm"};
    }
    @media print {
      body { margin:0; }
      @page { size: ${largeurMm}mm auto; margin: 2mm; }
    }
  </style>
</head>
<body>
  ${logoBase64
    ? `<div style="text-align:center;margin-bottom:4px">
         <img src="${logoBase64}" style="max-height:40px;max-width:60mm;object-fit:contain"/>
       </div>`
    : ""}
  <div style="text-align:center;font-weight:bold;font-size:13px">${societe.nom}</div>
  ${societe.adresse ? `<div style="text-align:center;font-size:10px">${societe.adresse}</div>` : ""}
  ${societe.telephone ? `<div style="text-align:center;font-size:10px">Tél: ${societe.telephone}</div>` : ""}
  <div style="border-top:1px solid #000;margin:4px 0"></div>
  <div style="text-align:center;font-weight:bold">${titre}</div>
  <div style="display:flex;justify-content:space-between">
    <span>N° ${piece.numero}</span>
    <span>${fmtDateHeure(piece.date_piece)}</span>
  </div>
  <div style="font-size:10px">Client: ${piece.client_nom}</div>
  <div style="border-top:1px dashed #000;margin:4px 0"></div>
  ${lignesHTML}
  <div style="border-top:1px solid #000;margin:4px 0"></div>
  ${totaux.remise_montant > 0 ? `
    <div style="display:flex;justify-content:space-between">
      <span>Sous-total HT</span><span>${fmt(totaux.total_ht, devise)}</span>
    </div>
    <div style="display:flex;justify-content:space-between;color:#888">
      <span>Remise</span><span>−${fmt(totaux.remise_montant, devise)}</span>
    </div>` : ""}
  ${totaux.total_tva > 0 ? `
    <div style="display:flex;justify-content:space-between;color:#1a56db">
      <span>TVA</span><span>${fmt(totaux.total_tva, devise)}</span>
    </div>` : ""}
  <div style="border-top:1px solid #000;margin:4px 0"></div>
  <div style="display:flex;justify-content:space-between;font-weight:bold;font-size:14px">
    <span>${totaux.total_tva > 0 ? "TOTAL TTC" : "TOTAL"}</span>
    <span>${fmt(totaux.total_ttc, devise)}</span>
  </div>
  ${totaux.reste_du > 0 && piece.type_piece === "facture" ? `
    ${totaux.total_paye > 0 ? `
      <div style="display:flex;justify-content:space-between">
        <span>Acompte reçu</span><span>−${fmt(totaux.total_paye, devise)}</span>
      </div>` : ""}
    <div style="border-top:1px solid #000;margin:3px 0"></div>
    <div style="display:flex;justify-content:space-between;font-weight:bold">
      <span>RESTE DÛ</span><span>${fmt(totaux.reste_du, devise)}</span>
    </div>` : ""}
  <div style="border-top:1px dashed #000;margin:4px 0"></div>
  <div style="text-align:center;font-size:10px;margin-top:4px">
    ${societe.pied_facture ?? "Merci de votre confiance !"}
  </div>
  <div style="text-align:center;font-size:9px;color:#888;margin-top:2px">
    ${fmtDate(new Date().toISOString())}
    ${new Date().toLocaleTimeString("fr-ML", { hour: "2-digit", minute: "2-digit" })}
  </div>
  ${SCRIPT_IMPRESSION}
</body>
</html>`;
}
// =====================================================================
//  Bon de sortie — sans aucun montant
// =====================================================================
//
// Cas d'usage : quincaillerie ou depot dont le magasin est separe de la
// caisse. Le client paie au comptoir, repart avec ce bon, et le
// magasinier ne delivre la marchandise que contre ce papier signe.
//
// INVARIANT : aucun prix, aucun total, aucune TVA. Le magasinier n'a
// pas a connaitre les montants, et le client ne doit pas pouvoir
// presenter ce bon comme une facture. Un seul chiffre y figure : la
// quantite.
//
// Le bon porte le NUMERO DE LA FACTURE, pas le sien : c'est ce qui
// permet au magasinier de remonter a la vente. Aucune piece n'est
// creee en base — c'est une vue, pas un document commercial de plus.
//
// Toutes les classes sont prefixees `bs-` et tous les selecteurs sont
// scopes sous `.bs-page` : le bon doit pouvoir cohabiter avec la
// facture DANS LE MEME DOCUMENT sans deteindre sur elle.

const STYLES_BON_SORTIE = `
  .bs-page { padding:12mm; page-break-after:always;
             display:flex; flex-direction:column; min-height:190mm; }
  .bs-page:last-child { page-break-after:avoid; }
  .bs-entete { display:flex; justify-content:space-between;
               align-items:flex-start;
               border-bottom:2px solid #000; padding-bottom:8px; }
  .bs-soc { font-size:15px; font-weight:bold; }
  .bs-soc-det { font-size:10px; color:#555; }
  /* Le titre doit se lire d'un coup d'oeil au magasin, souvent dans
     une lumiere mediocre. */
  .bs-titre { font-size:20px; font-weight:bold; letter-spacing:1px;
              text-align:right; line-height:1.1; }
  .bs-ref { font-size:12px; text-align:right; margin-top:3px; }
  .bs-ref strong { font-size:14px; }
  /* La propriete gap n'est pas honoree par tous les moteurs
     d'impression : marge explicite plutot que d'en dependre. */
  .bs-client { margin:10px 0 8px; padding:6px 8px; border:1px solid #bbb;
               display:flex; align-items:baseline; }
  .bs-client > div { margin-right:28px; }
  .bs-client > div:last-child { margin-right:0; }
  .bs-lbl { font-size:9px; color:#777; text-transform:uppercase; }

  .bs-page table { width:100%; border-collapse:collapse; margin-top:4px; }
  .bs-page th { background:#eee; border-bottom:2px solid #000;
                padding:6px 5px; font-size:11px; text-align:left; }
  .bs-page td { padding:6px 5px; border-bottom:1px solid #ddd; }
  .bs-c-num { width:7%; text-align:center; color:#777; }
  .bs-c-des { width:48%; font-weight:600; }
  /* Quantite en gros : c'est le seul chiffre du document, et celui sur
     lequel une erreur coute de la marchandise. */
  .bs-c-qte { width:13%; text-align:right; font-size:15px; font-weight:bold; }
  .bs-c-uni { width:14%; }
  .bs-c-srv { width:18%; border-left:1px solid #ddd; }

  .bs-recap { margin-top:6px; font-size:11px; color:#555; }
  .bs-pied { margin-top:auto; padding-top:10px; }
  .bs-signatures { display:flex; margin-top:14px; }
  .bs-sig { flex:1; border:1px solid #999; padding:6px 8px 26px;
            margin-right:14px; }
  .bs-sig:last-child { margin-right:0; }
  .bs-mention { margin-top:8px; font-size:10px; color:#666;
                border-top:1px solid #ddd; padding-top:6px;
                text-align:center; }
`;

/** Corps du bon, sans balise html — reutilisable dans un document mixte. */
function corpsBonSortie(
  donnees: DonneesPiece,
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  const { piece, lignes, societe } = donnees;
  const avecEntete = !!enteteBase64;

  const lignesHTML = lignes.map((l: any, i: number) => {
    const qte = l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2);
    return `
      <tr style="background:${i % 2 === 0 ? "#fff" : "#f7f7f7"}">
        <td class="bs-c-num">${i + 1}</td>
        <td class="bs-c-des">${l.article_nom}</td>
        <td class="bs-c-qte">${qte}</td>
        <td class="bs-c-uni">${l.unite_libelle ?? ""}</td>
        <td class="bs-c-srv"></td>
      </tr>`;
  }).join("");

  const nb = lignes.length;
  const totalQte = lignes.reduce(
    (s: number, l: any) => s + (Number(l.quantite) || 0), 0);

  return `
  <div class="bs-page">
    ${avecEntete
      ? `<div style="margin-bottom:10px">
           <img src="${enteteBase64}" alt="" style="width:100%;height:auto;display:block"/>
         </div>`
      : ""}
    <div class="bs-entete">
      <div>
        ${avecEntete ? "" : `
        ${logoBase64
          ? `<img src="${logoBase64}" alt=""
                 style="max-height:42px;max-width:150px;object-fit:contain;display:block"/>`
          : ""}
        <div class="bs-soc">${societe.nom}</div>
        ${societe.adresse ? `<div class="bs-soc-det">${societe.adresse}</div>` : ""}
        ${societe.telephone ? `<div class="bs-soc-det">Tél : ${societe.telephone}</div>` : ""}`}
      </div>
      <div>
        <div class="bs-titre">BON DE SORTIE</div>
        <div class="bs-ref">
          Facture n° <strong>${piece.numero}</strong><br>
          <span style="font-size:11px;color:#555">
            ${fmtDateHeure(piece.date_piece)}
          </span>
        </div>
      </div>
    </div>

    <div class="bs-client">
      <div>
        <div class="bs-lbl">Client</div>
        <div style="font-weight:bold;font-size:13px">${piece.client_nom}</div>
      </div>
      ${piece.client_telephone
        ? `<div><div class="bs-lbl">Téléphone</div>
             <div>${piece.client_telephone}</div></div>`
        : ""}
    </div>

    <table>
      <thead>
        <tr>
          <th class="bs-c-num">#</th>
          <th class="bs-c-des">Désignation</th>
          <th class="bs-c-qte" style="text-align:right">Qté</th>
          <th class="bs-c-uni">Unité</th>
          <th class="bs-c-srv">Servi</th>
        </tr>
      </thead>
      <tbody>${lignesHTML}</tbody>
    </table>

    <div class="bs-recap">
      ${nb} article${nb > 1 ? "s" : ""} ·
      ${totalQte % 1 === 0 ? totalQte : totalQte.toFixed(2)} unité(s) au total
    </div>

    ${piece.note
      ? `<div style="margin-top:8px;font-size:11px;font-style:italic;color:#555">
           ${piece.note}</div>`
      : ""}

    <div class="bs-pied">
      <div class="bs-signatures">
        <div class="bs-sig"><span class="bs-lbl">Le magasinier (nom et signature)</span></div>
        <div class="bs-sig"><span class="bs-lbl">Le client (reçu la marchandise)</span></div>
      </div>
      <div class="bs-mention">
        Ce bon ne vaut pas facture et ne porte aucun montant.
        À remettre au magasinier contre la marchandise.
      </div>
    </div>
  </div>`;
}

/** Bon de sortie seul, en A5. */
export function genererBonSortieHTML(
  donnees: DonneesPiece,
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>Bon de sortie ${donnees.piece.numero}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:12px; color:#000; }
    ${STYLES_BON_SORTIE}
    @media print { body { margin:0; } @page { size:148mm 210mm; margin:0; } }
  </style>
</head>
<body>
  ${corpsBonSortie(donnees, logoBase64, enteteBase64)}
  ${SCRIPT_IMPRESSION}
</body>
</html>`;
}

/**
 * Facture ET bon de sortie dans UN SEUL document, deux pages.
 *
 * Pourquoi pas deux impressions enchainees : `imprimer_facture` ferme
 * toute fenetre `impression_*` avant d'ouvrir la sienne. La seconde
 * impression tuerait donc la boite de dialogue de la premiere avant que
 * le caissier ait clique. Un document, une boite de dialogue, deux
 * pages — et le bon sort a la suite sans qu'on ait rien a enchainer.
 *
 * Les deux pages prennent le format de la facture : un bon rendu sur A4
 * a plus de blanc, mais reste parfaitement lisible.
 */
export function genererFactureEtBonHTML(
  donnees: DonneesPiece,
  format: "a4" | "a5" = "a4",
  logoBase64?: string | null,
  enteteBase64?: string | null,
  piedBase64?: string | null,
): string {
  const facture = genererPieceHTML(
    donnees, logoBase64, format, enteteBase64, piedBase64);

  // On reprend le document facture et on lui greffe le bon : styles
  // dans le <head>, corps avant le script d'impression.
  return facture
    .replace("</style>", STYLES_BON_SORTIE + "\n  </style>")
    .replace(
      SCRIPT_IMPRESSION,
      corpsBonSortie(donnees, logoBase64, enteteBase64) + "\n  " + SCRIPT_IMPRESSION,
    );
}

// =====================================================================
//  Bon d'échange — A5, SANS AUCUN MONTANT
// =====================================================================

export interface DonneesEchange {
  numero_vente: string;
  date: string;
  client_nom: string;
  client_telephone?: string | null;
  rendu: { article_nom: string; unite_libelle: string; quantite: number };
  remis: { article_nom: string; unite_libelle: string; quantite: number };
  societe: any;
  note?: string | null;
}

/**
 * Bon d'echange magasin.
 *
 * L'echange est le SEUL cas ou le magasinier doit faire deux gestes
 * opposes : reprendre une chose, en donner une autre. Sans papier il
 * n'a aucune instruction, et c'est la que naissent les ecarts de stock.
 *
 * Comme le bon de sortie : aucun montant. Le complement ou le reliquat
 * se regle a la caisse, pas au magasin — le magasinier n'a pas a savoir
 * qui doit quoi a qui.
 *
 * Les deux blocs sont volontairement DISSEMBLABLES : fleche vers le bas
 * pour ce qui rentre, vers le haut pour ce qui sort. Deux tableaux
 * identiques se confondent quand on travaille vite.
 */
export function genererBonEchangeHTML(
  d: DonneesEchange,
  logoBase64?: string | null,
): string {
  const q = (n: number) => (n % 1 === 0 ? n : n.toFixed(2));

  const bloc = (
    titre: string, sens: string, couleur: string,
    a: { article_nom: string; unite_libelle: string; quantite: number },
  ) => `
    <div class="bloc" style="border-left:5px solid ${couleur}">
      <div class="bloc-t" style="color:${couleur}">${sens} ${titre}</div>
      <div class="bloc-a">${a.article_nom}</div>
      <div class="bloc-q">${q(a.quantite)} <span>${a.unite_libelle}</span></div>
    </div>`;

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>Bon d'échange ${d.numero_vente}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:12px; color:#000; }
    .page { padding:12mm; min-height:190mm; display:flex; flex-direction:column; }
    .entete { display:flex; justify-content:space-between;
              align-items:flex-start; border-bottom:2px solid #000;
              padding-bottom:8px; }
    .soc { font-size:15px; font-weight:bold; }
    .soc-det { font-size:10px; color:#555; }
    .titre { font-size:20px; font-weight:bold; letter-spacing:1px;
             text-align:right; line-height:1.1; }
    .ref { font-size:12px; text-align:right; margin-top:3px; }
    .client { margin:10px 0; padding:6px 8px; border:1px solid #bbb; }
    .lbl { font-size:9px; color:#777; text-transform:uppercase; }

    .bloc { padding:10px 12px; margin:10px 0; background:#f7f7f7; }
    .bloc-t { font-size:11px; font-weight:bold; text-transform:uppercase;
              letter-spacing:0.5px; margin-bottom:4px; }
    .bloc-a { font-size:15px; font-weight:bold; }
    /* La quantite est le seul chiffre du document. */
    .bloc-q { font-size:22px; font-weight:bold; margin-top:2px; }
    .bloc-q span { font-size:13px; font-weight:normal; color:#555; }

    .pied { margin-top:auto; padding-top:10px; }
    .signatures { display:flex; margin-top:14px; }
    .sig { flex:1; border:1px solid #999; padding:6px 8px 26px;
           margin-right:14px; }
    .sig:last-child { margin-right:0; }
    .sig .lbl { font-size:10px; color:#666; }
    .mention { margin-top:8px; font-size:10px; color:#666;
               border-top:1px solid #ddd; padding-top:6px; text-align:center; }
    @media print { body { margin:0; } @page { size:148mm 210mm; margin:0; } }
  </style>
</head>
<body>
  <div class="page">

    <div class="entete">
      <div>
        ${logoBase64
          ? `<img src="${logoBase64}" alt=""
                 style="max-height:42px;max-width:150px;object-fit:contain;display:block"/>`
          : ""}
        <div class="soc">${d.societe.nom}</div>
        ${d.societe.adresse ? `<div class="soc-det">${d.societe.adresse}</div>` : ""}
        ${d.societe.telephone ? `<div class="soc-det">Tél : ${d.societe.telephone}</div>` : ""}
      </div>
      <div>
        <div class="titre">BON D'ÉCHANGE</div>
        <div class="ref">
          Vente n° <strong>${d.numero_vente}</strong><br>
          <span style="font-size:11px;color:#555">${fmtDateHeure(d.date)}</span>
        </div>
      </div>
    </div>

    <div class="client">
      <div class="lbl">Client</div>
      <div style="font-weight:bold;font-size:13px">${d.client_nom}</div>
      ${d.client_telephone
        ? `<div style="font-size:11px;color:#555">${d.client_telephone}</div>` : ""}
    </div>

    ${bloc("le client rapporte", "&#8681;", "#b45309", d.rendu)}
    ${bloc("à lui remettre", "&#8679;", "#15803d", d.remis)}

    ${d.note
      ? `<div style="margin-top:6px;font-size:11px;font-style:italic;color:#555">
           ${d.note}</div>`
      : ""}

    <div class="pied">
      <div class="signatures">
        <div class="sig"><span class="lbl">Le magasinier (nom et signature)</span></div>
        <div class="sig"><span class="lbl">Le client (échange effectué)</span></div>
      </div>
      <div class="mention">
        Ce bon ne porte aucun montant. Toute différence de prix se règle
        à la caisse, pas au magasin.
      </div>
    </div>

  </div>
  ${SCRIPT_IMPRESSION}
</body>
</html>`;
}
