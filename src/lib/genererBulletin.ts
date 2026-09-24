// lib/genererBulletin.ts — le bulletin de paie (PLAN-EQUIPE, G-3 — D31).
//
// Le papier d'une fiche de paie VALIDÉE : ce qu'on remet à la personne.
// Même habillage que les autres documents (lib/impression.ts) : en-tête,
// pied, signatures avec cachet — « L'employé » et « Pour la société » par
// défaut, réglables dans Paramètres → Documents → Bulletin de paie.
//
// Chaque ligne redit d'où vient son montant, telle que le serveur l'a
// écrite : le bulletin ne recalcule rien.

import {
  type Habillage, esc, enLettres, blocSociete, imageEntete, imagePied, mention, blocSignatures,
} from "@/lib/impression";

export interface LigneBulletin { genre: string; libelle: string; montant: number }
export interface VersementBulletin { montant: number; moyen: string; date: string }
export interface DonneesBulletin {
  numero: string | null;
  statut: "brouillon" | "validee" | "remplacee";
  nom: string;
  fonction: string;
  du: string;
  au: string;
  brut: number;
  retenues: number;
  net: number;
  reporte: number;
  verse: number;
  reste: number;
  rectifie_numero?: string | null;
  remplacee_par?: string | null;
  valide_le?: string | null;
  lignes: LigneBulletin[];
  versements: VersementBulletin[];
  personne?: { telephone?: string | null; date_entree?: string | null; declare?: boolean; numero_inps?: string | null } | null;
  societe: Record<string, unknown>;
}

const MOYENS: Record<string, string> = {
  especes: "Espèces", orange_money: "Orange Money", moov_money: "Moov Money",
  cheque: "Chèque", virement: "Virement",
};

const f = (n: number) => new Intl.NumberFormat("fr-FR").format(n).replace(/\s/g, " ") + " F";
const jj = (iso?: string | null) => (iso ?? "").slice(0, 10).split("-").reverse().join("/");

