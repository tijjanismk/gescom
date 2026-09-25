// Revue du 25/09 : la copie automatique du serveur attendait 24 h de
// marche continue — jamais atteintes sur un poste éteint le soir — et
// l'écran lisait la « dernière sauvegarde » dans le journal, où cette
// copie n'écrit rien. Ici : Paramètres → Sauvegarde dit ce que le
// dossier contient VRAIMENT, en rouge quand il n'y a rien, en vert après
// une copie, avec la liste.
import { navigateur, connecter, capture, verifieur, erreursUtiles, parametres, TRAVAIL } from "./pw.mjs";
import fs from "node:fs";
import path from "node:path";

const { b, page, erreurs } = await navigateur();
const ok = verifieur();

// Le dossier que lancer.sh donne au serveur (--sauvegardes), vidé.
const dossier = path.join(TRAVAIL, "sauvegardes");
fs.mkdirSync(dossier, { recursive: true });
for (const f of fs.readdirSync(dossier)) fs.rmSync(path.join(dossier, f), { force: true, recursive: true });

await connecter(page);
await parametres(page, "Sauvegarde");
const carte = page.getByTestId("etat-copies");
ok((await carte.innerText()).includes("Aucune copie dans ce dossier"), "dossier vide : l'écran le dit, en alerte");
ok((await carte.innerText()).includes("sauvegardes"), "le dossier où le serveur copie est nommé");

const r = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const c = await appeler("lire_config_sauvegarde");
  const chemin = await appeler("sauvegarder_base", { dossierDestination: c.dossier_effectif });
  return { chemin, apres: await appeler("lire_config_sauvegarde") };
});
ok(/gescom_backup_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}\.db$/.test(r.chemin), `la copie porte sa date à la seconde (${path.basename(r.chemin)})`);
ok(r.apres.copies.length === 1 && r.apres.derniere_copie?.nom === path.basename(r.chemin), "relue du dossier : une copie, la bonne");

await page.reload();
await page.waitForTimeout(1500);
await parametres(page, "Sauvegarde");
const texte = await page.getByTestId("etat-copies").innerText();
ok(/Dernière copie le .*il y a moins d'une heure/.test(texte), "après la copie : la date, en clair");
ok(texte.includes("1 copie dans le dossier"), "la liste des copies");
await capture(page, "i3-sauvegarde");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
