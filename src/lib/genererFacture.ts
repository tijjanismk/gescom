// Générateur de facture HTML — supporte A4 et thermique (58mm/80mm)
// Le logo est passé en base64 directement dans le HTML.

interface Societe {
  nom: string;
  adresse?: string;
  telephone?: string;
  telephone2?: string;
  email?: string;
  nif?: string;
  rccm?: string;
  pied_facture?: string;
  devise: string;
  logo_base64?: string; // data:image/png;base64,...
}

interface LigneFacture {
  article_nom: string;
  unite_libelle: string;
  quantite: number;
  prix_pratique: number;
  prix_reference: number;
  montant: number;
}

interface Paiement {
  montant: number;
  mode: string;
  date_paiement: string;
}

interface Vente {
  date_vente: string;
  statut: string;
  mode_reglement: string;
  client_nom: string;
  client_code: string;
  client_telephone?: string;
  client_adresse?: string;
  client_nif?: string;
  numero_facture?: string;
}

interface DonneesFacture {
  societe: Societe;
  vente: Vente;
  lignes: LigneFacture[];
  paiements: Paiement[];
  total: number;
  total_paye: number;
  reste: number;
  logo_base64?: string | null;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n);
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit",
  });
}

function fmtMode(mode: string): string {
  return {
    especes: "Espèces", orange_money: "Orange Money",
    moov_money: "Moov Money", cheque: "Chèque", avoir: "Avoir",
  }[mode] ?? mode;
}

