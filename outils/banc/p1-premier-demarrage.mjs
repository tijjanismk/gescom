// Monoposte (v3, D22/D24 revises le 25/10/2026) : la boutique a une
// caisse, telle qu'elle s'installe — Gescom seul sur la machine. Au
// premier demarrage la vraie fenetre Tauri demande le role du poste,
// une fois, sans le deviner ; « Ce poste garde la base » : elle lance
// elle-meme son serveur (aucun ne tournait), s'y branche sur 127.0.0.1,
// rien a saisir. Connexion du patron (mot de passe d'usine change), un
// seul dossier qu'on ne choisit pas. Un cache vide ne redemande rien
// (poste.json est la source). La base du proprietaire
// (%APPDATA%\ml.gescom.app) n'a pas ete touchee.
//
//   .\lancer.ps1 monoposte -Neuf ; node p1-premier-demarrage.mjs
import fs from "node:fs";
import path from "node:path";
import { fenetre, boitesMuettes, connecter, capture, verifieur, erreursUtiles, sante, TRAVAIL, URL_FENETRE } from "./pw.mjs";

const ok = verifieur();
const DONNEES = path.join(TRAVAIL, "monoposte");
const PROPRIO = path.join(process.env.APPDATA ?? "", "ml.gescom.app");
const empreinte = (dossier) => ["gescom.db", "gescom.db-wal", "poste.json"]
  .map(f => path.join(dossier, f)).filter(f => fs.existsSync(f))
  .map(f => `${path.basename(f)}:${fs.statSync(f).size}:${fs.statSync(f).mtimeMs}`).join(" ");
const proprioAvant = empreinte(PROPRIO);

const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
ok(await page.evaluate(() => "__TAURI_INTERNALS__" in window), "c'est la fenêtre Tauri, pas un navigateur");

// ---- Le dossier isole : la fenetre y a pose sa base et poste.json.
const lirePoste = () => JSON.parse(fs.readFileSync(path.join(DONNEES, "poste.json"), "utf8"));
ok(fs.existsSync(path.join(DONNEES, "gescom.db")), "GESCOM_DONNEES suivi : la fenêtre a ouvert sa base dans .travail/monoposte");
// L'ecran ecrit poste.json a son premier demarrage : a froid (Vite
// compile encore), il arrive apres la fenetre — on attend l'ecran.
await page.getByText("Comment travaille ce poste ?").waitFor({ timeout: 30000 }).catch(() => {});
ok(await page.getByText("Comment travaille ce poste ?").count() === 1, "premier démarrage : la fenêtre demande le rôle du poste");
ok(fs.existsSync(path.join(DONNEES, "poste.json")) && lirePoste().mode === "a_choisir", "poste.json neuf, rôle « à choisir » (rien n'est deviné)");
ok(await sante() === null, "aucun serveur ne tourne encore sur 127.0.0.1:7300");
await capture(page, "p1-01-role");

// ---- « Ce poste garde la base » : la fenetre lance son serveur.
const t0 = Date.now();
await page.getByRole("button", { name: /Ce poste garde la base/ }).click();
// A froid, le serveur de debogage amorce sa base neuve (comptes, demo) :
// une dizaine de secondes. La fenetre attend jusqu'a vingt ; si elle
// abandonne, son ecran d'erreur s'afficherait ici au lieu de la connexion.
await page.getByText("Pseudo ou email").or(page.getByTestId("erreur-serveur-local"))
  .first().waitFor({ timeout: 90000 }).catch(() => {});
const duree = Math.round((Date.now() - t0) / 1000);
ok(await page.getByText("Pseudo ou email").count() > 0,
  `« garde la base » : la page de connexion en ${duree} s, sans adresse à saisir`
  + (await page.getByTestId("erreur-serveur-local").count() ? ` — écran d'erreur : ${await page.getByTestId("erreur-serveur-local").innerText()}` : ""));
ok((await page.locator("body").innerText()).includes("Ce poste garde la base — serveur de cette machine"),
  "la connexion dit que ce poste garde la base (pas « poste caisse »)");
const s = await sante();
ok(!!s && s.base_saine, `la fenêtre a lancé son serveur : il répond (${s?.version_serveur ?? "rien"})`);
const poste = lirePoste();
ok(poste.mode === "monoposte" && poste.serveur === "127.0.0.1:7300", `poste.json réécrit (${poste.mode}, ${poste.serveur})`);
ok(fs.existsSync(path.join(DONNEES, "serveur.db")), "le serveur tient sa base dans le dossier isolé (serveur.db)");

// ---- La connexion du patron, un seul dossier.
let choix = false;
const surveiller = setInterval(async () => { if (await page.getByText("Quel dossier ouvrir ?").count().catch(() => 0)) choix = true; }, 300);
await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_FENETRE);
clearInterval(surveiller);
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "le patron est connecté : le tableau de bord");
ok(!choix, "un seul dossier : rien à choisir");
const etat = await page.evaluate(async () => (await import("/src/lib/pont.ts")).etatReseau());
ok(etat.mode === "monoposte" && etat.serveur === "127.0.0.1:7300" && !!etat.jeton && !!etat.dossierId,
  "le pont parle au serveur de cette machine, jeton et dossier posés");
const dossiers = await page.evaluate(async () => (await (await import("/src/lib/pont.ts")).appeler("lire_dossiers", {})).length);
ok(dossiers === 1, `un dossier sur ce serveur (${dossiers})`);
await capture(page, "p1-02-accueil");

// ---- Cache vide : la fenetre relit poste.json, ne redemande rien.
// D'abord laisser l'accueil finir ses lectures : un rechargement en plein
// vol les coupe, et l'ecran les dit « Serveur injoignable » (c'est le test
// qui coupe, pas le serveur — son journal ne montre rien).
await page.waitForTimeout(4000);
await page.evaluate(() => localStorage.clear());
await page.reload();
await page.waitForTimeout(3000);
ok(await page.getByText("Comment travaille ce poste ?").count() === 0 && await page.getByText("Brancher ce poste").count() === 0,
  "cache vidé : ni le rôle ni l'adresse ne sont redemandés");
// Le jeton vivait dans le cache : on se reconnecte.
if (await page.getByText("Pseudo ou email").count()) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "reconnecté");
const s2 = await sante();
ok(s2?.postes_connectes >= 1, `le serveur compte ce poste (${s2?.postes_connectes})`);

ok(empreinte(PROPRIO) === proprioAvant, "la base et poste.json du propriétaire n'ont pas bougé");
const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
