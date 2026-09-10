// lib/genererRecu.ts — Reçu de règlement, client ou fournisseur
//
// Le reçu n'est PAS une pièce commerciale de plus : ni numéro de série,
// ni ligne en base. C'est une vue d'un règlement qui existe déjà, comme
// le bon de sortie est une vue d'une facture.
//
// Un seul générateur pour les deux côtés. Le document est le même — un
// montant, une date, un moyen, ce qui reste dû ; seul le sens de
// l'argent change, et c'est la seule chose que porte `cote`. En écrire
// deux les aurait fait diverger au premier ajout.
//
// Format A5 : un reçu se glisse dans une poche, il n'a pas besoin d'une
// pleine page A4.

export interface DonneesRecu {
  cote: "client" | "fournisseur";
  montant: number;
  mode: string;
  date: string;
  note?: string | null;
  auteur: string;
  /** Numéro de la facture réglée, vide si le règlement n'est pas imputé. */
  reference: string;
  /** Reste dû après ce règlement. `null` = pas de facture rattachée. */
  reste_du: number | null;
  /** Vrai si ce « règlement » est en fait une annulation. */
  est_annulation?: boolean;
  tiers: {
    nom: string; code?: string;
    telephone?: string | null; adresse?: string | null;
  };
  societe: { nom: string; adresse?: string | null; telephone?: string | null };
}

const MOTS = {
  client: {
    titre: "REÇU DE RÈGLEMENT",
    tiers: "Reçu de",
    // Le client paie : l'argent entre. La formule consacrée.
    formule: "Reçu la somme de",
    pied: "Ce reçu atteste du règlement ci-dessus. Il ne vaut pas facture.",
    signature: "Le caissier",
  },
  fournisseur: {
    titre: "REÇU DE PAIEMENT",
    tiers: "Payé à",
    formule: "Versé la somme de",
    pied: "Ce reçu atteste du paiement ci-dessus. Il ne vaut pas facture.",
    signature: "Le bénéficiaire",
  },
} as const;

const MOYENS: Record<string, string> = {
  especes: "Espèces", orange_money: "Orange Money",
  moov_money: "Moov Money", cheque: "Chèque", avoir: "Avoir",
};

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " FCFA";
}

function fmtDateHeure(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  }) + " à " + d.toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, c => (
    { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!
  ));
}

/**
 * Le montant en toutes lettres.
 *
 * Sur un reçu, c'est ce qui empêche de transformer 5 000 en 50 000 d'un
 * coup de stylo. Le franc CFA n'a pas de centime : pas de décimales à
 * écrire.
 */
export function enLettres(n: number): string {
  if (n === 0) return "zéro";
  if (n < 0) return "moins " + enLettres(-n);

  const U = ["", "un", "deux", "trois", "quatre", "cinq", "six", "sept",
             "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze",
             "quinze", "seize"];
  const D: Record<number, string> = {
    2: "vingt", 3: "trente", 4: "quarante", 5: "cinquante",
    6: "soixante", 8: "quatre-vingt",
  };

  const sousCent = (x: number): string => {
    if (x < 17) return U[x];
    if (x < 20) return "dix-" + U[x - 10];
    const d = Math.floor(x / 10), u = x % 10;
    // 70 et 90 se disent « soixante-dix » et « quatre-vingt-dix ».
    if (d === 7 || d === 9) {
      const base = d === 7 ? "soixante" : "quatre-vingt";
      const reste = x - (d === 7 ? 60 : 80);
      return base + (reste === 11 && d === 7 ? "-et-onze" : "-" + sousCent(reste));
    }
    if (u === 0) return D[d] + (d === 8 ? "s" : "");
    if (u === 1 && d !== 8) return D[d] + "-et-un";
    return D[d] + "-" + U[u];
  };

  const sousMille = (x: number): string => {
    if (x < 100) return sousCent(x);
    const c = Math.floor(x / 100), r = x % 100;
    const tete = c === 1 ? "cent" : U[c] + " cent" + (r === 0 ? "s" : "");
    return r === 0 ? tete : tete + " " + sousCent(r);
  };

  const tranches: [number, string, string][] = [
    [1_000_000_000, "milliard", "milliards"],
    [1_000_000, "million", "millions"],
    [1_000, "mille", "mille"],
  ];

  let reste = n;
  const bouts: string[] = [];
  for (const [valeur, sing, plur] of tranches) {
    const q = Math.floor(reste / valeur);
    if (q > 0) {
      // « mille » est invariable et ne prend pas « un » devant.
      if (valeur === 1000 && q === 1) bouts.push("mille");
      else bouts.push(sousMille(q) + " " + (q > 1 ? plur : sing));
      reste %= valeur;
    }
  }
  if (reste > 0) bouts.push(sousMille(reste));
  return bouts.join(" ");
}