export function genererFactureHTML(
  donnees: DonneesFacture,
  format: "a4" | "thermique_58" | "thermique_80" = "a4",
  logoBase64?: string | null,
): string {
  const { societe, vente, lignes, paiements, total, total_paye, reste } = donnees;
  const devise = societe.devise ?? "FCFA";
  const logo = logoBase64 ?? societe.logo_base64 ?? null;

  const isThermique = format !== "a4";
  const largeur = format === "a4" ? "210mm"
    : format === "thermique_58" ? "58mm" : "80mm";

  const cssCommun = `
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: ${isThermique ? "'Courier New', monospace" : "Arial, sans-serif"};
      font-size: ${isThermique ? "11px" : "12px"};
      color: #000;
      width: ${largeur};
      ${isThermique ? "padding: 2mm;" : "padding: 15mm;"}
    }
    .centre { text-align: center; }
    .droite { text-align: right; }
    .bold { font-weight: bold; }
    .sep { border-top: 1px ${isThermique ? "dashed" : "solid"} #000; margin: 6px 0; }
    .sep2 { border-top: 2px solid #000; margin: 6px 0; }
    .row { display: flex; justify-content: space-between; }
    .petit { font-size: ${isThermique ? "10px" : "11px"}; color: #555; }
    .logo { max-height: ${isThermique ? "40px" : "60px"}; max-width: 100%; object-fit: contain; }
    @media print {
      body { margin: 0; }
      @page {
        size: ${format === "a4" ? "A4" : largeur + " auto"};
        margin: ${isThermique ? "2mm" : "10mm"};
      }
    }
  `;

  // ---- Logo ----
  const logoHtml = logo
    ? `<div class="centre" style="margin-bottom: 6px;">
         <img src="${logo}" alt="Logo" class="logo" />
       </div>`
    : "";

  // ---- En-tête société ----
  const enteteSociete = `
    ${logoHtml}
    <div class="centre" style="margin-bottom: 8px;">
      <div class="bold" style="font-size: ${isThermique ? "13px" : "18px"};">
        ${societe.nom}
      </div>
      ${societe.adresse ? `<div class="petit">${societe.adresse}</div>` : ""}
      ${societe.telephone
        ? `<div class="petit">Tél: ${societe.telephone}${societe.telephone2 ? " / " + societe.telephone2 : ""}</div>`
        : ""}
      ${societe.email ? `<div class="petit">${societe.email}</div>` : ""}
      ${societe.nif ? `<div class="petit">NIF: ${societe.nif}</div>` : ""}
      ${societe.rccm ? `<div class="petit">RCCM: ${societe.rccm}</div>` : ""}
    </div>
  `;

  // ---- Titre ----
  const titreFacture = `
    <div class="sep2"></div>
    <div class="centre bold" style="font-size: ${isThermique ? "12px" : "14px"}; margin: 6px 0;">
      ${vente.statut === "payee" ? "FACTURE" : "FACTURE PROFORMA"}
      ${vente.numero_facture ? ` N° ${vente.numero_facture}` : ""}
    </div>
    <div class="sep2"></div>
  `;

  // ---- Infos vente ----
  const infosVente = isThermique ? `
    <div style="margin-bottom: 6px;">
      <div class="row"><span class="petit">Date:</span>
        <span class="petit">${fmtDate(vente.date_vente)}</span></div>
      <div class="row"><span class="petit">Client:</span>
        <span class="petit bold">${vente.client_nom}</span></div>
      ${vente.client_telephone
        ? `<div class="row"><span class="petit">Tél:</span>
           <span class="petit">${vente.client_telephone}</span></div>`
        : ""}
    </div>
  ` : `
    <div style="display:flex; justify-content:space-between; margin-bottom:12px; gap:20px;">
      <div>
        <div class="bold" style="margin-bottom:4px;">CLIENT</div>
        <div>${vente.client_nom}</div>
        <div class="petit">${vente.client_code}</div>
        ${vente.client_telephone ? `<div class="petit">Tél: ${vente.client_telephone}</div>` : ""}
        ${vente.client_adresse ? `<div class="petit">${vente.client_adresse}</div>` : ""}
        ${vente.client_nif ? `<div class="petit">NIF: ${vente.client_nif}</div>` : ""}
      </div>
      <div class="droite">
        <div class="bold" style="margin-bottom:4px;">FACTURE</div>
        ${vente.numero_facture ? `<div>N° ${vente.numero_facture}</div>` : ""}
        <div class="petit">Date: ${fmtDate(vente.date_vente)}</div>
        <div class="petit">Mode: ${vente.mode_reglement === "comptant" ? "Comptant" : "Crédit"}</div>
      </div>
    </div>
  `;

  // ---- Articles ----
  const tableauArticles = isThermique ? `
    <div class="sep"></div>
    <div class="row bold petit">
      <span style="flex:3">Article</span>
      <span style="flex:1;text-align:right">Qté</span>
      <span style="flex:2;text-align:right">P.U.</span>
      <span style="flex:2;text-align:right">Total</span>
    </div>
    <div class="sep"></div>
    ${lignes.map(l => `
      <div style="margin-bottom:3px;">
        <div class="bold" style="font-size:11px;">${l.article_nom}</div>
        <div class="row petit">
          <span style="flex:3"></span>
          <span style="flex:1;text-align:right">${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)} ${l.unite_libelle}</span>
          <span style="flex:2;text-align:right">${fmt(l.prix_pratique)}</span>
          <span style="flex:2;text-align:right" class="bold">${fmt(l.montant)}</span>
        </div>
        ${l.prix_pratique < l.prix_reference ? `<div class="petit" style="color:#888">Remise appliquée</div>` : ""}
      </div>
    `).join("")}
  ` : `
    <table style="width:100%;border-collapse:collapse;margin-bottom:12px;">
      <thead>
        <tr style="background:#f0f0f0;border-bottom:2px solid #000;">
          <th style="text-align:left;padding:6px 4px;">Désignation</th>
          <th style="text-align:center;padding:6px 4px;">Qté</th>
          <th style="text-align:right;padding:6px 4px;">P.U. (${devise})</th>
          <th style="text-align:right;padding:6px 4px;">Montant (${devise})</th>
        </tr>
      </thead>
      <tbody>
        ${lignes.map((l, i) => `
          <tr style="border-bottom:1px solid #ddd;background:${i % 2 === 0 ? "#fff" : "#fafafa"};">
            <td style="padding:6px 4px;">
              ${l.article_nom}
              ${l.prix_pratique < l.prix_reference
                ? `<br><span style="font-size:10px;color:#888;">Remise: -${fmt(l.prix_reference - l.prix_pratique)} ${devise}/${l.unite_libelle}</span>`
                : ""}
            </td>
            <td style="text-align:center;padding:6px 4px;">
              ${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)} ${l.unite_libelle}
            </td>
            <td style="text-align:right;padding:6px 4px;">${fmt(l.prix_pratique)}</td>
            <td style="text-align:right;padding:6px 4px;font-weight:bold;">${fmt(l.montant)}</td>
          </tr>
        `).join("")}
      </tbody>
    </table>
  `;

  // ---- Totaux ----
  const totaux = isThermique ? `
    <div class="sep"></div>
    <div class="row bold">
      <span>TOTAL</span>
      <span>${fmt(total)} ${devise}</span>
    </div>
    ${total_paye > 0 && total_paye < total ? `
      <div class="row"><span class="petit">Acompte</span>
        <span class="petit">${fmt(total_paye)} ${devise}</span></div>
      <div class="row bold">
        <span>RESTE DÛ</span><span>${fmt(reste)} ${devise}</span>
      </div>
    ` : ""}
    ${vente.statut === "payee" ? `
      <div class="row"><span class="petit">Règlement</span>
        <span class="petit">${paiements.map(p => fmtMode(p.mode)).join(", ")}</span></div>
    ` : ""}
  ` : `
    <div style="display:flex;justify-content:flex-end;margin-bottom:12px;">
      <table style="border-collapse:collapse;min-width:220px;">
        <tr>
          <td style="padding:4px 8px;">Total HT</td>
          <td style="padding:4px 8px;text-align:right;font-weight:bold;">
            ${fmt(total)} ${devise}
          </td>
        </tr>
        ${total_paye > 0 && total_paye < total ? `
          <tr>
            <td style="padding:4px 8px;">Acompte reçu</td>
            <td style="padding:4px 8px;text-align:right;">${fmt(total_paye)} ${devise}</td>
          </tr>
          <tr style="border-top:2px solid #000;background:#f0f0f0;">
            <td style="padding:4px 8px;font-weight:bold;">Reste dû</td>
            <td style="padding:4px 8px;text-align:right;font-weight:bold;">${fmt(reste)} ${devise}</td>
          </tr>
        ` : `
          <tr style="border-top:2px solid #000;background:#f0f0f0;">
            <td style="padding:4px 8px;font-weight:bold;">TOTAL TTC</td>
            <td style="padding:4px 8px;text-align:right;font-weight:bold;">${fmt(total)} ${devise}</td>
          </tr>
        `}
        ${vente.statut === "payee" ? `
          <tr>
            <td colspan="2" style="padding:4px 8px;font-size:11px;color:#555;">
              Règlement: ${paiements.map(p => fmtMode(p.mode)).join(", ")}
            </td>
          </tr>
        ` : ""}
      </table>
    </div>
  `;

  // ---- Pied ----
  const pied = `
    <div class="sep"></div>
    <div class="centre petit" style="margin-top:6px;">
      ${societe.pied_facture ?? "Merci de votre confiance"}
    </div>
    <div class="centre petit" style="margin-top:4px;">
      Imprimé le ${fmtDate(new Date().toISOString())}
    </div>
  `;

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>Facture ${vente.numero_facture ?? ""}</title>
  <style>${cssCommun}</style>
</head>
<body>
  ${enteteSociete}
  ${titreFacture}
  ${infosVente}
  ${tableauArticles}
  ${totaux}
  ${pied}
  <script>window.onload = () => { window.focus(); window.print(); }</script>
</body>
</html>`;
}