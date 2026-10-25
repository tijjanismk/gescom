// Monoposte (v3) : la boutique a une caisse, telle qu'elle s'installe —
// le serveur SQLite et la fenetre sur la MEME machine (ARCHITECTURE.md).
// Le premier demarrage de la vraie fenetre Tauri : elle ne devine rien,
// elle demande ou est le serveur ; 127.0.0.1 essaye puis enregistre dans
// poste.json ; connexion du patron (mot de passe d'usine change) ; un
// seul dossier, qu'on ne choisit pas. Un cache vide ne la renvoie pas au
// branchement (poste.json est la source). Et la base du proprietaire
// (%APPDATA%\ml.gescom.app) n'a pas ete touchee.
//
//   .\lancer.ps1 monoposte -Neuf ; node p1-premier-demarrage.mjs
import fs from "node:fs";
import path from "node:path";
import { fenetre, boitesMuettes, connecter, capture, verifieur, erreursUtiles, TRAVAIL, URL_FENETRE } from "./pw.mjs";

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
await page.getByText("Brancher ce poste").waitFor({ timeout: 30000 }).catch(() => {});
ok(fs.existsSync(path.join(DONNEES, "poste.json")) && lirePoste().mode === "monoposte", "poste.json neuf, mode « monoposte » (le défaut)");

// ---- Le branchement.
ok(await page.getByText("Brancher ce poste").count() === 1, "sans serveur connu, la fenêtre demande où il est");
await capture(page, "p1-01-brancher");
await page.getByPlaceholder("192.168.1.10:7300").fill(`127.0.0.1:${process.env.PORT_SERVEUR ?? 7300}`);
await page.getByPlaceholder("Caisse 1").fill("Caisse banc");
ok(await page.getByRole("button", { name: "Enregistrer" }).isDisabled(), "« Enregistrer » fermé avant un essai");
await page.getByRole("button", { name: "Essayer" }).click();
await page.getByText("Serveur joignable").waitFor({ timeout: 10000 }).catch(() => {});
ok(await page.getByText("Serveur joignable").count() === 1, "« Essayer » : serveur joignable (tester_serveur, côté Rust)");
await page.getByRole("button", { name: "Enregistrer" }).click();
await page.getByText("Pseudo ou email").waitFor({ timeout: 10000 }).catch(() => {});
ok(await page.getByText("Pseudo ou email").count() > 0, "enregistré : la page de connexion");
const poste = lirePoste();
ok(poste.mode === "poste" && poste.serveur.startsWith("127.0.0.1:") && poste.poste_nom === "Caisse banc",
  `poste.json réécrit (${poste.mode}, ${poste.serveur}, ${poste.poste_nom})`);

// ---- La connexion du patron, un seul dossier.
let choix = false;
const surveiller = setInterval(async () => { if (await page.getByText("Quel dossier ouvrir ?").count().catch(() => 0)) choix = true; }, 300);
await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_FENETRE);
clearInterval(surveiller);
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "le patron est connecté : le tableau de bord");
ok(!choix, "un seul dossier : rien à choisir");
const etat = await page.evaluate(async () => (await import("/src/lib/pont.ts")).etatReseau());
ok(etat.mode === "poste" && !!etat.jeton && !!etat.dossierId, "le pont parle au serveur, jeton et dossier posés");
const dossiers = await page.evaluate(async () => (await (await import("/src/lib/pont.ts")).appeler("lire_dossiers", {})).length);
ok(dossiers === 1, `un dossier sur ce serveur (${dossiers})`);
await capture(page, "p1-02-accueil");

// ---- Cache vide : la fenetre relit poste.json, ne redemande pas le serveur.
// D'abord laisser l'accueil finir ses lectures : un rechargement en plein
// vol les coupe, et l'ecran les dit « Serveur injoignable » (c'est le test
// qui coupe, pas le serveur — son journal ne montre rien).
await page.waitForTimeout(4000);
await page.evaluate(() => localStorage.clear());
await page.reload();
await page.waitForTimeout(2500);
ok(await page.getByText("Brancher ce poste").count() === 0, "cache vidé : pas de retour au branchement");
// Le jeton vivait dans le cache : on se reconnecte (sans recharger,
// pour constater ce que la fenetre montre d'elle-meme).
if (await page.getByText("Pseudo ou email").count()) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "reconnecté");

// ---- Le serveur voit ce poste.
const sante = await (await fetch(`http://127.0.0.1:${process.env.PORT_SERVEUR ?? 7300}/sante`)).json();
ok(sante.postes_connectes >= 1, `le serveur compte ce poste (${sante.postes_connectes})`);

ok(empreinte(PROPRIO) === proprioAvant, "la base et poste.json du propriétaire n'ont pas bougé");
const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
