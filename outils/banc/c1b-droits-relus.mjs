// Les droits gardés par le navigateur sont relus au démarrage. Une
// session enregistrée avant une mise à jour (ici : sans `rapports:lire`,
// comme avant la v3) montrait au patron « les chiffres de la boutique ne
// sont pas ouverts à votre compte » jusqu'à la reconnexion.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL);
await page.waitForTimeout(1500);
ok(await page.getByTestId("accueil-sans-chiffres").count() === 0, "patron connecté : le tableau de bord chiffré");

// La session du navigateur vieillit : on lui retire une permission.
const retirer = cle => page.evaluate(cle => {
  const s = JSON.parse(localStorage.getItem(cle));
  const u = s.utilisateur ?? s;
  u.permissions = u.permissions.filter(p => p !== "rapports:lire" && p !== "paie:preparer");
  localStorage.setItem(cle, JSON.stringify(s));
  return u.permissions.includes("rapports:lire");
}, cle);
ok(await retirer("gescom_session") === false, "session du navigateur sans « rapports:lire »");
await page.reload();
await page.waitForTimeout(2500);
ok(await page.getByTestId("accueil-sans-chiffres").count() === 0, "rechargé : les droits relus, les chiffres reviennent sans se reconnecter");
const relue = await page.evaluate(() => JSON.parse(localStorage.getItem("gescom_session")).utilisateur.permissions.includes("rapports:lire"));
ok(relue, "la session du navigateur est remise à jour");
await capture(page, "c1b-01-droits-relus");

// Même chose dans Équipe.
await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
await page.waitForTimeout(1200);
ok(await retirer("gescom_equipe_session") === false, "Équipe : session sans « paie:preparer »");
await page.reload();
await page.waitForTimeout(2500);
ok(await page.evaluate(() => JSON.parse(localStorage.getItem("gescom_equipe_session")).permissions.includes("paie:preparer")), "Équipe : les droits relus au démarrage");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
