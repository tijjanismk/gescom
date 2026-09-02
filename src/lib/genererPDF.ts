// lib/genererPDF.ts — Génération PDF via impression HTML → PDF navigateur
// Pas de dépendance externe — utilise window.print() avec CSS @page

/** Formats d'impression. Un seul generateur pour toutes les pieces. */
export type FormatImpression = "a4" | "a5" | "thermique_58" | "thermique_80";

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
): string {
  const { piece, lignes, societe, totaux } = donnees;
  const devise = societe.devise ?? "FCFA";
  const titre = TITRES[piece.type_piece] ?? "PIÈCE COMMERCIALE";
  const isA5 = format === "a5";

  const logoHtml = logoBase64
    ? `<img src="${logoBase64}" alt="Logo"
           style="max-height:55px;max-width:180px;object-fit:contain;display:block"/>`
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
      <div style="display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:14px">
        <div>
          ${logoHtml}
          <div style="font-size:${isA5 ? "14" : "17"}px;font-weight:bold;margin-top:4px">${societe.nom}</div>
          ${societe.adresse ? `<div style="font-size:10px;color:#555">${societe.adresse}</div>` : ""}
          ${societe.telephone ? `<div style="font-size:10px;color:#555">Tél: ${societe.telephone}${societe.telephone2 ? " / " + societe.telephone2 : ""}</div>` : ""}
          ${societe.nif ? `<div style="font-size:10px;color:#555">NIF: ${societe.nif}</div>` : ""}
          ${societe.rccm ? `<div style="font-size:10px;color:#555">RCCM: ${societe.rccm}</div>` : ""}
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
                  border-radius:4px;display:inline-block;min-width:210px">
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

      <div style="border-top:1px solid #ddd;margin-top:14px;padding-top:6px;
                  text-align:center">
        <div style="font-size:10px;color:#555">
          ${societe.pied_facture ?? "Merci de votre confiance"}
        </div>
        <div style="font-size:9px;color:#aaa;margin-top:2px">
          Imprimé le ${fmtDateHeure(new Date().toISOString())}
        </div>
      </div>
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
    }
    .page:last-child { page-break-after: avoid; }
    @media print {
      body { margin:0; }
      @page { size: ${pageSize}; margin:0; }
    }
  </style>
</head>
<body>
  ${uneFacture()}
  <script>window.onload = () => { window.focus(); window.print(); }</script>
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
): string {
  switch (format) {
    case "thermique_58": return genererTicketThermique(donnees, logoBase64, 58);
    case "thermique_80": return genererTicketThermique(donnees, logoBase64, 80);
    case "a5":           return genererPieceHTML(donnees, logoBase64, "a5");
    default:             return genererPieceHTML(donnees, logoBase64, "a4");
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
  <script>window.onload = () => { window.focus(); window.print(); }</script>
</body>
</html>`;
}