// Monoposte (v3) : la sauvegarde automatique (correctif du 25/09) part
// toute seule peu apres le demarrage du serveur, sans attendre 24 h de
// marche — un poste de boutique s'eteint le soir. Ici le premier
// controle est a 20 s (GESCOM_SAUVEGARDE_DELAI_S, pose par lancer.ps1
// monoposte). La copie est datee par son nom, et la fenetre la montre
// dans Parametres → Sauvegarde, lue du dossier.
//
//   apres p1 : node p4-sauvegarde-auto.mjs
import fs from "node:fs";
import path from "node:path";
import { fenetre, boitesMuettes, connecter, capture, parametres, verifieur, erreursUtiles, TRAVAIL, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const DOSSIER = path.join(TRAVAIL, "monoposte", "sauvegardes");
const copies = () => (fs.existsSync(DOSSIER) ? fs.readdirSync(DOSSIER) : []).filter(f => f.startsWith("gescom_backup_"));

// La copie arrive seule : on attend jusqu'a 90 s, sans rien demander.
for (let i = 0; i < 90 && copies().length === 0; i++) await new Promise(r => setTimeout(r, 1000));
const liste = copies();
ok(liste.length >= 1, `une copie automatique dans le dossier du serveur (${liste.join(", ") || "aucune"})`);
const aujourdhui = new Date().toISOString().slice(0, 10);
ok(liste.some(f => /^gescom_backup_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}\.db$/.test(f) && f.includes(aujourdhui.slice(0, 7))),
  "nommée à la seconde, datée d'aujourd'hui");
const taille = liste.length ? fs.statSync(path.join(DOSSIER, liste[0])).size : 0;
ok(taille > 100_000, `une vraie base, pas un fichier vide (${taille} octets)`);

const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
if (!(await page.getByText("Tableau de bord", { exact: true }).count())) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
erreurs.splice(0);
await parametres(page, "Sauvegarde");
const carte = await page.getByTestId("etat-copies").innerText();
ok(/Dernière copie le /.test(carte), "Paramètres › Sauvegarde : la date de la dernière copie, en clair");
ok(/\d+ copies? dans le dossier/.test(carte), "et la liste du dossier");
await capture(page, "p4-sauvegarde");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