export function genererRecuHTML(
  d: DonneesRecu,
  logoBase64?: string | null,
  enteteBase64?: string | null,
): string {
  const m = MOTS[d.cote];
  const annulation = !!d.est_annulation;
  // Une annulation n'encaisse rien : elle constate une correction. Le
  // dire en toutes lettres évite qu'un reçu de correction circule comme
  // une preuve de paiement.
  const titre = annulation ? "ANNULATION DE RÈGLEMENT" : m.titre;
  const montant = Math.abs(d.montant);

  const entete = enteteBase64
    ? `<img src="${enteteBase64}" alt=""
            style="width:100%;height:auto;display:block;margin-bottom:8px"/>`
    : `<div class="entete">
         <div>
           ${logoBase64
             ? `<img src="${logoBase64}" alt="" style="max-height:46px;display:block;margin-bottom:3px">`
             : ""}
           <div class="soc">${esc(d.societe.nom)}</div>
           ${d.societe.adresse
             ? `<div class="det">${esc(d.societe.adresse)}</div>` : ""}
           ${d.societe.telephone
             ? `<div class="det">Tél. ${esc(d.societe.telephone)}</div>` : ""}
         </div>
         <div style="text-align:right">
           <div class="titre${annulation ? " annule" : ""}">${titre}</div>
           <div class="det">${fmtDateHeure(d.date)}</div>
         </div>
       </div>`;

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${titre} — ${esc(d.tiers.nom)}</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Arial,sans-serif; font-size:12px; color:#000; }
  .page { min-height:190mm; display:flex; flex-direction:column; }
  .corps { flex:1 1 auto; display:flex; flex-direction:column;
           padding:${enteteBase64 ? "0" : "12mm"} 12mm 12mm 12mm; }
  .entete { display:flex; justify-content:space-between;
            border-bottom:2px solid #000; padding-bottom:7px; }
  .soc { font-size:15px; font-weight:bold; }
  .det { font-size:10px; color:#555; }
  .titre { font-size:17px; font-weight:bold; letter-spacing:.5px; }
  .titre.annule { color:#c00; }

  .tiers { margin:12px 0; padding:7px 9px; border:1px solid #bbb;
           background:#fafafa; }
  .lbl { font-size:9px; color:#777; text-transform:uppercase; }
  .nom { font-size:14px; font-weight:bold; }

  /* Le montant est ce qu'on lit en premier : il domine la page. */
  .montant-bloc { border:2px solid #000; padding:12px 14px; margin:8px 0; }
  .formule { font-size:11px; color:#444; }
  .chiffre { font-size:26px; font-weight:bold; margin:4px 0;
             ${annulation ? "color:#c00;" : ""} }
  .lettres { font-size:11px; font-style:italic; color:#333;
             text-transform:capitalize; border-top:1px dashed #999;
             padding-top:5px; margin-top:5px; }

  table.det-reg { width:100%; border-collapse:collapse; margin-top:10px; }
  table.det-reg td { padding:5px 7px; border-bottom:1px solid #e5e5e5;
                     font-size:11px; }
  table.det-reg td.cle { color:#666; width:42%; }
  table.det-reg td.val { text-align:right; font-weight:600; }
  .reste { background:#fff5f5; }
  .reste td { color:#c00; font-weight:bold; font-size:12px; }
  .solde { background:#f0f9f2; }
  .solde td { color:#1b6b3a; font-weight:bold; }

  .bas { margin-top:auto; }
  .mention { font-size:9.5px; color:#666; border-top:1px solid #ddd;
             padding-top:6px; margin-top:10px; }
  .sign { display:flex; justify-content:flex-end; margin-top:20px; }
  .sign > div { width:52%; }
  .sign .lbl2 { font-size:10px; color:#333; margin-bottom:26px; }
  .sign .trait { border-bottom:1px solid #000; }

  @media print { body { margin:0; } @page { size:148mm 210mm; margin:0; } }
</style></head>
<body>
<div class="page">
  ${enteteBase64 ? entete : ""}
  <div class="corps">
    ${enteteBase64 ? "" : entete}

    <div class="tiers">
      <div class="lbl">${m.tiers}</div>
      <div class="nom">${esc(d.tiers.nom)}${
        d.tiers.code ? ` <span class="det">· ${esc(d.tiers.code)}</span>` : ""}</div>
      ${d.tiers.telephone ? `<div class="det">Tél. ${esc(d.tiers.telephone)}</div>` : ""}
      ${d.tiers.adresse ? `<div class="det">${esc(d.tiers.adresse)}</div>` : ""}
    </div>

    <div class="montant-bloc">
      <div class="formule">${annulation ? "Annulation d'un règlement de" : m.formule}</div>
      <div class="chiffre">${fmt(montant)}</div>
      <div class="lettres">${enLettres(montant)} francs CFA</div>
    </div>

    <table class="det-reg">
      <tr>
        <td class="cle">Moyen</td>
        <td class="val">${MOYENS[d.mode] ?? esc(d.mode)}</td>
      </tr>
      ${d.reference ? `
      <tr>
        <td class="cle">Facture</td>
        <td class="val" style="font-family:monospace">${esc(d.reference)}</td>
      </tr>` : ""}
      <tr>
        <td class="cle">Date</td>
        <td class="val">${fmtDateHeure(d.date)}</td>
      </tr>
      <tr>
        <td class="cle">Enregistré par</td>
        <td class="val">${esc(d.auteur)}</td>
      </tr>
      ${d.note ? `
      <tr>
        <td class="cle">Note</td>
        <td class="val" style="font-weight:400">${esc(d.note)}</td>
      </tr>` : ""}
      ${d.reste_du === null ? "" : d.reste_du > 0 ? `
      <tr class="reste">
        <td>Reste dû après ce règlement</td>
        <td class="val">${fmt(d.reste_du)}</td>
      </tr>` : `
      <tr class="solde">
        <td>Solde</td>
        <td class="val">Facture soldée</td>
      </tr>`}
    </table>

    <div class="bas">
      <div class="sign">
        <div>
          <div class="lbl2">${m.signature}</div>
          <div class="trait"></div>
        </div>
      </div>
      <p class="mention">${m.pied}</p>
    </div>
  </div>
</div>
<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}
