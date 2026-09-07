// lib/genererReleve.ts — État de créance (client) / de dette (fournisseur)
//
// Un seul générateur pour les deux côtés : le document est le même, seul
// le sens change. Chez le client il réclame, chez le fournisseur il
// oppose — mais les colonnes, les totaux et les signatures sont
// identiques. En écrire deux les aurait fait diverger.
//
// Le vocabulaire, lui, ne peut pas être commun : « reste dû » veut dire
// « ce que vous me devez » d'un côté et « ce que je vous dois » de
// l'autre. C'est la seule chose que porte `cote`.

export interface LigneReleve {
  date: string;
  numero: string;
  /** Fournisseur seulement : « Facture » ou « Avoir ». */
  type?: string;
  total: number;
  paye: number;
  reste: number;
}

export interface DonneesReleve {
  tiers: {
    nom: string; code?: string;
    telephone?: string | null; adresse?: string | null;
  };
  lignes: LigneReleve[];
  total_du: number;
  avoirs: number;
  net_du: number;
  societe: {
    nom: string; adresse?: string | null; telephone?: string | null;
  };
}

const MOTS = {
  client: {
    titre: "ÉTAT DE CRÉANCE",
    tiers: "Client",
    total: "TOTAL DÛ PAR LE CLIENT",
    mention:
      "Document récapitulatif des factures non soldées à ce jour. "
      + "Les règlements postérieurs à la date d'édition n'y figurent pas.",
    gauche: "Le client",
    droite: "Pour l'entreprise",
  },
  fournisseur: {
    titre: "ÉTAT DE DETTE",
    tiers: "Fournisseur",
    total: "TOTAL DÛ AU FOURNISSEUR",
    mention:
      "Document récapitulatif des factures fournisseur non soldées à ce "
      + "jour. Les règlements postérieurs à la date d'édition n'y figurent pas.",
    gauche: "Le fournisseur",
    droite: "Pour l'entreprise",
  },
} as const;

/** Vocabulaire de l'historique des versements, par côté.
 *
 *  Le document est le même des deux côtés — mêmes colonnes, mêmes règles
 *  d'affichage des annulations — mais un fournisseur à qui l'on tend une
 *  feuille titrée « encaissé par » aurait raison de la trouver fausse :
 *  c'est lui qui a reçu l'argent. */
const MOTS_HISTO = {
  client: {
    titre:   "HISTORIQUE DES RÈGLEMENTS",
    onglet:  "Règlements",
    tiers:   "Client",
    par:     "Encaissé par",
    reste:   "Reste dû après",
    total:   "TOTAL DES RÈGLEMENTS AFFICHÉS",
    solde:   "RESTE DÛ AUJOURD'HUI, TOUTES FACTURES",
    dette:   "la dette totale du client",
    vide:    "Aucun règlement sur cette période.",
    gauche:  "Le client",
  },
  fournisseur: {
    titre:   "HISTORIQUE DES PAIEMENTS",
    onglet:  "Paiements",
    tiers:   "Fournisseur",
    par:     "Versé par",
    reste:   "Reste à payer après",
    total:   "TOTAL DES PAIEMENTS AFFICHÉS",
    solde:   "RESTE À PAYER AUJOURD'HUI, TOUTES FACTURES",
    dette:   "ce qu'on doit encore à ce fournisseur",
    vide:    "Aucun paiement sur cette période.",
    gauche:  "Le fournisseur",
  },
} as const;

export interface LigneReleveGlobal {
  nom: string;
  code: string;
  telephone?: string | null;
  /** Nombre de factures ouvertes. */
  nb: number;
  total_du: number;
}

