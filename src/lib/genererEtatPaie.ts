// lib/genererEtatPaie.ts — l'état de paie du mois, imprimé (PLAN-EQUIPE,
// EQ-2 — D48).
//
// L'habillage du bulletin (en-tête, pied, filigrane de la société), sans
// réglage de plus ; toujours en A4. Le serveur a tout compté : l'état
// imprimé ne recalcule rien, il range ce que `lire_etat_paie` a rendu.

import { type Habillage, esc, blocSociete, mention, documentHTML } from "@/lib/impression";

export interface MontantsEtat {
  brut: number; avances: number; retenues: number; cotisations: number; charges: number;
  net: number; verse: number; reste: number; trop_verse: number;
}
export interface LigneEtat extends MontantsEtat {
  employe_id: string; nom: string; fonction: string; numeros: string[];
}
export interface EtatPaie {
  mois: string; du: string; au: string;
  lignes: LigneEtat[];
  total: MontantsEtat;
  /** Brut + charges patronales : le chiffre de Rapports → CA mensuel. */
  cout: number;
  brouillons: { id: string; employe_id: string; nom: string; fonction: string; du: string; au: string; net: number }[];
  sans_fiche: { employe_id: string; nom: string; fonction: string }[];
  etabli_le: string;
  etabli_par: string | null;
  societe: Record<string, unknown>;
}

const f = (n: number) => new Intl.NumberFormat("fr-FR").format(n).replace(/\s/g, " ") + " F";
const jj = (iso?: string | null) => (iso ?? "").slice(0, 10).split("-").reverse().join("/");

/** « octobre 2026 ». */
export function moisDit(mois: string): string {
  const [a, m] = mois.split("-").map(Number);
  return new Date(a, m - 1, 1).toLocaleDateString("fr-FR", { month: "long", year: "numeric" });
}

/** Les colonnes de l'état, dans l'ordre ; les charges et le versé en trop
 *  seulement si le mois en a. Partagées par l'écran et le papier. */
export function colonnesEtat(total: MontantsEtat): [keyof MontantsEtat, string][] {
  const c: [keyof MontantsEtat, string][] = [
    ["brut", "Brut"], ["avances", "Avances"], ["retenues", "Autres retenues"], ["cotisations", "Cotisations"],
    ["net", "Net"], ["verse", "Déjà versé"], ["reste", "Reste dû"],
  ];
  if (total.trop_verse > 0) c.push(["trop_verse", "Versé en trop"]);
  if (total.charges > 0) c.push(["charges", "Charges patronales"]);
  return c;
}

export function genererEtatPaieHTML(d: EtatPaie, h: Habillage = {}): string {
  const titre = "ÉTAT DE PAIE";
  const colonnes = colonnesEtat(d.total);
  const chiffre = (m: MontantsEtat, cle: keyof MontantsEtat) => m[cle] === 0 ? "—" : f(m[cle]);

  const titreEtMois = `
    <div style="text-align:right">
      <div class="titre">${titre}</div>
      <div class="det">${esc(moisDit(d.mois))} · du ${jj(d.du)} au ${jj(d.au)}</div>
    </div>`;
  const entete = h.entete
    ? `<div class="entete" style="justify-content:flex-end">${titreEtMois}</div>`
    : `<div class="entete"><div>${blocSociete(d.societe, h, 14)}</div>${titreEtMois}</div>`;

  const css = `
  .entete { display:flex; justify-content:space-between; border-bottom:2px solid #000; padding-bottom:7px; }
  .titre { font-size:17px; font-weight:bold; letter-spacing:.5px; }
  .det { font-size:9.5px; color:#555; }
  table { width:100%; border-collapse:collapse; margin-top:10px; font-size:8.5pt; }
  th { text-align:left; font-size:9px; color:#555; border-bottom:1px solid #000; padding:4px 4px; vertical-align:bottom; }
  td { padding:4px 4px; border-bottom:1px solid #e5e5e5; vertical-align:top; }
  /* Neuf colonnes tiennent dans la largeur d'une A4 : les montants restent
     sur une ligne, les entetes passent a la ligne. */
  td.m, th.m { text-align:right; }
  td.m { white-space:nowrap; }
  tr.total td { border-top:2px solid #000; border-bottom:none; font-weight:bold; }
  .cout { border:2px solid #000; padding:8px 10px; margin-top:10px; display:flex; justify-content:space-between; align-items:baseline; }
  .cout .chiffre { font-size:17px; font-weight:bold; }
  h3 { font-size:11px; margin:12px 0 2px; }
  ul { margin:0; padding-left:16px; }
  .note { font-size:9.5px; color:#444; margin-top:8px; }
`;

  const lignes = d.lignes.map(l => `
    <tr>
      <td><b>${esc(l.nom)}</b><div class="det">${esc(l.fonction)} · ${esc(l.numeros.join(", "))}</div></td>
      ${colonnes.map(([c]) => `<td class="m">${chiffre(l, c)}</td>`).join("")}
    </tr>`).join("");

  const contenu = `
    ${entete}
    <table>
      <thead><tr><th>Personne</th>${colonnes.map(([, t]) => `<th class="m">${esc(t)}</th>`).join("")}</tr></thead>
      <tbody>
        ${lignes || `<tr><td colspan="${colonnes.length + 1}" class="det">Aucune fiche validée pour ce mois.</td></tr>`}
        <tr class="total"><td>Total (${d.lignes.length} personne${d.lignes.length > 1 ? "s" : ""})</td>
          ${colonnes.map(([c]) => `<td class="m">${f(d.total[c])}</td>`).join("")}</tr>
      </tbody>
    </table>

    <div class="cout">
      <div><div class="det">Coût du mois pour la boutique (brut${d.total.charges > 0 ? " + charges patronales" : ""})</div></div>
      <div class="chiffre">${f(d.cout)}</div>
    </div>

    ${d.brouillons.length ? `
    <h3>À valider — pas comptés ci-dessus</h3>
    <ul>${d.brouillons.map(b => `<li>${esc(b.nom)} (${esc(b.fonction)}), du ${jj(b.du)} au ${jj(b.au)} : ${f(b.net)} provisoire</li>`).join("")}</ul>` : ""}
    ${d.sans_fiche.length ? `
    <h3>Pas de fiche ce mois</h3>
    <ul>${d.sans_fiche.map(p => `<li>${esc(p.nom)} (${esc(p.fonction)})</li>`).join("")}</ul>` : ""}

    <div class="note">Seules les fiches validées comptent. Une fiche compte dans le mois où finit sa période.
      Le « déjà versé » est celui du jour où l'état est établi.</div>
    <div class="note">Établi le ${jj(d.etabli_le)}${d.etabli_par ? ` par ${esc(d.etabli_par)}` : ""}.</div>`;

  return documentHTML(h, {
    titre: `${titre} — ${moisDit(d.mois)}`,
    papier: "a4",
    marge: "10mm",
    mention: mention(h, "État de paie — document interne."),
    police: 10.5,
    css,
    contenu,
  });
}
