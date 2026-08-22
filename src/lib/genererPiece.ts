// lib/genererPiece.ts — Templates HTML pour impression pièces commerciales
// Supporte format A4 (défaut) et thermique 80mm

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

function fmt(n: number, devise = "FCFA"): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " " + devise;
}
function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Format A4 — standard
// =====================================================================

export function genererPieceHTML(
  donnees: any,
  logoBase64?: string | null,
): string {
  const { piece, lignes, societe, totaux } = donnees;
  const devise = societe.devise ?? "FCFA";
  const titre = TITRES[piece.type_piece] ?? "PIÈCE COMMERCIALE";

  const logoHtml = logoBase64
    ? `<div style="text-align:center;margin-bottom:8px">
         <img src="${logoBase64}" alt="Logo"
              style="max-height:60px;max-width:200px;object-fit:contain"/>
       </div>`
    : "";

  const lignesHTML = lignes.map((l: any, i: number) => `
    <tr style="background:${i % 2 === 0 ? "#fff" : "#f9f9f9"};border-bottom:1px solid #eee">
      <td style="padding:5px 6px">${l.article_nom}
        ${l.remise_pct > 0
          ? `<br><span style="font-size:10px;color:#888">Remise ${l.remise_pct}%</span>`
          : ""}
      </td>
      <td style="text-align:center;padding:5px 4px">
        ${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)} ${l.unite_libelle}
      </td>
      <td style="text-align:right;padding:5px 4px">${fmt(l.prix_unitaire, devise)}</td>
      <td style="text-align:right;padding:5px 4px;font-weight:bold">
        ${fmt(l.montant_ht, devise)}
      </td>
    </tr>`).join("");

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${titre} ${piece.numero}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:12px; color:#000; padding:15mm; }
    @media print { body { margin:0; } @page { size:A4; margin:10mm; } }
  </style>
</head>
<body>
  ${logoHtml}
  <div style="display:flex;justify-content:space-between;margin-bottom:16px">
    <div>
      <div style="font-size:18px;font-weight:bold">${societe.nom}</div>
      ${societe.adresse ? `<div style="font-size:11px;color:#555">${societe.adresse}</div>` : ""}
      ${societe.telephone ? `<div style="font-size:11px;color:#555">Tél: ${societe.telephone}${societe.telephone2 ? " / " + societe.telephone2 : ""}</div>` : ""}
      ${societe.nif ? `<div style="font-size:11px;color:#555">NIF: ${societe.nif}</div>` : ""}
      ${societe.rccm ? `<div style="font-size:11px;color:#555">RCCM: ${societe.rccm}</div>` : ""}
    </div>
    <div style="text-align:right">
      <div style="font-size:16px;font-weight:bold;color:#333">${titre}</div>
      <div>N° <strong>${piece.numero}</strong></div>
      <div style="font-size:11px;color:#555">Date : ${fmtDate(piece.date_piece)}</div>
      ${piece.date_echeance ? `<div style="font-size:11px;color:#e65c00">Échéance : ${fmtDate(piece.date_echeance)}</div>` : ""}
    </div>
  </div>

  <div style="margin-bottom:12px;padding:8px;border:1px solid #ddd;
              border-radius:4px;display:inline-block;min-width:220px">
    <div style="font-size:10px;color:#777;margin-bottom:2px;text-transform:uppercase">
      ${piece.tiers_type === "fournisseur" ? "FOURNISSEUR" : "CLIENT"}
    </div>
    <div style="font-weight:bold">${piece.client_nom}</div>
    <div style="font-size:11px;color:#555">${piece.client_code ?? ""}</div>
    ${piece.client_telephone ? `<div style="font-size:11px;color:#555">Tél: ${piece.client_telephone}</div>` : ""}
    ${piece.client_adresse ? `<div style="font-size:11px;color:#555">${piece.client_adresse}</div>` : ""}
    ${piece.client_nif ? `<div style="font-size:11px;color:#555">NIF: ${piece.client_nif}</div>` : ""}
  </div>

  <table style="width:100%;border-collapse:collapse;margin:12px 0">
    <thead>
      <tr style="background:#f0f0f0;border-bottom:2px solid #000">
        <th style="text-align:left;padding:6px 6px">Désignation</th>
        <th style="text-align:center;padding:6px 4px">Qté</th>
        <th style="text-align:right;padding:6px 4px">P.U. (${devise})</th>
        <th style="text-align:right;padding:6px 4px">Montant (${devise})</th>
      </tr>
    </thead>
    <tbody>${lignesHTML}</tbody>
  </table>

  <div style="display:flex;justify-content:flex-end;margin-bottom:12px">
    <table style="border-collapse:collapse;min-width:240px">
      ${totaux.total_tva > 0 ? `
        <tr>
          <td style="padding:4px 8px;color:#555">Total HT</td>
          <td style="padding:4px 8px;text-align:right">${fmt(totaux.total_ht, devise)}</td>
        </tr>` : ""}
      ${totaux.remise_montant > 0 ? `
        <tr>
          <td style="padding:4px 8px;color:#e65c00">
            Remise (${piece.remise_globale}%)
          </td>
          <td style="padding:4px 8px;text-align:right;color:#e65c00">
            −${fmt(totaux.remise_montant, devise)}
          </td>
        </tr>` : ""}
      ${totaux.total_tva > 0 ? `
        <tr>
          <td style="padding:4px 8px;color:#555">TVA</td>
          <td style="padding:4px 8px;text-align:right">${fmt(totaux.total_tva, devise)}</td>
        </tr>` : ""}
      <tr style="border-top:2px solid #000;background:#f5f5f5">
        <td style="padding:7px 8px;font-weight:bold;font-size:13px">
          ${totaux.total_tva > 0 ? "TOTAL TTC" : "TOTAL"}
        </td>
        <td style="padding:7px 8px;text-align:right;font-weight:bold;font-size:14px">
          ${fmt(totaux.total_ttc, devise)}
        </td>
      </tr>
    </table>
  </div>

  ${piece.note ? `
    <div style="border-top:1px solid #ddd;padding-top:8px;margin-top:8px;
                font-size:11px;color:#555;font-style:italic">
      ${piece.note}
    </div>` : ""}

  <div style="border-top:1px solid #ddd;margin-top:16px;padding-top:8px;
              text-align:center">
    <span style="font-size:11px;color:#555">
      ${societe.pied_facture ?? "Merci de votre confiance"}
    </span>
    <br>
    <span style="font-size:10px;color:#aaa">
      Imprimé le ${fmtDate(new Date().toISOString())}
    </span>
  </div>

  <script>window.onload = () => { window.focus(); window.print(); }</script>
</body>
</html>`;
}

// =====================================================================
//  Format thermique 80mm — ticket caisse
// =====================================================================

export function genererTicketThermique(
  donnees: any,
  logoBase64?: string | null,
): string {
  const { piece, lignes, societe, totaux } = donnees;
  const devise = societe.devise ?? "F";
  const titre = TITRES[piece.type_piece] ?? "FACTURE";

  function ligne80(gauche: string, droite: string, largeur = 42): string {
    const espaces = Math.max(1, largeur - gauche.length - droite.length);
    return gauche + " ".repeat(espaces) + droite;
  }

  const lignesHTML = lignes.map((l: any) => {
    const montant = new Intl.NumberFormat("fr-ML").format(l.montant_ht);
    const qte = l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2);
    return `
      <div style="margin:3px 0">
        <div style="font-weight:bold">${l.article_nom}</div>
        <div style="display:flex;justify-content:space-between">
          <span>${qte} ${l.unite_libelle} × ${new Intl.NumberFormat("fr-ML").format(l.prix_unitaire)}</span>
          <span>${montant} ${devise}</span>
        </div>
        ${l.remise_pct > 0
          ? `<div style="color:#888;font-size:10px">Remise ${l.remise_pct}% −${new Intl.NumberFormat("fr-ML").format(l.remise_montant)} ${devise}</div>`
          : ""}
      </div>`;
  }).join('<div style="border-top:1px dashed #ccc;margin:2px 0"></div>');

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
      width: 72mm;
      padding: 4mm 3mm;
    }
    @media print {
      body { margin:0; }
      @page { size: 80mm auto; margin: 2mm; }
    }
    .centre { text-align:center; }
    .bold { font-weight:bold; }
    .sep { border-top:1px dashed #000; margin:4px 0; }
    .sep-solid { border-top:1px solid #000; margin:4px 0; }
    .total-ligne { display:flex; justify-content:space-between; }
    .grand-total { font-size:14px; font-weight:bold; }
  </style>
</head>
<body>
  ${logoBase64 ? `
    <div class="centre" style="margin-bottom:4px">
      <img src="${logoBase64}" alt="Logo"
           style="max-height:40px;max-width:60mm;object-fit:contain"/>
    </div>` : ""}

  <div class="centre bold" style="font-size:13px">${societe.nom}</div>
  ${societe.adresse ? `<div class="centre" style="font-size:10px">${societe.adresse}</div>` : ""}
  ${societe.telephone ? `<div class="centre" style="font-size:10px">Tél: ${societe.telephone}</div>` : ""}

  <div class="sep-solid"></div>

  <div class="centre bold">${titre}</div>
  <div class="total-ligne">
    <span>N° ${piece.numero}</span>
    <span>${fmtDate(piece.date_piece)}</span>
  </div>
  <div style="font-size:10px">Client: ${piece.client_nom}</div>

  <div class="sep"></div>

  ${lignesHTML}

  <div class="sep-solid"></div>

  ${totaux.remise_montant > 0 ? `
    <div class="total-ligne">
      <span>Sous-total</span>
      <span>${fmt(totaux.total_ht, devise)}</span>
    </div>
    <div class="total-ligne" style="color:#888">
      <span>Remise</span>
      <span>−${fmt(totaux.remise_montant, devise)}</span>
    </div>` : ""}

  ${totaux.total_tva > 0 ? `
    <div class="total-ligne">
      <span>TVA</span>
      <span>${fmt(totaux.total_tva, devise)}</span>
    </div>` : ""}

  <div class="sep-solid"></div>

  <div class="total-ligne grand-total">
    <span>TOTAL</span>
    <span>${fmt(totaux.total_ttc, devise)}</span>
  </div>

  <div class="sep"></div>

  <div class="centre" style="font-size:10px;margin-top:4px">
    ${societe.pied_facture ?? "Merci de votre confiance !"}
  </div>
  <div class="centre" style="font-size:9px;color:#888;margin-top:2px">
    ${fmtDate(new Date().toISOString())} ${new Date().toLocaleTimeString("fr-ML", { hour: "2-digit", minute: "2-digit" })}
  </div>

  <script>window.onload = () => { window.focus(); window.print(); }</script>
</body>
</html>`;
}