export interface DonneesReleveGlobal {
  lignes: LigneReleveGlobal[];
  total_general: number;
  societe: {
    nom: string; adresse?: string | null; telephone?: string | null;
  };
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " FCFA";
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

/** Échappe le HTML : un nom de tiers est une saisie libre. */
function esc(s: string): string {
  return s.replace(/[&<>"']/g, c => (
    { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!
  ));
}

/**
 * État GLOBAL — un tiers par ligne, tous confondus.
 *
 * C'est le document de pilotage : combien le commerce a dehors, et chez
 * qui. Il partage la feuille de style du relevé individuel, mais pas sa
 * structure — ici une ligne est un tiers, pas une facture.
 *
 * Trié du plus gros au plus petit : c'est le premier de la liste qu'on
 * appelle le lundi matin.
 */
export function genererReleveGlobalHTML(
  d: DonneesReleveGlobal,
  cote: "client" | "fournisseur",
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  const maintenant = new Date();
  const titre = cote === "client"
    ? "ÉTAT GLOBAL DES CRÉANCES" : "ÉTAT GLOBAL DES DETTES";
  const colTiers = cote === "client" ? "Client" : "Fournisseur";
  const totalLabel = cote === "client"
    ? "TOTAL DÛ PAR LES CLIENTS" : "TOTAL DÛ AUX FOURNISSEURS";

  const lignes = d.lignes.map((l, i) => `
    <tr>
      <td class="rang">${i + 1}</td>
      <td>
        <strong>${esc(l.nom)}</strong>
        ${l.code ? `<span class="det"> · ${esc(l.code)}</span>` : ""}
        ${l.telephone ? `<div class="det">${esc(l.telephone)}</div>` : ""}
      </td>
      <td class="d">${l.nb}</td>
      <td class="d fort">${fmt(l.total_du)}</td>
    </tr>`).join("");

  const entete = enteteBase64
    ? `<img src="${enteteBase64}" style="width:100%;display:block;margin-bottom:10px">`
    : `<div class="entete">
         <div>
           ${logoBase64
             ? `<img src="${logoBase64}" style="max-height:52px;margin-bottom:4px">`
             : ""}
           <div class="soc">${esc(d.societe.nom)}</div>
           ${d.societe.adresse
             ? `<div class="det">${esc(d.societe.adresse)}</div>` : ""}
         </div>
         <div style="text-align:right">
           <div class="titre">${titre}</div>
           <div class="det">Édité le ${maintenant.toLocaleDateString("fr-ML")}
             à ${maintenant.toLocaleTimeString("fr-ML",
               { hour: "2-digit", minute: "2-digit" })}</div>
           <div class="det">${d.lignes.length} ${
             cote === "client" ? "client(s)" : "fournisseur(s)"} concerné(s)</div>
         </div>
       </div>`;

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${titre}</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Arial,sans-serif; font-size:12px; padding:12mm; }
  .entete { display:flex; justify-content:space-between;
            border-bottom:2px solid #000; padding-bottom:8px;
            margin-bottom:12px; }
  .soc { font-size:16px; font-weight:bold; }
  .titre { font-size:18px; font-weight:bold; letter-spacing:1px; }
  .det { font-size:10px; color:#555; }
  table { width:100%; border-collapse:collapse; }
  th { background:#eee; border-bottom:2px solid #000; padding:6px 8px;
       text-align:left; font-size:10px; text-transform:uppercase; }
  td { padding:6px 8px; border-bottom:1px solid #e5e5e5;
       vertical-align:top; }
  .rang { color:#999; width:28px; }
  .d { text-align:right; }
  .fort { font-weight:bold; }
  .total { border-top:2px solid #000; background:#f5f5f5; }
  .total td { padding:9px 8px; font-size:14px; font-weight:bold; }
  .vide { text-align:center; padding:26px; color:#777; }
  .mention { margin-top:10px; font-size:10px; color:#666;
             border-top:1px solid #ddd; padding-top:6px; }
  .sign { display:flex; justify-content:space-between; margin-top:34px; }
  .sign > div { width:45%; border-top:1px solid #000; padding-top:6px;
                text-align:center; font-size:11px; }
  @media print { body { margin:0; } @page { size:A4; margin:8mm; }
                 thead { display:table-header-group; } }
</style></head>
<body>

${entete}

${d.lignes.length === 0 ? `
  <p class="vide">${cote === "client"
    ? "Aucune créance ouverte — tous les clients sont à jour."
    : "Aucune dette ouverte — tous les fournisseurs sont réglés."}</p>
` : `
<table>
  <thead>
    <tr>
      <th></th><th>${colTiers}</th>
      <th class="d">Factures</th><th class="d">Reste dû</th>
    </tr>
  </thead>
  <tbody>${lignes}</tbody>
  <tfoot>
    <tr class="total">
      <td colspan="3">${totalLabel}</td>
      <td class="d">${fmt(d.total_general)}</td>
    </tr>
  </tfoot>
</table>
`}

<p class="mention">
  Situation arrêtée à la date d'édition. Les règlements postérieurs n'y
  figurent pas. Montants calculés sur les paiements réellement
  enregistrés.
</p>

<div class="sign">
  <div>Établi par</div>
  <div>Vérifié par</div>
</div>

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}

export interface LigneHistorique {
  date_paiement: string;
  numero_facture: string;
  mode: string;
  montant: number;
  reste_apres: number;
  auteur_nom: string;
  est_annulation: boolean;
  deja_annule: boolean;
}

const MOYENS_RECU: Record<string, string> = {
  especes: "Espèces", orange_money: "Orange Money",
  moov_money: "Moov Money", cheque: "Chèque", avoir: "Avoir",
};

/**
 * Historique des règlements d'un client, tel qu'il est filtré à l'écran.
 *
 * On imprime CE QUI EST AFFICHÉ, critères compris — comme l'historique
 * des mouvements de stock. Un document qui ne correspondrait pas à
 * l'écran d'où il sort ferait douter des deux.
 *
 * Les annulations y figurent, en négatif : c'est justement ce document
 * qu'on tend au client qui conteste, et une correction masquée n'aurait
 * aucune valeur.
 */
export function genererHistoriqueReglementsHTML(
  tiers: { nom: string; code?: string; telephone?: string | null },
  cote: "client" | "fournisseur",
  lignes: LigneHistorique[],
  criteres: string,
  totalDuTiers: number,
  societe: { nom: string; adresse?: string | null; telephone?: string | null },
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  const m = MOTS_HISTO[cote];
  const maintenant = new Date();
  const totalPeriode = lignes.reduce((s, l) => s + l.montant, 0);

  const corps = lignes.map(l => `
    <tr${l.deja_annule ? ' class="barre"' : ""}>
      <td>${fmtDate(l.date_paiement)}</td>
      <td class="mono">${esc(l.numero_facture || "—")}</td>
      <td>${MOYENS_RECU[l.mode] ?? esc(l.mode)}</td>
      <td class="det">${esc(l.auteur_nom)}</td>
      <td class="d ${l.est_annulation ? "rouge" : "fort"}">${fmt(l.montant)}</td>
      <td class="d det">${fmt(l.reste_apres)}</td>
    </tr>`).join("");

  const entete = enteteBase64
    ? `<img src="${enteteBase64}" style="width:100%;display:block;margin-bottom:10px">`
    : `<div class="entete">
         <div>
           ${logoBase64
             ? `<img src="${logoBase64}" style="max-height:52px;margin-bottom:4px">`
             : ""}
           <div class="soc">${esc(societe.nom)}</div>
           ${societe.adresse
             ? `<div class="det">${esc(societe.adresse)}</div>` : ""}
         </div>
         <div style="text-align:right">
           <div class="titre">${m.titre}</div>
           <div class="det">Édité le ${maintenant.toLocaleDateString("fr-ML")}
             à ${maintenant.toLocaleTimeString("fr-ML",
               { hour: "2-digit", minute: "2-digit" })}</div>
           ${criteres ? `<div class="det">${esc(criteres)}</div>` : ""}
         </div>
       </div>`;

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${m.onglet} — ${esc(tiers.nom)}</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Arial,sans-serif; font-size:12px; padding:12mm; }
  .entete { display:flex; justify-content:space-between;
            border-bottom:2px solid #000; padding-bottom:8px; }
  .soc { font-size:16px; font-weight:bold; }
  .titre { font-size:17px; font-weight:bold; letter-spacing:.5px; }
  .det { font-size:10px; color:#555; }
  .tiers { margin:12px 0 10px; padding:8px 10px; border:1px solid #bbb;
           background:#fafafa; }
  .lbl { font-size:9px; color:#777; text-transform:uppercase; }
  .nom { font-size:14px; font-weight:bold; }
  table { width:100%; border-collapse:collapse; margin-top:6px; }
  th { background:#eee; border-bottom:2px solid #000; padding:6px 8px;
       text-align:left; font-size:10px; text-transform:uppercase; }
  td { padding:6px 8px; border-bottom:1px solid #e5e5e5; }
  .d { text-align:right; }
  .fort { font-weight:bold; }
  .rouge { color:#c00; font-weight:bold; }
  .mono { font-family:'Courier New',monospace; font-size:11px; }
  .barre td { color:#999; text-decoration:line-through; }
  .total { border-top:2px solid #000; background:#f5f5f5; }
  .total td { padding:9px 8px; font-size:13px; font-weight:bold; }
  .du { background:#fff5f5; }
  .du td { padding:9px 8px; font-size:13px; font-weight:bold; color:#c00; }
  .vide { text-align:center; padding:26px; color:#777; }
  .mention { margin-top:10px; font-size:10px; color:#666;
             border-top:1px solid #ddd; padding-top:6px; }
  .sign { display:flex; justify-content:space-between; margin-top:34px; }
  .sign > div { width:45%; border-top:1px solid #000; padding-top:6px;
                text-align:center; font-size:11px; }
  @media print { body { margin:0; } @page { size:A4; margin:8mm; }
                 thead { display:table-header-group; } }
</style></head>
<body>

${entete}

<div class="tiers">
  <div class="lbl">${m.tiers}</div>
  <div class="nom">${esc(tiers.nom)}${
    tiers.code ? ` <span class="det">· ${esc(tiers.code)}</span>` : ""}</div>
  ${tiers.telephone ? `<div class="det">Tél. ${esc(tiers.telephone)}</div>` : ""}
</div>

${lignes.length === 0 ? `
  <p class="vide">${m.vide}</p>
` : `
<table>
  <thead>
    <tr>
      <th>Date</th><th>Facture</th><th>Moyen</th><th>${m.par}</th>
      <th class="d">Montant</th><th class="d">${m.reste}</th>
    </tr>
  </thead>
  <tbody>${corps}</tbody>
  <tfoot>
    <tr class="total">
      <td colspan="4">${m.total}</td>
      <td class="d">${fmt(totalPeriode)}</td>
      <td></td>
    </tr>
    ${totalDuTiers > 0 ? `
    <tr class="du">
      <td colspan="4">${m.solde}</td>
      <td class="d" colspan="2">${fmt(totalDuTiers)}</td>
    </tr>` : ""}
  </tfoot>
</table>
`}

<p class="mention">
  La colonne « ${m.reste.toLowerCase()} » donne le solde de la facture
  concernée juste après ce versement, pas ${m.dette}. Une ligne barrée est
  un versement annulé ; une ligne en rouge est l'annulation elle-même.
</p>

<div class="sign">
  <div>${m.gauche}</div>
  <div>Pour l'entreprise</div>
</div>

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}

export function genererReleveHTML(
  d: DonneesReleve,
  cote: "client" | "fournisseur",
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  const m = MOTS[cote];
  const maintenant = new Date();
  const avecType = cote === "fournisseur";

  const lignes = d.lignes.map(l => `
    <tr>
      <td>${fmtDate(l.date)}</td>
      <td class="mono">${esc(l.numero || "—")}</td>
      ${avecType ? `<td>${esc(l.type ?? "")}</td>` : ""}
      <td class="d">${fmt(l.total)}</td>
      <td class="d">${fmt(l.paye)}</td>
      <td class="d fort">${fmt(l.reste)}</td>
    </tr>`).join("");

  // Bandeau à en-tête s'il existe : il porte déjà nom, adresse et
  // téléphone, les répéter ferait doublon sur le papier (cf. D4 et le
  // même choix dans genererPDF).
  const entete = enteteBase64
    ? `<img src="${enteteBase64}" style="width:100%;display:block;margin-bottom:10px">`
    : `<div class="entete">
         <div>
           ${logoBase64
             ? `<img src="${logoBase64}" style="max-height:52px;margin-bottom:4px">`
             : ""}
           <div class="soc">${esc(d.societe.nom)}</div>
           ${d.societe.adresse
             ? `<div class="det">${esc(d.societe.adresse)}</div>` : ""}
           ${d.societe.telephone
             ? `<div class="det">Tél. ${esc(d.societe.telephone)}</div>` : ""}
         </div>
         <div style="text-align:right">
           <div class="titre">${m.titre}</div>
           <div class="det">Édité le ${maintenant.toLocaleDateString("fr-ML")}
             à ${maintenant.toLocaleTimeString("fr-ML",
               { hour: "2-digit", minute: "2-digit" })}</div>
         </div>
       </div>`;

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${m.titre} — ${esc(d.tiers.nom)}</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Arial,sans-serif; font-size:12px; padding:12mm; }
  .entete { display:flex; justify-content:space-between;
            border-bottom:2px solid #000; padding-bottom:8px; }
  .soc { font-size:16px; font-weight:bold; }
  .titre { font-size:18px; font-weight:bold; letter-spacing:1px; }
  .det { font-size:10px; color:#555; }
  .tiers { margin:12px 0 10px; padding:8px 10px; border:1px solid #bbb;
           background:#fafafa; }
  .tiers .lbl { font-size:9px; color:#777; text-transform:uppercase; }
  .tiers .nom { font-size:14px; font-weight:bold; }
  table { width:100%; border-collapse:collapse; margin-top:6px; }
  th { background:#eee; border-bottom:2px solid #000; padding:6px 8px;
       text-align:left; font-size:10px; text-transform:uppercase; }
  td { padding:6px 8px; border-bottom:1px solid #e5e5e5; }
  .d { text-align:right; }
  .fort { font-weight:bold; }
  .mono { font-family:'Courier New',monospace; font-size:11px; }
  .total { border-top:2px solid #000; background:#f5f5f5; }
  .total td { padding:9px 8px; font-size:14px; font-weight:bold; }
  .avoir td { color:#0a7; font-weight:normal; }
  .vide { text-align:center; padding:26px; color:#777; }
  .mention { margin-top:10px; font-size:10px; color:#666;
             border-top:1px solid #ddd; padding-top:6px; }
  .sign { display:flex; justify-content:space-between; margin-top:34px; }
  .sign > div { width:45%; border-top:1px solid #000; padding-top:6px;
                text-align:center; font-size:11px; }
  @media print { body { margin:0; } @page { size:A4; margin:8mm; } }
</style></head>
<body>

${entete}

<div class="tiers">
  <div class="lbl">${m.tiers}</div>
  <div class="nom">${esc(d.tiers.nom)}${
    d.tiers.code ? ` <span class="det">· ${esc(d.tiers.code)}</span>` : ""}</div>
  ${d.tiers.telephone ? `<div class="det">Tél. ${esc(d.tiers.telephone)}</div>` : ""}
  ${d.tiers.adresse ? `<div class="det">${esc(d.tiers.adresse)}</div>` : ""}
</div>

${d.lignes.length === 0 ? `
  <p class="vide">Aucune somme due à ce jour.</p>
` : `
<table>
  <thead>
    <tr>
      <th>Date</th><th>Pièce</th>
      ${avecType ? "<th>Nature</th>" : ""}
      <th class="d">Total</th><th class="d">Réglé</th><th class="d">Reste dû</th>
    </tr>
  </thead>
  <tbody>${lignes}</tbody>
  <tfoot>
    ${d.avoirs > 0 ? `
    <tr class="avoir">
      <td colspan="${avecType ? 5 : 4}">Avoirs disponibles à déduire</td>
      <td class="d">− ${fmt(d.avoirs)}</td>
    </tr>` : ""}
    <tr class="total">
      <td colspan="${avecType ? 5 : 4}">${m.total}</td>
      <td class="d">${fmt(d.net_du)}</td>
    </tr>
  </tfoot>
</table>
`}

<p class="mention">${m.mention}</p>

<div class="sign">
  <div>${m.gauche}</div>
  <div>${m.droite}</div>
</div>

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}
