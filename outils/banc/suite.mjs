// Joue les parcours du banc dans l'ordre et dit, a la fin, ce qui passe
// et ce qui casse — sans s'arreter au premier echec (`npm run tout`
// s'arrete, et sa boucle `for` ne tourne pas sous cmd.exe).
//
//   node suite.mjs                 les parcours a1… i3 (serveur : sqlite ou multiposte)
//   node suite.mjs m               les parcours multiposte m*.mjs
//   node suite.mjs p               les parcours monoposte p*.mjs (lancer.ps1 monoposte)
//   node suite.mjs a1 c2 h1        ceux-la seulement
//
// Chaque sortie est gardee dans .travail/suite/<parcours>.log.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ICI = path.dirname(fileURLToPath(import.meta.url));
const JOURNAUX = path.join(ICI, ".travail", "suite");
fs.mkdirSync(JOURNAUX, { recursive: true });

const tous = fs.readdirSync(ICI).filter(f => /^[a-z][0-9][a-z0-9-]*\.mjs$/.test(f)).sort();
const demandes = process.argv.slice(2);
let liste;
if (demandes.length === 0) liste = tous.filter(f => !/^[mp][0-9]/.test(f));
else if (demandes.length === 1 && /^[mp]$/.test(demandes[0])) liste = tous.filter(f => f.startsWith(demandes[0]));
else liste = demandes.map(d => tous.find(f => f.startsWith(d + "-") || f === d + ".mjs" || f === d)).filter(Boolean);

// i2 lit les pages du PDF avec pdftoppm (poppler) : sans lui, il ne
// prouve rien — on le dit plutot que de le compter en echec.
const aPdftoppm = spawnSync("pdftoppm", ["-v"]).error === undefined;

const bilan = [];
for (const f of liste) {
  if (f.startsWith("i2-") && !aPdftoppm) { bilan.push([f, "SAUTE", "pdftoppm absent"]); console.log(`-- ${f} : saute (pdftoppm absent)`); continue; }
  const debut = Date.now();
  const r = spawnSync(process.execPath, [path.join(ICI, f)], { cwd: ICI, encoding: "utf8", timeout: 15 * 60 * 1000, env: process.env });
  const sortie = (r.stdout ?? "") + (r.stderr ?? "");
  fs.writeFileSync(path.join(JOURNAUX, f.replace(/\.mjs$/, ".log")), sortie);
  const oks = (sortie.match(/^ok  /gm) ?? []).length;
  const echecs = sortie.split("\n").filter(l => l.startsWith("ECHEC"));
  const secondes = Math.round((Date.now() - debut) / 1000);
  const passe = r.status === 0 && echecs.length === 0;
  bilan.push([f, passe ? "ok" : "ECHEC", `${oks} ok, ${echecs.length} echec(s), ${secondes} s`]);
  console.log(`-- ${f} : ${passe ? "ok" : "ECHEC"} (${oks} ok, ${echecs.length} echec(s), ${secondes} s)`);
  for (const e of echecs) console.log("   " + e);
  if (!passe && echecs.length === 0) console.log("   " + sortie.trim().split("\n").slice(-6).join("\n   "));
}
const n = bilan.filter(b => b[1] === "ECHEC").length;
console.log(`\n${bilan.length} parcours, ${bilan.filter(b => b[1] === "ok").length} verts, ${n} en echec, ${bilan.filter(b => b[1] === "SAUTE").length} sautes.`);
process.exitCode = n ? 1 : 0;
