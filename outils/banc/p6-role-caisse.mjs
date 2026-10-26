// Monoposte / multiposte (v3, D22/D24 revises le 25/10/2026) : le role
// d'un poste, dans la vraie fenetre.
//
// 1. Un poste neuf dit « caisse d'un autre poste » : l'ecran de
//    branchement (adresse essayee avant d'etre enregistree), poste.json
//    en « poste ».
// 2. Le serveur lance comme par le raccourci de demarrage (sans --base)
//    s'efface sur la caisse d'une AUTRE machine — et tourne sur le poste
//    principal, meme regle « poste » sur 127.0.0.1 (le reglage d'avant
//    la revision). Joue sur un dossier a part, port 7301.
// 3. Paramètres → Réseau : la caisse redevient le poste qui garde la base
//    par un geste volontaire ; la fenetre rejoint son serveur.
//
//   apres p1 : node p6-role-caisse.mjs
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { fenetre, boitesMuettes, boites, arreterPort, connecter, parametres, capture, verifieur, erreursUtiles, sante, TRAVAIL, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const DONNEES = path.join(TRAVAIL, "monoposte");
const POSTE = path.join(DONNEES, "poste.json");
const lirePoste = () => JSON.parse(fs.readFileSync(POSTE, "utf8"));
const RACINE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SERVEUR = path.join(RACINE, "src-tauri", "target", "debug", "gescom-serveur.exe");

const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);

// ---- 0. Le serveur de cette machine joue le « poste principal » de la
// caisse : s'il ne tourne pas (p6 joue seul, poste laisse en caisse), la
// fenetre le relance en gardant la base.
if (!(await sante())) {
  fs.writeFileSync(POSTE, JSON.stringify({ ...lirePoste(), mode: "monoposte", serveur: "127.0.0.1:7300" }, null, 2));
  await page.reload();
  for (let i = 0; i < 80 && !(await sante()); i++) await page.waitForTimeout(500);
}
if (!(await sante())) throw new Error("Aucun serveur sur 127.0.0.1:7300 : lancer p1 d'abord.");

