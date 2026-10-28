// Monoposte (v3, D22/D24 revises le 25/10/2026) : la boutique a une
// caisse, telle qu'elle s'installe — Gescom seul sur la machine. Au
// premier demarrage la vraie fenetre Tauri demande le role du poste,
// une fois, sans le deviner ; « Ce poste garde la base » : elle lance
// elle-meme son serveur (aucun ne tournait), s'y branche sur 127.0.0.1,
// rien a saisir. Base neuve (D43) : la connexion dit le compte d'usine,
// l'assistant cree le dossier (le patron, la societe, les dates, les
// reglages, une vendeuse), puis le tableau de bord ; un seul dossier
// qu'on ne choisit pas. Un cache vide ne redemande rien (poste.json est
// la source) ; on se reconnecte avec le NOUVEAU compte. La base du
// proprietaire (%APPDATA%\ml.gescom.app) n'a pas ete touchee.
//
//   .\lancer.ps1 monoposte -Neuf -SansDemo ; node p1-premier-demarrage.mjs
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

// ---- Le dossier isole : la fenetre y pose poste.json, et aucune base
// (lot E.13, 27/10/2026 : elle n'ouvre plus de gescom.db a elle).
const lirePoste = () => JSON.parse(fs.readFileSync(path.join(DONNEES, "poste.json"), "utf8"));
// L'ecran ecrit poste.json a son premier demarrage : a froid (Vite
// compile encore), il arrive apres la fenetre — on attend l'ecran.
await page.getByText("Comment travaille ce poste ?").waitFor({ timeout: 30000 }).catch(() => {});
ok(await page.getByText("Comment travaille ce poste ?").count() === 1, "premier démarrage : la fenêtre demande le rôle du poste");
ok(fs.existsSync(path.join(DONNEES, "poste.json")) && lirePoste().mode === "a_choisir",
  "GESCOM_DONNEES suivi : poste.json neuf dans .travail/monoposte, rôle « à choisir » (rien n'est deviné)");
ok(!fs.existsSync(path.join(DONNEES, "gescom.db")), "la fenêtre n'a ouvert aucune base à elle (pas de gescom.db)");
ok(await sante() === null, "aucun serveur ne tourne encore sur 127.0.0.1:40000");
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
ok(poste.mode === "monoposte" && poste.serveur === "127.0.0.1:40000", `poste.json réécrit (${poste.mode}, ${poste.serveur})`);
ok(fs.existsSync(path.join(DONNEES, "serveur.db")), "le serveur tient sa base dans le dossier isolé (serveur.db)");

// ---- Base neuve (D43) : la connexion dit le compte d'usine.
ok(s.assistant_a_faire === true, "le serveur annonce une base neuve (/sante : assistant à faire)");
ok(await page.getByText("Première ouverture de Gescom").count() === 1, "la connexion dit « admin / admin123 » sur une base neuve");
let choix = false;
const surveiller = setInterval(async () => { if (await page.getByText("Quel dossier ouvrir ?").count().catch(() => 0)) choix = true; }, 300);
await page.locator("#identifiant").fill("admin");
await page.locator("#mot_de_passe").fill("admin123");
await page.keyboard.press("Enter");
await page.getByText("création du dossier").waitFor({ timeout: 30000 }).catch(() => {});
ok(await page.getByText("création du dossier").count() > 0, "connecté : l'assistant prend l'écran, pas le tableau de bord");
await capture(page, "p1-02-assistant");

// ---- L'assistant, étape par étape.
const suivant = async () => { await page.getByRole("button", { name: /^(Commencer|Suivant)/ }).click(); await page.waitForTimeout(300); };
await suivant();
await page.locator("#patron-nom").fill("Moussa Traoré");
await page.locator("#patron-pseudo").fill("moussa");
await page.locator("#patron-mdp").fill("admin123");
await page.locator("#patron-mdp2").fill("admin123");
ok(await page.getByRole("button", { name: /^Suivant/ }).isDisabled(), "le mot de passe d'usine ne se garde pas");
await page.locator("#patron-mdp").fill("Bamako-2026");
await page.locator("#patron-mdp2").fill("Bamako-2026");
await suivant();
await page.locator("#soc-nom").fill("Quincaillerie Traoré");
await page.locator("#soc-adresse").fill("Marché de Médine, Bamako");
await page.locator("#soc-tel").fill("76 00 00 00");
await page.locator("#soc-nif").fill("084123456M");
await capture(page, "p1-03-societe");
await suivant();
ok((await page.locator("body").innerText()).includes("1er janvier"), "les dates proposées, dites en clair");
await capture(page, "p1-04-dates");
await suivant();
await page.locator("#magasin").fill("Boutique Médine");
await page.locator("#fond").fill("25000");
await capture(page, "p1-05-reglages");
await suivant();
await page.getByRole("button", { name: /Ajouter un vendeur/ }).click();
await page.locator("#v-nom-0").fill("Awa Diallo");
await page.locator("#v-pseudo-0").fill("awa");
await page.locator("#v-mdp-0").fill("awa-2026");
await suivant();
ok((await page.locator("body").innerText()).includes("Quincaillerie Traoré"), "le récapitulatif reprend la société");
await capture(page, "p1-06-recapitulatif");
await page.getByRole("button", { name: /Créer mon dossier/ }).click();
await page.getByText(/Le dossier .* est créé/).waitFor({ timeout: 30000 }).catch(() => {});
ok(await page.getByText(/Le dossier .* est créé/).count() > 0, "« Créer mon dossier » : créé, la reprise des données est proposée");
await capture(page, "p1-07-reprise");
await page.getByRole("button", { name: /Commencer à travailler/ }).click();
await page.getByText("Tableau de bord", { exact: true }).first().waitFor({ timeout: 20000 }).catch(() => {});
clearInterval(surveiller);
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "le patron arrive au tableau de bord");
ok(await page.locator('input[type="password"]').count() === 0, "plus de fenêtre « changer le mot de passe » par-dessus");
ok(!choix, "un seul dossier : rien à choisir");
ok((await sante())?.assistant_a_faire === false, "le serveur sert tout, sans redémarrer");
const etat = await page.evaluate(async () => (await import("/src/lib/pont.ts")).etatReseau());
ok(etat.mode === "monoposte" && etat.serveur === "127.0.0.1:40000" && !!etat.jeton && !!etat.dossierId,
  "le pont parle au serveur de cette machine, jeton et dossier posés");
const dossiers = await page.evaluate(async () => (await (await import("/src/lib/pont.ts")).appeler("lire_dossiers", {})));
ok(dossiers.length === 1 && dossiers[0].societe === "Quincaillerie Traoré", `un dossier, au nom de la société (${dossiers.map(d => d.societe)})`);
await capture(page, "p1-08-accueil");

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
  ok(await page.getByText("Première ouverture de Gescom").count() === 0, "la connexion ne parle plus du compte d'usine");
  await connecter(page, "moussa", "Bamako-2026", "Bamako-2026", "Quincaillerie Traoré", URL_FENETRE);
}
ok(await page.getByText("Tableau de bord", { exact: true }).count() > 0, "reconnecté");
const s2 = await sante();
ok(s2?.postes_connectes >= 1, `le serveur compte ce poste (${s2?.postes_connectes})`);

ok(!fs.existsSync(path.join(DONNEES, "gescom.db")), "toujours aucun gescom.db : la base est celle du serveur (serveur.db)");
ok(empreinte(PROPRIO) === proprioAvant, "la base et poste.json du propriétaire n'ont pas bougé");
const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
