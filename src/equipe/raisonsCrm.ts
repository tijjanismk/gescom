// equipe/raisonsCrm.ts — les mots de D50 (points 10 a 13), partages par
// la fiche 360, les prospects et la liste a relancer. Les cles sont
// celles du serveur (coeur::crm) ; seul le libelle est d'ici.

export const ETAPES_PROSPECT: [string, string][] = [
  ["nouveau", "Nouveau"], ["contacte", "Contacté"], ["interesse", "Intéressé"], ["abandonne", "Abandonné"],
];
export const RAISONS_ABANDON: [string, string][] = [
  ["trop_cher", "Trop cher"], ["achete_ailleurs", "A acheté ailleurs"], ["plus_besoin", "Plus besoin"],
  ["injoignable", "Injoignable"], ["autre", "Autre"],
];
export const RAISONS_DEPART: [string, string][] = [
  ["prix", "Prix"], ["concurrent", "Concurrent"], ["mecontent", "Mécontent"], ["demenage", "A déménagé"], ["autre", "Autre"],
];

const tous = Object.fromEntries([...ETAPES_PROSPECT, ...RAISONS_ABANDON, ...RAISONS_DEPART]);
export const libelle = (cle: string | null | undefined) => (cle ? tous[cle] ?? cle : "—");
