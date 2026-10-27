// Monoposte (v3, D22/D24 revises le 25/10/2026) : le serveur de ce
// poste s'est arrete (fenetre noire fermee, plantage) — la fenetre le
// relance toute seule a son ouverture, sans rien demander. Un serveur
// qui redemarre ferme les sessions d'avant (sessions.rs) : la fenetre
// revient a la connexion, sur la meme base.
// Quand 40000 est pris par un autre programme, le serveur prend le port
// suivant libre de la plage 40000-40100 et la fenetre le retient (D39,
// 27/10/2026) : jamais un ecran qui echouerait au premier geste.
//
//   apres p1 : node p5-serveur-relance.mjs
import http from "node:http";
import { fenetre, boitesMuettes, connecter, capture, verifieur, erreursUtiles, sante, arreterPort, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
if (await page.getByText("Comment travaille ce poste ?").count()) throw new Error("Le rôle n'est pas choisi : lancer p1 d'abord.");
if (await page.getByText("Pseudo ou email").count()) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
const DEMARRAGE = "Démarrage du serveur de ce poste…";
/** Attendre que la fenetre ait fini de joindre (ou lancer) son serveur. */
const serveurJoint = async () => {
  for (let i = 0; i < 80; i++) {
    await page.waitForTimeout(500);
    if (await page.getByText(DEMARRAGE).count() === 0 && await sante()) break;
  }
  await page.waitForTimeout(2500);
};
const articles = () => page.evaluate(async () => (await (await import("/src/lib/pont.ts")).appeler("lire_articles_avec_unites", {})).length);

// ---- Le serveur tombe ; la fenetre se rouvre : elle le relance.
ok(!!(await sante()), "au départ, le serveur de ce poste répond");
const avant = await articles();
await page.waitForTimeout(3000); // l'accueil a fini ses lectures
arreterPort(40000);
ok(await sante() === null, "serveur arrêté");
await page.reload();
await serveurJoint();
erreurs.splice(0); // ce qui s'est dit pendant que le serveur etait mort
ok(!!(await sante()), "à la réouverture, la fenêtre a relancé le serveur, sans rien demander");
await page.getByText("Pseudo ou email").waitFor({ timeout: 15000 }).catch(() => {});
erreurs.splice(0); // le premier appel sur l'ancien jeton (401)
ok(await page.getByText("Pseudo ou email").count() > 0, "les sessions d'avant sont fermées : retour à la connexion");
await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
ok(await articles() === avant, `reconnecté, la même base (${avant} articles)`);

// ---- 40000 pris par un autre programme : le serveur prend 40001 (D39).
await page.waitForTimeout(3000);
arreterPort(40000);
const intrus = http.createServer((_, res) => { res.writeHead(200, { "Content-Type": "text/plain" }); res.end("ceci n'est pas Gescom"); });
await new Promise(r => intrus.listen(40000, "127.0.0.1", r));
await page.reload();
for (let i = 0; i < 80; i++) {
  await page.waitForTimeout(500);
  if (await page.getByText(DEMARRAGE).count() === 0 && await sante(40001)) break;
}
await page.waitForTimeout(2500);
erreurs.splice(0);
ok(!!(await sante(40001)), "40000 pris : le serveur de ce poste a pris 40001, sans rien demander");
ok(await page.getByTestId("erreur-serveur-local").count() === 0, "aucun écran d'erreur");
await page.getByText("Pseudo ou email").waitFor({ timeout: 15000 }).catch(() => {});
erreurs.splice(0);
await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "connecté sur 40001 : le tableau de bord vient");
ok(await articles() === avant, "et les données avec : la même base");
await capture(page, "p5-01-port-suivant");

// ---- L'intrus parti, le port retenu reste 40001 : le serveur y revient.
await new Promise(r => intrus.close(r));
await page.waitForTimeout(3000);
arreterPort(40001);
await page.reload();
for (let i = 0; i < 80; i++) {
  await page.waitForTimeout(500);
  if (await page.getByText(DEMARRAGE).count() === 0 && await sante(40001)) break;
}
erreurs.splice(0);
ok(!!(await sante(40001)) && (await sante(40000)) === null, "relancé sur le port retenu, 40001");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
