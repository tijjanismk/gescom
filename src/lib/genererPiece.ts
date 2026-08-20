// lib/genererPiece.ts — Génère le HTML d'une pièce commerciale pour impression

const TITRES: Record<string, string> = {
  devis:           "DEVIS",
  proforma:        "FACTURE PROFORMA",
  commande_client: "BON DE COMMANDE",
  bon_livraison:   "BON DE LIVRAISON",
  facture:         "FACTURE",
  facture_acompte: "FACTURE D'ACOMPTE",
  avoir_client:    "AVOIR",
};

function fmt(n: number, devise = "FCFA"): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " " + devise;
}
function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

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

  const lignesHTML = lignes.map((l: any) => {
    const montantBrut = Math.round(l.prix_unitaire * l.quantite);
    return `
      <tr style="border-bottom:1px solid #eee">
        <td style="padding:6px 4px">${l.article_nom}
          ${l.remise_pct > 0
            ? `<br><span style="font-size:10px;color:#888">
               Remise ${l.remise_pct}% (−${fmt(l.remise_montant, "")} ${devise})
               </span>`
            : ""}
        </td>
        <td style="text-align:center;padding:6px 4px">
          ${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)} ${l.unite_libelle}
        </td>
        <td style="text-align:right;padding:6px 4px">${fmt(l.prix_unitaire, devise)}</td>
        <td style="text-align:right;padding:6px 4px;font-weight:bold">
          ${fmt(l.montant_ht, devise)}
        </td>
      </tr>`;
  }).join("");

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${titre} ${piece.numero}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:12px; color:#000; padding:15mm; }
    @media print {
      body { margin:0; }
      @page { size:A4; margin:10mm; }
    }
  </style>
</head>
<body>

  <!-- En-tête société -->
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
      <div>N° ${piece.numero}</div>
      <div style="font-size:11px;color:#555">Date : ${fmtDate(piece.date_piece)}</div>
      ${piece.date_echeance ? `<div style="font-size:11px;color:#555">Échéance : ${fmtDate(piece.date_echeance)}</div>` : ""}
    </div>
  </div>

  <!-- Client -->
  <div style="margin-bottom:12px;padding:8px;border:1px solid #ddd;border-radius:4px;display:inline-block;min-width:200px">
    <div style="font-size:11px;color:#777;margin-bottom:2px">CLIENT</div>
    <div style="font-weight:bold">${piece.client_nom}</div>
    <div style="font-size:11px;color:#555">${piece.client_code}</div>
    ${piece.client_telephone ? `<div style="font-size:11px;color:#555">Tél: ${piece.client_telephone}</div>` : ""}
    ${piece.client_adresse ? `<div style="font-size:11px;color:#555">${piece.client_adresse}</div>` : ""}
    ${piece.client_nif ? `<div style="font-size:11px;color:#555">NIF: ${piece.client_nif}</div>` : ""}
  </div>

  <!-- Tableau articles -->
  <table style="width:100%;border-collapse:collapse;margin:12px 0">
    <thead>
      <tr style="background:#f0f0f0;border-bottom:2px solid #000">
        <th style="text-align:left;padding:6px 4px">Désignation</th>
        <th style="text-align:center;padding:6px 4px">Quantité</th>
        <th style="text-align:right;padding:6px 4px">P.U. (${devise})</th>
        <th style="text-align:right;padding:6px 4px">Montant (${devise})</th>
      </tr>
    </thead>
    <tbody>${lignesHTML}</tbody>
  </table>

  <!-- Totaux -->
  <div style="display:flex;justify-content:flex-end;margin-bottom:12px">
    <table style="border-collapse:collapse;min-width:220px">
      ${totaux.total_tva > 0 ? `
        <tr>
          <td style="padding:4px 8px">Total HT</td>
          <td style="padding:4px 8px;text-align:right">${fmt(totaux.total_ht, devise)}</td>
        </tr>` : ""}
      ${totaux.remise_montant > 0 ? `
        <tr>
          <td style="padding:4px 8px;color:#e65c00">
            Remise ${piece.remise_globale}%
          </td>
          <td style="padding:4px 8px;text-align:right;color:#e65c00">
            − ${fmt(totaux.remise_montant, devise)}
          </td>
        </tr>` : ""}
      ${totaux.total_tva > 0 ? `
        <tr>
          <td style="padding:4px 8px">TVA</td>
          <td style="padding:4px 8px;text-align:right">${fmt(totaux.total_tva, devise)}</td>
        </tr>` : ""}
      <tr style="border-top:2px solid #000;background:#f0f0f0">
        <td style="padding:6px 8px;font-weight:bold">
          ${totaux.total_tva > 0 ? "TOTAL TTC" : "TOTAL"}
        </td>
        <td style="padding:6px 8px;text-align:right;font-weight:bold;font-size:14px">
          ${fmt(totaux.total_ttc, devise)}
        </td>
      </tr>
    </table>
  </div>

  <!-- Note -->
  ${piece.note ? `
    <div style="border-top:1px solid #ddd;padding-top:8px;margin-top:8px">
      <span style="font-size:11px;color:#555">${piece.note}</span>
    </div>` : ""}

  <!-- Pied -->
  <div style="border-top:1px solid #ddd;margin-top:16px;padding-top:8px;text-align:center">
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