// ---- 1. Un poste neuf : « caisse d'un autre poste ».
const avant = lirePoste();
fs.writeFileSync(POSTE, JSON.stringify({ ...avant, mode: "a_choisir", serveur: "" }, null, 2));
await page.evaluate(() => localStorage.clear());
await page.reload();
await page.getByText("Comment travaille ce poste ?").waitFor({ timeout: 20000 }).catch(() => {});
erreurs.splice(0);
ok(await page.getByText("Comment travaille ce poste ?").count() === 1, "rôle à choisir : la fenêtre le demande");
await page.getByRole("button", { name: /Ce poste est une caisse d'un autre poste/ }).click();
await page.getByText("Brancher ce poste").waitFor({ timeout: 10000 }).catch(() => {});
ok(await page.getByText("Brancher ce poste").count() === 1, "« caisse » : l'écran de branchement");
ok(await page.getByRole("button", { name: /ce poste garde la base/ }).count() === 1, "avec le chemin de retour « ce poste garde la base »");
ok(await page.getByRole("button", { name: "Enregistrer" }).isDisabled(), "« Enregistrer » fermé avant un essai");
await page.getByPlaceholder("192.168.1.10:7300").fill("127.0.0.1:7300");
await page.getByPlaceholder("Caisse 1").fill("Caisse banc");
await page.getByRole("button", { name: "Essayer" }).click();
await page.getByText("Serveur joignable").waitFor({ timeout: 10000 }).catch(() => {});
await page.getByRole("button", { name: "Enregistrer" }).click();
await page.getByText("Pseudo ou email").waitFor({ timeout: 10000 }).catch(() => {});
ok(await page.getByText("Pseudo ou email").count() > 0, "essayée puis enregistrée : la connexion");
const p = lirePoste();
ok(p.mode === "poste" && p.serveur === "127.0.0.1:7300" && p.poste_empreinte === avant.poste_empreinte,
  `poste.json : caisse de ${p.serveur}, même empreinte`);
await capture(page, "p6-01-caisse");

// ---- 2. Le serveur du raccourci : il s'efface sur une caisse.
const ROLE = path.join(TRAVAIL, "role-poste");
fs.rmSync(ROLE, { recursive: true, force: true });
fs.mkdirSync(ROLE, { recursive: true });
// Le port 7301 libre : un serveur d'un essai precedent peut le tenir encore.
arreterPort(7301);
const raccourci = (serveur) => new Promise(resolve => {
  fs.writeFileSync(path.join(ROLE, "poste.json"), JSON.stringify({ mode: "poste", serveur, poste_nom: "C", poste_empreinte: "e" }));
  // Sans --base : comme le raccourci. GESCOM_DONNEES (debogage) au lieu
  // de %APPDATA% : jamais la base du proprietaire.
  const enfant = spawn(SERVEUR, ["--port", "7301", "--hote", "127.0.0.1"], { env: { ...process.env, GESCOM_DONNEES: ROLE }, stdio: "pipe" });
  let sortie = "";
  enfant.stdout.on("data", d => sortie += d);
  enfant.stderr.on("data", d => sortie += d);
  const minuterie = setTimeout(() => resolve({ tourne: true, enfant, sortie }), 8000);
  enfant.on("exit", code => { clearTimeout(minuterie); resolve({ tourne: false, code, sortie }); });
});
const autre = await raccourci("192.0.2.50:7300");
ok(!autre.tourne && autre.code === 0, `caisse de 192.0.2.50 : le serveur s'arrête de lui-même (code ${autre.code})`);
const journal = fs.existsSync(path.join(ROLE, "gescom.log")) ? fs.readFileSync(path.join(ROLE, "gescom.log"), "utf8") : "";
ok(journal.includes("Serveur non démarré") && journal.includes("192.0.2.50"), "et son journal dit pourquoi");
ok(!fs.existsSync(path.join(ROLE, "gescom.db")), "sans avoir créé de base");
const ici = await raccourci("127.0.0.1:7300");
ok(ici.tourne && !!(await sante(7301)), "« caisse » de 127.0.0.1 (le poste principal d'avant) : le serveur tourne");
ici.enfant?.kill();
arreterPort(7301);

// ---- 3. Paramètres → Réseau : redevenir le poste qui garde la base.
await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
await parametres(page, "Réseau");
const role = page.getByTestId("role-poste");
ok((await role.innerText()).includes("Poste caisse"), "Réseau : ce poste est une caisse");
// La confirmation est une boite native (`ask`) : on s'assure que
// l'intercepteur est pose sur CETTE page avant de cliquer — sinon la
// vraie boite Windows s'ouvre et attend un clic que personne ne fait.
await boitesMuettes(page);
await role.getByRole("button", { name: "Ce poste garde la base" }).click();
// La confirmation (ask) dit oui ; poste.json change, puis la fenetre se
// recharge et rejoint son serveur.
for (let i = 0; i < 30 && lirePoste().mode !== "monoposte"; i++) await page.waitForTimeout(500);
if (lirePoste().mode !== "monoposte") await capture(page, "p6-echec-garde-la-base");
ok((await boites(page)).some(b => b.message.includes("garder sa propre base")),
  "la confirmation est demandée (les ventes du poste principal ne suivent pas)");
for (let i = 0; i < 80; i++) {
  await page.waitForTimeout(500);
  if (await page.getByText("Démarrage du serveur de ce poste…").count() === 0
    && (await page.getByText("Pseudo ou email").count() || await page.getByText("Tableau de bord", { exact: true }).count())) break;
}
await page.waitForTimeout(2000);
ok(lirePoste().mode === "monoposte" && lirePoste().serveur === "127.0.0.1:7300", "poste.json : ce poste garde la base");
if (await page.getByText("Pseudo ou email").count()) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
await parametres(page, "Réseau");
const texte = await page.getByTestId("role-poste").innerText();
ok(texte.includes("Ce poste garde la base") && texte.includes("127.0.0.1:7300"), "Réseau le dit, avec l'adresse du serveur de cette machine");
await capture(page, "p6-02-garde-la-base");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
