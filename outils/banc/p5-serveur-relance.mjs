// Monoposte (v3, D22/D24 revises le 25/10/2026) : le serveur de ce
// poste s'est arrete (fenetre noire fermee, plantage) — la fenetre le
// relance toute seule a son ouverture, sans rien demander. Un serveur
// qui redemarre ferme les sessions d'avant (sessions.rs) : la fenetre
// revient a la connexion, sur la meme base.
// Quand elle ne peut pas (le port 7300 pris par un autre programme),
// elle le dit, propose de reessayer, et repart des que la place est
// libre : jamais un ecran qui echouerait au premier geste.
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
arreterPort(7300);
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

// ---- Le port pris par un autre programme : la fenetre le dit.
await page.waitForTimeout(3000);
arreterPort(7300);
const intrus = http.createServer((_, res) => { res.writeHead(200, { "Content-Type": "text/plain" }); res.end("ceci n'est pas Gescom"); });
await new Promise(r => intrus.listen(7300, "127.0.0.1", r));
await page.reload();
const erreur = page.getByTestId("erreur-serveur-local");
await erreur.waitFor({ timeout: 30000 }).catch(() => {});
erreurs.splice(0);
ok(await page.getByText("Le serveur de ce poste ne répond pas").count() === 1, "port pris : l'écran dit que le serveur ne répond pas");
const texte = await erreur.innerText().catch(() => "");
ok(/pris par un autre programme/.test(texte), `et pourquoi (« ${texte.slice(0, 90)}… »)`);
ok(await page.getByText("Pseudo ou email").count() === 0 && await page.getByText("Tableau de bord", { exact: true }).count() === 0,
  "ni connexion ni tableau de bord qui échoueraient");
ok(await page.getByRole("button", { name: "Ce poste est une caisse d'un autre poste" }).count() === 1, "il propose aussi le branchement à un autre poste");
await capture(page, "p5-01-port-pris");

// ---- La place se libere : « Reessayer » repart.
await new Promise(r => intrus.close(r));
await page.getByRole("button", { name: /Réessayer/ }).click();
await serveurJoint();
erreurs.splice(0);
ok(!!(await sante()), "« Réessayer » : le serveur est lancé");
await page.getByText("Pseudo ou email").waitFor({ timeout: 15000 }).catch(() => {});
erreurs.splice(0);
await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "reconnecté : le tableau de bord revient");
ok(await articles() === avant, "et les données avec");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
