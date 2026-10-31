// Rend tij.svg en tij.png (1024 px, fond transparent) avec le Playwright
// du banc et Edge, deja installe : rien a installer. Depuis un worktree
// sans `outils/banc/node_modules`, BANC nomme le banc du depot principal.
//
//   node outils/icones/rendre.mjs
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { readFileSync } from "node:fs";

const ICI = path.dirname(fileURLToPath(import.meta.url));
const BANC = process.env.BANC ?? path.join(ICI, "..", "banc");
const require = createRequire(path.join(BANC, "node_modules", "/"));
const { chromium } = require("playwright");

const navigateur = await chromium.launch({
  executablePath: process.env.CHROMIUM ?? "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
});
const page = await (await navigateur.newContext({ viewport: { width: 1200, height: 1200 } })).newPage();
// Le SVG dans une page sans marge : ouvert seul, il prend la barre de
// defilement et sort en 1008 px.
const svg = readFileSync(path.join(ICI, "tij.svg"), "utf8");
await page.setContent(`<html><body style="margin:0;background:transparent">${svg}</body></html>`);
await page.locator("svg").screenshot({ path: path.join(ICI, "tij.png"), omitBackground: true });
await navigateur.close();
console.log("tij.png");
