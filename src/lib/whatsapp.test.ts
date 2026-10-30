// Vérifie lib/whatsapp.ts (D51) : `node --test src/lib/whatsapp.test.ts`
// Un numéro lu de travers enverrait un salaire ou une créance à un
// inconnu : chaque forme acceptée, et chaque refus avec sa raison.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  lireNumero, lienWhatsApp, raisonBulletin, texteBulletin, debutMessageClient, messageEcrit,
  type BulletinResume,
} from "./whatsapp.ts";

const accepte = (texte: string) => {
  const n = lireNumero(texte);
  assert.ok(n.ok, `${texte} : ${n.ok ? "" : n.raison}`);
  return n;
};
const refuse = (texte: string | null) => {
  const n = lireNumero(texte);
  assert.ok(!n.ok, `${texte} aurait dû être refusé`);
  return n.raison;
};

test("les écritures d'un numéro malien donnent le même numéro", () => {
  for (const t of [
    "76 11 22 33", "76112233", "76.11.22.33", "76-11-22-33", " 76 11 22 33 ",
    "0022376112233", "00 223 76 11 22 33", "+223 76 11 22 33", "+22376112233",
    "(+223) 76 11 22 33", "223 76 11 22 33",
  ]) {
    const n = accepte(t);
    assert.equal(n.chiffres, "22376112233", t);
    assert.equal(n.affiche, "+223 76 11 22 33", t);
  }
});

test("un numéro étranger écrit avec son indicatif passe tel quel", () => {
  const fr = accepte("+33 6 12 34 56 78");
  assert.equal(fr.chiffres, "33612345678");
  assert.equal(fr.affiche, "+33612345678");
  assert.equal(accepte("00225 07 07 12 34 56").chiffres, "2250707123456"); // Côte d'Ivoire
});

test("le vide, les lettres et deux numéros sont refusés avec leur raison", () => {
  assert.match(refuse(""), /Pas de numéro/);
  assert.match(refuse(null), /Pas de numéro/);
  assert.match(refuse("   "), /Pas de numéro/);
  assert.match(refuse("76 11 22 33 / 66 44 55 66"), /Deux numéros/);
  assert.match(refuse("76112233 ou 66445566"), /Deux numéros/);
  assert.match(refuse("7611223366445566"), /deux numéros/);
  assert.match(refuse("voir Awa"), /lettres/);
  assert.match(refuse("76*11*22*33"), /Caractère inattendu/);
  assert.match(refuse("76 11 + 22 33"), /« \+ »/);
});

test("une longueur fausse est refusée, on ne devine pas le pays", () => {
  assert.match(refuse("76 11 22 3"), /7 chiffres : un numéro malien en a 8/);
  assert.match(refuse("06 11 22 33"), /ne commence pas par 0/);
  assert.match(refuse("06 12 34 56 78"), /10 chiffres sans indicatif/);
  assert.match(refuse("+223 76 11 22"), /Après \+223, il faut 8 chiffres \(il y en a 6\)/);
  assert.match(refuse("+223 06 11 22 33"), /ne commence pas par 0/);
  assert.match(refuse("+0 761122334"), /indicatif/);
  assert.match(refuse("+33 61"), /de 8 à 15/);
});

test("le lien porte le numéro et le texte encodé, rien d'autre", () => {
  const lien = lienWhatsApp("22376112233", "Bonjour Awa,\n\nNet : 85 000 F & reste 0 %");
  assert.equal(lien, "https://wa.me/22376112233?text=Bonjour%20Awa%2C%0A%0ANet%20%3A%2085%20000%20F%20%26%20reste%200%20%25");
  const u = new URL(lien);
  assert.equal(u.hostname, "wa.me");
  assert.equal(u.pathname, "/22376112233");
  assert.equal(u.searchParams.get("text"), "Bonjour Awa,\n\nNet : 85 000 F & reste 0 %");
  assert.throws(() => lienWhatsApp("223 76", "x"));
  assert.throws(() => lienWhatsApp("22376112233?text=autre", "x"));
});

const fiche: BulletinResume = {
  numero: "PAIE-2026-00012", statut: "validee", nom: "Moussa Traoré",
  du: "2026-10-01", au: "2026-10-31", net: 85_000, verse: 50_000, reste: 35_000, reporte: 0,
  societe: { nom: "Boutique Awa" },
};

test("le bulletin résumé : période, numéro, net, versé, reste dû, société", () => {
  assert.equal(texteBulletin(fiche), [
    "Bonjour Moussa Traoré,",
    "",
    "Votre paie du 01/10/2026 au 31/10/2026 (bulletin PAIE-2026-00012) :",
    "• Net à payer : 85 000 F",
    "• Déjà versé : 50 000 F",
    "• Reste dû : 35 000 F",
    "",
    "Boutique Awa",
  ].join("\n"));
});

test("soldée, avance reportée et versé en trop se disent", () => {
  const t = texteBulletin({ ...fiche, verse: 85_000, reste: 0, reporte: 10_000, trop_verse: 5_000, societe: {} });
  assert.match(t, /Reste dû : rien, la paie est soldée\./);
  assert.match(t, /Avances à retenir sur la prochaine paie : 10 000 F/);
  assert.match(t, /Versé en trop, repris sur la prochaine paie : 5 000 F/);
  assert.ok(!t.endsWith("\n"), "sans société, pas de ligne vide en fin");
});

test("seule une fiche validée s'envoie", () => {
  assert.equal(raisonBulletin(fiche), null);
  assert.match(raisonBulletin({ ...fiche, statut: "brouillon" })!, /Brouillon/);
  assert.match(raisonBulletin({ ...fiche, statut: "remplacee", remplacee_par: "PAIE-2026-00013" })!, /remplacée par PAIE-2026-00013/);
});

test("le message au client commence par bonjour, et il faut écrire la suite", () => {
  assert.equal(debutMessageClient(" Awa Diarra "), "Bonjour Awa Diarra,\n\n");
  assert.equal(messageEcrit("Bonjour Awa Diarra,\n\n", "Awa Diarra"), false);
  assert.equal(messageEcrit("  ", "Awa Diarra"), false);
  assert.equal(messageEcrit("Bonjour Awa Diarra,\n\nVotre commande est arrivée.", "Awa Diarra"), true);
});
