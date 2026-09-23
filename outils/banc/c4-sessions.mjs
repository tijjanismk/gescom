// C-4 : Parametres -> Utilisateurs montre qui est connecte, sur quel
// poste, sa derniere commande ; « Deconnecter » renvoie le poste a la
// connexion ; « Desactiver » ferme ses sessions dans le meme geste.
import { chromium } from "playwright";
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles, URL } from "./pw.mjs";
const { b, page: patron, erreurs } = await navigateur();
const ok = verifieur();

// La caisse B : un second navigateur, un autre poste.
const ctxB = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
await ctxB.addInitScript(() => {
  if (!localStorage.getItem("gescom_reseau")) {
    localStorage.setItem("gescom_reseau", JSON.stringify({ posteNom: "Caisse B", posteEmpreinte: "banc-caisse-b" }));
  }
});
const caisse = await ctxB.newPage();
const MDP_EMPLOYE = "Employe-2026!";
await connecter(caisse, "employe", "employe123", MDP_EMPLOYE);
await caisse.getByText("Stock", { exact: true }).first().click();
await caisse.waitForTimeout(1200);
const surLaConnexion = async () => (await caisse.locator('input[type="password"]').count()) > 0
  && (await caisse.getByText("Tableau de bord", { exact: true }).count()) === 0;
ok(!(await surLaConnexion()), "la caisse B est connectée");
// Un geste de la caisse : si elle n'est pas deja revenue d'elle-meme a
// la connexion (le canal l'y ramene des qu'il recoit un 401).
const geste = async () => {
  const lien = caisse.getByText("Tableau de bord", { exact: true });
  if (await lien.count()) await lien.first().click();
  await caisse.waitForTimeout(2000);
};

await connecter(patron);
await parametres(patron, "Utilisateurs");
const sessions = patron.getByTestId("session");
const ligneB = sessions.filter({ hasText: "Caisse B" });
ok(await ligneB.count() === 1, "la session de la caisse B est listée");
const texteB = await ligneB.innerText();
ok(texteB.includes("Employé") && /depuis/.test(texteB), "qui, sur quel poste, depuis quand");
const derniere = texteB.match(/dernière action [^(]*\(([^)]+)\)/);
ok(!!derniere && derniere[1].startsWith("lire"), `et sa dernière commande (${derniere?.[1]})`);
// Ce poste (le navigateur du patron) peut porter plusieurs sessions
// des parcours precedents : aucune n'a de bouton.
const miennes = sessions.filter({ hasText: "ce poste" });
ok(await miennes.count() >= 1 && await miennes.getByRole("button").count() === 0, "ses propres sessions : « ce poste », pas de bouton");
ok(await patron.getByRole("button", { name: /^Désactiver Patron/ }).count() === 0, "pas de « Désactiver » sur son propre compte");
await capture(patron, "c4-01-sessions");

// Deconnecter.
await ligneB.getByRole("button", { name: /Déconnecter/ }).click();
await patron.waitForTimeout(1200);
ok(await sessions.filter({ hasText: "Caisse B" }).count() === 0, "Déconnecter : la session quitte la liste");
await geste();
ok(await surLaConnexion(), "la caisse B revient à l'écran de connexion");

// Reconnexion, puis desactivation du compte.
await caisse.evaluate(() => { const r = JSON.parse(localStorage.getItem("gescom_reseau")); localStorage.clear(); localStorage.setItem("gescom_reseau", JSON.stringify({ ...r, jeton: null })); });
await connecter(caisse, "employe", MDP_EMPLOYE, MDP_EMPLOYE);
ok(!(await surLaConnexion()), "la caisse B se reconnecte");
await patron.getByRole("button", { name: "Actualiser les sessions" }).click();
await patron.waitForTimeout(1000);
ok(await sessions.filter({ hasText: "Caisse B" }).count() === 1, "de nouveau listée");
await patron.getByRole("button", { name: "Désactiver Employé" }).click();
await patron.waitForTimeout(1500);
// Au moins la session de la caisse B (d'autres parcours ont pu en
// laisser une ouverte pour ce compte).
ok(/Employé est désactivé — \d+ sessions? fermées?\./.test(await patron.getByRole("status").innerText()), "« désactivé — N session(s) fermée(s) »");
ok(await sessions.filter({ hasText: "Caisse B" }).count() === 0, "la session est partie avec le compte");
ok(await patron.getByRole("button", { name: "Réactiver Employé" }).count() === 1, "le compte est marqué inactif");
await capture(patron, "c4-02-desactive");
await geste();
ok(await surLaConnexion(), "la caisse B est renvoyée à la connexion");
await caisse.evaluate(() => localStorage.removeItem("gescom_session"));
await caisse.locator("input").nth(0).fill("employe");
await caisse.locator('input[type="password"]').first().fill(MDP_EMPLOYE);
await caisse.keyboard.press("Enter");
await caisse.waitForTimeout(2000);
ok(await surLaConnexion(), "un compte désactivé ne se reconnecte pas");

// Remise en etat.
await patron.getByRole("button", { name: "Réactiver Employé" }).click();
await patron.waitForTimeout(1200);
ok(/Employé est réactivé\./.test(await patron.getByRole("status").innerText()), "réactivé");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