export function genererBulletinHTML(d: DonneesBulletin, h: Habillage = {}): string {
  const r = h.reglage;
  const a5 = r?.format === "a5";
  const gains = d.lignes.filter(l => l.montant >= 0);
  const retenues = d.lignes.filter(l => l.montant < 0);
  const remplacee = d.statut === "remplacee";
  const titre = "BULLETIN DE PAIE";
  const ligne = (l: LigneBulletin) =>
    `<tr><td>${esc(l.libelle)}</td><td class="m">${l.montant < 0 ? "− " : ""}${f(Math.abs(l.montant))}</td></tr>`;

  const titreEtNumero = `
    <div style="text-align:right">
      <div class="titre">${titre}</div>
      <div class="det">${esc(d.numero ?? "")}${d.valide_le ? ` · validé le ${jj(d.valide_le)}` : ""}</div>
      <div class="det">Période du ${jj(d.du)} au ${jj(d.au)}</div>
    </div>`;
  const entete = h.entete
    ? `<div class="entete" style="justify-content:flex-end">${titreEtNumero}</div>`
    : `<div class="entete"><div>${blocSociete(d.societe, h, 15)}</div>${titreEtNumero}</div>`;
  const p = d.personne ?? {};
  const infos = [
    p.telephone ? `Tél. ${esc(p.telephone)}` : "",
    p.date_entree ? `Entrée le ${jj(p.date_entree)}` : "",
    p.declare && p.numero_inps ? `N° INPS ${esc(p.numero_inps)}` : "",
  ].filter(Boolean).join(" · ");

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${titre} — ${esc(d.nom)} — ${esc(d.numero ?? "")}</title>
<style>
  * { margin:0; padding:0; box-sizing:border-box; }
  body { font-family:Arial,sans-serif; font-size:12px; color:#000; }
  .page { min-height:${a5 ? "190mm" : "277mm"}; display:flex; flex-direction:column; }
  .corps { flex:1 1 auto; display:flex; flex-direction:column; padding:${h.entete ? "0" : "12mm"} 12mm 12mm 12mm; }
  .entete { display:flex; justify-content:space-between; border-bottom:2px solid #000; padding-bottom:7px; }
  .titre { font-size:17px; font-weight:bold; letter-spacing:.5px; }
  .det { font-size:10px; color:#555; }
  .personne { margin:12px 0; padding:7px 9px; border:1px solid #bbb; background:#fafafa; }
  .lbl { font-size:9px; color:#777; text-transform:uppercase; }
  .nom { font-size:14px; font-weight:bold; }
  table { width:100%; border-collapse:collapse; margin-top:6px; }
  th { text-align:left; font-size:10px; color:#555; border-bottom:1px solid #000; padding:4px 6px; }
  td { padding:4px 6px; border-bottom:1px solid #e5e5e5; }
  td.m, th.m { text-align:right; white-space:nowrap; }
  tr.total td { border-top:1px solid #000; font-weight:bold; }
  .net { border:2px solid #000; padding:10px 12px; margin-top:10px; display:flex; justify-content:space-between; align-items:baseline; }
  .net .chiffre { font-size:22px; font-weight:bold; }
  .lettres { font-size:11px; font-style:italic; color:#333; margin-top:4px; }
  .note { font-size:10.5px; color:#444; margin-top:6px; }
  .remplacee { color:#c00; font-weight:bold; border:2px solid #c00; padding:6px 8px; margin-top:8px; text-align:center; }
  .bas { margin-top:auto; }
  .mention { font-size:9.5px; color:#666; border-top:1px solid #ddd; padding-top:6px; margin-top:10px; }
  @media print { @page { size:${a5 ? "148mm 210mm" : "210mm 297mm"}; margin:0; } }
</style></head>
<body>
<div class="page">
  ${imageEntete(h)}
  <div class="corps">
    ${entete}
    ${remplacee ? `<div class="remplacee">Remplacé par ${esc(d.remplacee_par ?? "une fiche rectificative")} — ne vaut plus.</div>` : ""}
    ${d.rectifie_numero ? `<div class="note">Rectifie et remplace ${esc(d.rectifie_numero)}.</div>` : ""}

    <div class="personne">
      <div class="lbl">Salarié</div>
      <div class="nom">${esc(d.nom)} <span class="det">· ${esc(d.fonction)}</span></div>
      ${infos ? `<div class="det">${infos}</div>` : ""}
    </div>

    <table>
      <thead><tr><th>Ce qui est dû</th><th class="m">Montant</th></tr></thead>
      <tbody>
        ${gains.length ? gains.map(ligne).join("") : `<tr><td colspan="2" class="det">Rien sur la période.</td></tr>`}
        <tr class="total"><td>Brut</td><td class="m">${f(d.brut)}</td></tr>
      </tbody>
    </table>
    ${retenues.length ? `
    <table>
      <thead><tr><th>Ce qui est retenu</th><th class="m">Montant</th></tr></thead>
      <tbody>
        ${retenues.map(ligne).join("")}
        <tr class="total"><td>Retenues</td><td class="m">− ${f(d.retenues)}</td></tr>
      </tbody>
    </table>` : ""}

    <div class="net">
      <div><div class="lbl">Net à payer</div>
        ${r && !r.montant_lettres ? "" : `<div class="lettres">${esc(enLettres(d.net))} francs CFA</div>`}</div>
      <div class="chiffre">${f(d.net)}</div>
    </div>
    ${d.reporte > 0 ? `<div class="note">${f(d.reporte)} d'avances restent à retenir sur la prochaine paie.</div>` : ""}

    ${d.versements.length ? `
    <table>
      <thead><tr><th>Versé</th><th>Par</th><th class="m">Montant</th></tr></thead>
      <tbody>
        ${d.versements.map(v => `<tr><td>le ${jj(v.date)}</td><td>${esc(MOYENS[v.moyen] ?? v.moyen)}</td><td class="m">${f(v.montant)}</td></tr>`).join("")}
        <tr class="total"><td colspan="2">${d.reste > 0 ? "Reste à verser" : d.reste < 0 ? "Versé en trop" : "Payé"}</td>
          <td class="m">${d.reste === 0 ? f(d.verse) : f(Math.abs(d.reste))}</td></tr>
      </tbody>
    </table>` : ""}

    <div class="bas">
      ${blocSignatures(r ? r.signatures : [{ libelle: "L'employé" }, { libelle: "Pour la société" }])}
      ${h.pied ? "" : `<p class="mention">${mention(h, "Bulletin remis au salarié. À conserver.")}</p>`}
    </div>
  </div>
  ${imagePied(h)}
</div>
</body></html>`;
}
