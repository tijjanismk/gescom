// lib/whatsapp.ts — WhatsApp sans service payant (PLAN-EQUIPE EQ-5, D51).
//
// On ne fait que preparer un lien `https://wa.me/<numero>?text=<texte>` :
// WhatsApp s'ouvre sur la conversation, le texte deja ecrit, et c'est la
// personne qui appuie sur Envoyer.
//
// Le numero est du texte libre sur les fiches (employe, client). On le
// lit, on ne le devine pas : un lien vers le mauvais numero enverrait un
// salaire ou une creance a un inconnu. Tout ce qui n'est pas clair rend
// une raison, que l'ecran affiche a cote du bouton desactive.
//
// Pur et sans import d'alias : `node --test src/lib/whatsapp.test.ts`.

export type NumeroLu =
  | { ok: true; chiffres: string; affiche: string }
  | { ok: false; raison: string };

const refus = (raison: string): NumeroLu => ({ ok: false, raison });

function malien(huit: string): NumeroLu {
  if (huit.startsWith("0")) return refus("Un numéro malien ne commence pas par 0.");
  const paires = huit.match(/../g)!.join(" ");
  return { ok: true, chiffres: `223${huit}`, affiche: `+223 ${paires}` };
}

/**
 * Lit un numero saisi sur une fiche. Acceptes : 8 chiffres (malien),
 * `+223` / `00223` / `223` suivi de 8 chiffres, `+` ou `00` suivi d'un
 * autre indicatif (8 a 15 chiffres en tout, la longueur maximale E.164).
 * Espaces, points, tirets et parentheses sont ignores.
 */
export function lireNumero(texte: string | null | undefined): NumeroLu {
  const t = (texte ?? "").trim();
  if (!t) return refus("Pas de numéro sur la fiche.");
  if (/[/,;]|\bou\b/i.test(t)) return refus("Deux numéros dans le champ : n'en garder qu'un sur la fiche.");
  if (/[a-z]/i.test(t)) return refus("Le numéro contient des lettres.");
  const intrus = t.match(/[^\d\s.\-()+]/);
  if (intrus) return refus(`Caractère inattendu « ${intrus[0]} » dans le numéro.`);
  // « (+223) 76 11 22 33 » s'ecrit aussi : le + se juge sans separateurs.
  const serre = t.replace(/[\s.\-()]/g, "");
  if (serre.lastIndexOf("+") > 0) return refus("Le « + » ne peut être qu'au début du numéro.");

  const chiffres = serre.replace(/\D/g, "");
  const international = serre.startsWith("+") ? chiffres : chiffres.startsWith("00") ? chiffres.slice(2) : null;

  if (international !== null) {
    if (international.startsWith("223")) {
      const reste = international.slice(3);
      if (reste.length !== 8) return refus(`Après +223, il faut 8 chiffres (il y en a ${reste.length}).`);
      return malien(reste);
    }
    if (international.startsWith("0")) return refus("Un indicatif de pays ne commence pas par 0.");
    if (international.length < 8 || international.length > 15) {
      return refus(`Numéro international de ${international.length} chiffres : il en faut de 8 à 15.`);
    }
    return { ok: true, chiffres: international, affiche: `+${international}` };
  }

  if (chiffres.length === 8) return malien(chiffres);
  if (chiffres.length === 11 && chiffres.startsWith("223")) return malien(chiffres.slice(3));
  if (chiffres.length < 8) return refus(`${chiffres.length} chiffres : un numéro malien en a 8.`);
  if (chiffres.length > 15) return refus(`${chiffres.length} chiffres : deux numéros ? N'en garder qu'un sur la fiche.`);
  return refus(
    `${chiffres.length} chiffres sans indicatif : un numéro malien en a 8 ; ` +
    "un numéro étranger s'écrit avec + (ex. +33…).",
  );
}

/** Le lien qui ouvre la conversation, le texte encode. */
export function lienWhatsApp(chiffres: string, texte: string): string {
  if (!/^\d{8,15}$/.test(chiffres)) throw new Error(`Numéro WhatsApp invalide : ${chiffres}`);
  return `https://wa.me/${chiffres}?text=${encodeURIComponent(texte)}`;
}

// ---------------------------------------------------------------------
//  Le bulletin resume (D51 § 3)
// ---------------------------------------------------------------------

/** Ce que le message lit des donnees du bulletin (`lire_donnees_bulletin`). */
export interface BulletinResume {
  numero: string | null;
  statut: string;
  nom: string;
  du: string;
  au: string;
  net: number;
  verse: number;
  reste: number;
  reporte: number;
  trop_verse?: number;
  remplacee_par?: string | null;
  societe: Record<string, unknown>;
}

const montant = (n: number) => new Intl.NumberFormat("fr-FR").format(n).replace(/\s/g, " ") + " F";
const jj = (iso: string) => iso.slice(0, 10).split("-").reverse().join("/");

/** Pourquoi ce bulletin ne s'envoie pas, ou null s'il s'envoie. */
export function raisonBulletin(d: BulletinResume): string | null {
  if (d.statut === "brouillon") return "Brouillon : le montant peut encore changer. Valider la fiche d'abord.";
  if (d.statut === "remplacee") {
    return `Fiche remplacée${d.remplacee_par ? ` par ${d.remplacee_par}` : ""} : envoyer la rectificative.`;
  }
  if (d.statut !== "validee") return `Fiche « ${d.statut} » : seule une fiche validée s'envoie.`;
  return null;
}

/** Le texte envoye a l'employe : periode, net, verse, reste du. */
export function texteBulletin(d: BulletinResume): string {
  const lignes = [
    `Bonjour ${d.nom},`,
    "",
    `Votre paie du ${jj(d.du)} au ${jj(d.au)}${d.numero ? ` (bulletin ${d.numero})` : ""} :`,
    `• Net à payer : ${montant(d.net)}`,
    `• Déjà versé : ${montant(d.verse)}`,
    d.reste > 0 ? `• Reste dû : ${montant(d.reste)}` : "• Reste dû : rien, la paie est soldée.",
  ];
  if (d.reporte > 0) lignes.push(`• Avances à retenir sur la prochaine paie : ${montant(d.reporte)}`);
  if ((d.trop_verse ?? 0) > 0) lignes.push(`• Versé en trop, repris sur la prochaine paie : ${montant(d.trop_verse ?? 0)}`);
  const societe = typeof d.societe?.nom === "string" ? d.societe.nom.trim() : "";
  if (societe) lignes.push("", societe);
  return lignes.join("\n");
}

// ---------------------------------------------------------------------
//  Le message au client (D51 § 4)
// ---------------------------------------------------------------------

/** Le debut du message ; la personne ecrit la suite. */
export function debutMessageClient(nom: string): string {
  return `Bonjour ${nom.trim()},\n\n`;
}

/** Vrai quand la personne a ecrit quelque chose apres le bonjour. */
export function messageEcrit(texte: string, nom: string): boolean {
  const t = texte.trim();
  return t.length > 0 && t !== debutMessageClient(nom).trim();
}
