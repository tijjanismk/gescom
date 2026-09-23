// Aides Playwright du banc Gescom : ouvrir, se connecter, capturer.
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const ICI = path.dirname(fileURLToPath(import.meta.url));
export const TRAVAIL = path.join(ICI, ".travail");
export const CAPTURES = path.join(TRAVAIL, "captures");
fs.mkdirSync(CAPTURES, { recursive: true });
export const URL = `http://127.0.0.1:${process.env.PORT_ECRAN ?? 1420}/?serveur=127.0.0.1:${process.env.PORT_SERVEUR ?? 7300}`;

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
export async function connecter(page, id = "admin", mdp = "admin123", nouveau = "Admin-2026!") {
  await page.goto(URL);
  await page.waitForTimeout(1500);
  const essayer = async (m) => {
    await page.locator("input").nth(0).fill(id);
    await page.locator('input[type="password"]').first().fill(m);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(2500);
    return !(await page.getByText("Identifiant ou mot de passe incorrect").count());
  };
  if (!(await essayer(mdp))) { await essayer(nouveau); return; }
  // La fenetre « changer le mot de passe » peut arriver apres l'accueil.
  const pw = page.locator('input[type="password"]');
  for (let i = 0; i < 10 && await pw.count() < 2; i++) await page.waitForTimeout(500);
  if (await pw.count() >= 2) {
    const n = await pw.count();
    if (n === 3) { await pw.nth(0).fill(mdp); await pw.nth(1).fill(nouveau); await pw.nth(2).fill(nouveau); }
    else { await pw.nth(0).fill(nouveau); await pw.nth(1).fill(nouveau); }
    await page.getByRole("button", { name: "Changer" }).click();
    await page.waitForTimeout(2000);
    // Dans un navigateur, la boite « c'est fait » de Tauri n'existe pas :
    // le mot de passe est change, la fenetre reste. On recharge et on
    // se reconnecte avec le nouveau.
    if (await pw.count() >= 2) {
      // L'identite gardee par la page dit encore « doit changer » :
      // on l'oublie, et on se reconnecte.
      await page.evaluate(() => localStorage.clear());
      await page.goto(URL); await page.waitForTimeout(1500);
      await essayer(nouveau);
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
