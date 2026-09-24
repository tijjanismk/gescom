// G-2 : Equipe -> Paie -> Fiches du mois. Moussa a la journee : ses
// jours se comptent, une tache s'ajoute, une retenue sans motif se
// refuse, la fiche validee prend un numero et ne se corrige plus. Awa
// au mois : son avance se retient ; une rectificative la remplace.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const moussa = `Moussa ${s}`;
const awa = `Awa ${s}`;
const d = new Date();
const mois = d.toISOString().slice(0, 7);
const jours = Math.min(10, d.getDate());

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
await page.evaluate(async ({ moussa, awa, mois, jours }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* déjà ouverte */ }
  const m = await appeler("creer_employe", { fiche: { nom: moussa, fonction: "manœuvre", tarif_journalier: 2500 } });
  const a = await appeler("creer_employe", { fiche: { nom: awa, fonction: "vendeuse", salaire_mensuel: 60000 } });
  for (let j = 1; j <= jours; j++) await appeler("marquer_presence", { employeId: m.id, jour: `${mois}-${String(j).padStart(2, "0")}`, etat: "present" });
  await appeler("donner_avance", { employeId: a.id, montant: 10000, moyen: "especes", motif: null });
}, { moussa, awa, mois, jours });
// La caisse déjà ouverte par un parcours précédent répond 409 : attendu.
erreurs.splice(0);

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Paie/ }).click();
await page.getByRole("tab", { name: "Fiches du mois" }).click();
await page.waitForTimeout(1500);
const net = t => t.replace(/\s/g, " ");
const statut = async () => net(await page.getByRole("status").innerText());
const aPayer = async () => net(await page.getByTestId("net-a-payer").innerText());
const fr = n => n.toLocaleString("fr-FR").replace(/\s/g, " ");

// Moussa : préparer.
await page.getByRole("button", { name: `Préparer la fiche de ${moussa}` }).click();
await page.waitForTimeout(1500);
ok((await statut()).includes(`Brouillon de ${moussa} préparé : ${fr(2500 * jours)} F net`), `brouillon préparé : ${jours} jours × 2 500 F`);
ok(net(await page.getByTestId("lignes-paie").innerText()).includes(`${jours} jour${jours > 1 ? "s" : ""} × 2 500 F`), "la ligne dit d'où vient le montant");

// Une tâche : 3 livraisons × 1 000.
await page.getByLabel("Genre de ligne").selectOption("tache");
await page.getByLabel("Libellé de la ligne").fill("Livraisons");
await page.getByLabel("Quantité").fill("3");
await page.getByLabel("Prix unitaire").fill("1000");
await page.getByRole("button", { name: /Ajouter la ligne/ }).click();
await page.waitForTimeout(1200);
ok((await aPayer()) === `${fr(2500 * jours + 3000)} F`, "3 livraisons ajoutées au net");

// Une retenue sans motif : refusée.
const n0 = erreurs.length;
await page.getByLabel("Genre de ligne").selectOption("retenue");
await page.getByLabel("Montant de la ligne").fill("500");
await page.getByRole("button", { name: /Ajouter la ligne/ }).click();
await page.waitForTimeout(1200);
ok((await statut()).includes("dit pourquoi"), "une retenue sans motif est refusée");
erreurs.splice(n0);
await capture(page, "g2-01-brouillon");

// Valider : un numéro, figée.
await page.getByRole("button", { name: /Valider la fiche/ }).click();
await page.waitForTimeout(1500);
const m1 = (await statut()).match(/Fiche (PAIE-\d{4}-\d{5}) validée/);
ok(!!m1, `validée avec un numéro (${m1?.[1]})`);
ok(await page.getByRole("button", { name: /Ajouter la ligne/ }).count() === 0 && await page.getByRole("button", { name: /Faire une rectificative/ }).count() === 1,
  "validée : plus rien ne s'ajoute, une rectificative est proposée");

// Awa : l'avance se retient.
await page.getByRole("button", { name: `Préparer la fiche de ${awa}` }).click();
await page.waitForTimeout(1500);
ok((await aPayer()) === "50 000 F" && net(await page.getByTestId("lignes-paie").innerText()).includes("Avance du"), "Awa : 60 000 F moins son avance de 10 000 F");
await page.getByRole("button", { name: /Valider la fiche/ }).click();
await page.waitForTimeout(1500);
const numeroAwa = (await statut()).match(/Fiche (PAIE-\d{4}-\d{5}) validée/)?.[1];

// On a oublié sa prime : une rectificative.
await page.getByRole("button", { name: /Faire une rectificative/ }).click();
await page.waitForTimeout(1500);
ok(net(await page.getByTestId("fiche-paie").innerText()).includes(`rectifie ${numeroAwa}`), "la rectificative dit ce qu'elle remplace");
ok((await aPayer()) === "50 000 F", "l'avance déjà retenue par l'ancienne reste retenue, pas deux fois");
await page.getByLabel("Genre de ligne").selectOption("prime");
await page.getByLabel("Libellé de la ligne").fill("Fin de mois");
await page.getByLabel("Montant de la ligne").fill("5000");
await page.getByRole("button", { name: /Ajouter la ligne/ }).click();
await page.waitForTimeout(1200);
await page.getByRole("button", { name: /Valider la fiche/ }).click();
await page.waitForTimeout(1500);
ok((await statut()).includes("55 000 F à payer"), "la rectificative validée : 55 000 F");
const lignesAwa = page.getByTestId("fiche").filter({ hasText: awa });
ok(await lignesAwa.count() === 2, "deux fiches pour Awa : l'ancienne reste, remplacée");
await lignesAwa.filter({ hasNotText: "rectifie" }).click();
await page.waitForTimeout(1200);
ok(net(await page.getByTestId("fiche-paie").innerText()).includes("remplacée par PAIE-"), "l'ancienne dit par quoi elle est remplacée");
await capture(page, "g2-02-rectificative");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
