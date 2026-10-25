// Aides Playwright du banc Gescom : ouvrir, se connecter, capturer.
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { execFileSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

const ICI = path.dirname(fileURLToPath(import.meta.url));
export const TRAVAIL = path.join(ICI, ".travail");
export const CAPTURES = path.join(TRAVAIL, "captures");
fs.mkdirSync(CAPTURES, { recursive: true });
export const URL = `http://127.0.0.1:${process.env.PORT_ECRAN ?? 1420}/?serveur=127.0.0.1:${process.env.PORT_SERVEUR ?? 7300}`;
/** Gescom Équipe (PLAN-EQUIPE) : la seconde entrée du même écran. */
export const URL_EQUIPE = URL.replace("/?", "/equipe.html?");

export async function navigateur() {
  // CHROMIUM : un Chromium deja installe (sinon celui de Playwright).
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
  const page = await ctx.newPage();
  const erreurs = [];
  page.on("pageerror", e => erreurs.push("pageerror: " + e.message));
  page.on("console", m => { if (m.type() === "error") erreurs.push("console: " + m.text()); });
  return { b, ctx, page, erreurs };
}

/** Une ecriture directe dans la base du banc, pour poser ce que l'ecran
 *  ne sait pas provoquer (b4, c1). La base est celle que lancer.ps1 a
 *  notee dans .travail/moteur.txt : un fichier SQLite, ou une URL
 *  PostgreSQL — et alors `gescom_test`, rien d'autre. Sans ce fichier
 *  (lancer.sh), essai.db comme avant. `?` = valeurs, citees ici. */
export function ecrireEnBase(sql, valeurs = []) {
  const f = path.join(TRAVAIL, "moteur.txt");
  const cible = fs.existsSync(f) ? fs.readFileSync(f, "utf8").trim() : path.join(TRAVAIL, "essai.db");
  if (cible.startsWith("postgresql://")) {
    if (!/\/gescom_test(\?|$)/.test(cible)) throw new Error(`Le banc n'écrit que dans gescom_test, pas ${cible}`);
    let i = 0;
    const lit = v => v === null ? "NULL" : typeof v === "number" ? String(v) : `'${String(v).replace(/'/g, "''")}'`;
    const texte = sql.replace(/\?/g, () => lit(valeurs[i++]));
    // Par l'entree standard, en UTF-8 : sur la ligne de commande, Windows
    // passe le texte dans la page de code du systeme et « n° » casse.
    execFileSync(process.env.PSQL ?? "C:\\Program Files\\PostgreSQL\\18\\bin\\psql.exe",
      ["-q", "-v", "ON_ERROR_STOP=1", "-d", cible, "-f", "-"],
      { input: texte, env: { ...process.env, PGCLIENTENCODING: "UTF8" } });
    return;
  }
  const base = new DatabaseSync(cible);
  try { base.prepare(sql).run(...valeurs); } finally { base.close(); }
}

/** La vraie fenetre Tauri (lancer.ps1 monoposte), par le port de
 *  debogage de WebView2. `b.close()` la detache sans la fermer. */
export async function fenetre() {
  const b = await chromium.connectOverCDP(`http://127.0.0.1:${process.env.PORT_CDP ?? 9222}`);
  let page;
  for (let i = 0; i < 40 && !page; i++) {
    page = b.contexts().flatMap(c => c.pages()).find(p => p.url().includes(":1420"));
    if (!page) await new Promise(r => setTimeout(r, 500));
  }
  if (!page) throw new Error("La fenetre Gescom n'a pas de page sur :1420 (lancer.ps1 monoposte ?)");
  const erreurs = [];
  page.on("pageerror", e => erreurs.push("pageerror: " + e.message));
  page.on("console", m => { if (m.type() === "error") erreurs.push("console: " + m.text()); });
  return { b, page, erreurs };
}

/** Les boites natives de la fenetre (`message` de plugin-dialog) sont
 *  des fenetres Windows que Playwright ne peut pas fermer : le geste
 *  resterait suspendu a leur `await`. On les intercepte dans la page —
 *  le texte est garde dans `window.__boites`, relu par `boites(page)` —
 *  et on les rearme a chaque chargement. Rien d'autre n'est touche. */
export async function boitesMuettes(page) {
  const poser = () => page.evaluate(() => {
    const t = window.__TAURI_INTERNALS__;
    if (!t || t.__banc) return;
    const vrai = t.invoke.bind(t);
    t.invoke = (cmd, args, options) => {
      if (cmd === "plugin:dialog|message" || cmd === "plugin:dialog|ask" || cmd === "plugin:dialog|confirm") {
        (window.__boites ??= []).push(args?.message ?? "");
        return Promise.resolve(cmd === "plugin:dialog|message" ? "Ok" : true);
      }
      return vrai(cmd, args, options);
    };
    t.__banc = true;
  }).catch(() => {});
  page.on("load", poser);
  await poser();
}
export const boites = (page) => page.evaluate(() => window.__boites ?? []);

/** L'adresse de la fenetre : celle qu'elle a chargee (devUrl). */
export const URL_FENETRE = "http://localhost:1420/";

/** Les erreurs qui comptent : ni le 401 du jeton perime au demarrage,
 *  ni les scripts bloques (voulu) des apercus sandboxes, ni les appels
 *  Tauri absents d'un navigateur, ni les commandes locales. */
export function erreursUtiles(erreurs) {
  return erreurs.filter(e => !e.includes("reading 'invoke'") && !e.includes("401") && !e.includes("sandboxed")
    // Les commandes locales de la fenetre (poste.json…) le disent
    // franchement dans un navigateur : ce n'est pas une panne d'ecran.
    && !e.includes("n'est disponible que dans l'application installée"));
}

export async function capture(page, nom) {
  const f = path.join(CAPTURES, `${nom}.png`);
  await page.screenshot({ path: f, fullPage: false });
  return f;
}

/** Verification : affiche et compte. `process.exitCode = 1` au premier echec. */
export function verifieur() {
  let n = 0;
  const ok = (c, m) => { n++; if (!c) { console.log("ECHEC :", m); process.exitCode = 1; } else console.log("ok  ", m); };
  ok.total = () => n;
  return ok;
}

/** Connexion ; change le mot de passe d'usine si l'ecran l'exige. */
export async function connecter(page, id = "admin", mdp = "admin123", nouveau = "Admin-2026!", dossier = "Ma boutique", adresse = URL) {
  await page.goto(adresse);
  await page.waitForTimeout(1500);
  const essayer = async (m) => {
    await page.locator("input").nth(0).fill(id);
    await page.locator('input[type="password"]').first().fill(m);
    await page.keyboard.press("Enter");
    // bcrypt prend ~2 s par verification sur un build de debogage : on
    // attend que la page de connexion s'en aille (ou refuse), jusqu'a
    // vingt secondes, plutot qu'un delai fixe qui laissait le jeton en
    // route et le parcours sans session.
    for (let i = 0; i < 40; i++) {
      await page.waitForTimeout(500);
      if (await page.getByText("Identifiant ou mot de passe incorrect").count()) return false;
      if (!(await page.getByText("Pseudo ou email").count())) { await page.waitForTimeout(1500); return true; }
    }
    return false;
  };
  // v3 (D-3) : plusieurs dossiers, aucun memorise — l'ecran demande
  // lequel ouvrir. Par defaut, le dossier d'origine.
  const choisirDossier = async () => {
    // La boite arrive apres la reponse du serveur : lui laisser le temps.
    await page.getByText("Quel dossier ouvrir ?").waitFor({ timeout: 4000 }).catch(() => {});
    if (await page.getByText("Quel dossier ouvrir ?").count()) {
      // Ne pas memoriser : sinon le compte rouvrirait ce dossier d'office
      // a chaque connexion des parcours suivants.
      await page.getByLabel(/directement la prochaine fois/).uncheck();
      await page.getByRole("button", { name: new RegExp(dossier) }).first().click();
      await page.waitForTimeout(2000);
    }
  };
  if (!(await essayer(mdp))) { await essayer(nouveau); await choisirDossier(); return; }
  await choisirDossier();
  // La fenetre « changer le mot de passe » peut arriver apres l'accueil.
  const pw = page.locator('input[type="password"]');
  for (let i = 0; i < 10 && await pw.count() < 2; i++) await page.waitForTimeout(500);
  if (await pw.count() >= 2) {
    const n = await pw.count();
    if (n === 3) { await pw.nth(0).fill(mdp); await pw.nth(1).fill(nouveau); await pw.nth(2).fill(nouveau); }
    else { await pw.nth(0).fill(nouveau); await pw.nth(1).fill(nouveau); }
    await page.getByRole("button", { name: "Changer" }).click();
    // Le premier changement de mot de passe prend parfois plus de deux
    // secondes (premiere commande du serveur) : attendre que la fenetre
    // disparaisse, jusqu'a dix secondes, plutot qu'un delai fixe —
    // recharger pendant la commande laissait la reponse en route.
    for (let i = 0; i < 20 && await pw.count() >= 2; i++) await page.waitForTimeout(500);
    await page.waitForTimeout(1000);
    // Dans un navigateur, la boite « c'est fait » de Tauri n'existe pas :
    // le mot de passe est change, la fenetre reste. On recharge et on
    // se reconnecte avec le nouveau.
    if (await pw.count() >= 2) {
      // L'identite gardee par la page dit encore « doit changer » :
      // on l'oublie, et on se reconnecte.
      await page.evaluate(() => localStorage.clear());
      await page.goto(adresse); await page.waitForTimeout(1500);
      await essayer(nouveau);
      await choisirDossier();
    }
  }
}

/** Aller a un onglet de Parametres. */
export async function parametres(page, onglet) {
  await page.getByText("Paramètres", { exact: true }).first().click();
  await page.waitForTimeout(800);
  await page.getByRole("button", { name: onglet }).first().click();
  await page.waitForTimeout(1500);
}

/** Un PNG plein, pour televerser sans fichier livre avec le depot. */
export function png(nom, largeur, hauteur, [r, g, bl]) {
  const f = path.join(TRAVAIL, nom);
  const ligne = Buffer.concat([Buffer.from([0]), Buffer.from(Array(largeur).fill([r, g, bl]).flat())]);
  const brut = Buffer.concat(Array(hauteur).fill(ligne));
  const bloc = (type, data) => {
    const t = Buffer.from(type);
    const lg = Buffer.alloc(4); lg.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(zlib.crc32(Buffer.concat([t, data])) >>> 0);
    return Buffer.concat([lg, t, data, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(largeur, 0); ihdr.writeUInt32BE(hauteur, 4); ihdr[8] = 8; ihdr[9] = 2;
  fs.writeFileSync(f, Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    bloc("IHDR", ihdr), bloc("IDAT", zlib.deflateSync(brut)), bloc("IEND", Buffer.alloc(0)),
  ]));
  return f;
}
