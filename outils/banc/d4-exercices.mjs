// D-4 : Parametres -> Dossiers -> Dates de travail. Dans un dossier
// dont l'exercice est passe, une ecriture est refusee avec le remede ;
// on prolonge, on clot, on ouvre le suivant — par l'ecran — et
// l'ecriture n'est plus refusee pour ses dates. L'Historique le dit.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const code = `E${Date.now() % 100000}`;
const societe = `Exercices ${code}`;

// Un dossier dont les dates de travail sont passees.
await parametres(page, "Dossiers");
await page.getByLabel("Code du dossier").fill(code);
await page.getByLabel("Société").fill(societe);
await page.getByLabel("Début des dates de travail").fill("2025-01-01");
await page.getByLabel("Fin des dates de travail").fill("2025-12-31");
await page.getByRole("button", { name: /Créer le dossier/ }).click();
await page.waitForTimeout(1500);
ok((await page.getByRole("status").innerText()).includes(`Dossier ${code} créé`), "dossier aux dates passées créé");

const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
const p2 = await ctx.newPage();
p2.on("console", m => { if (m.type() === "error") erreurs.push(m.text()); });
await connecter(p2, "admin", "Admin-2026!", "Admin-2026!", societe);

// Une depense (datee d'aujourd'hui) : refusee, le refus dit quoi faire.
const depense = () => p2.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("enregistrer_depense", { montant: 500, libelle: "banc d4" }); return "passée"; }
  catch (e) { return String(e); }
});
const n0 = erreurs.length;
let refus = await depense();
ok(refus.includes("hors des dates de travail (jusqu'au 31 décembre 2025)") && refus.includes("Prolonger l'exercice"),
  `refus avec le remède : « ${refus.slice(0, 110)}… »`);

await parametres(p2, "Dossiers");
const exercices = p2.getByTestId("exercice");
ok(await exercices.count() === 1, "un exercice");
ok((await exercices.first().innerText()).includes("Du 1er janvier 2025 au 31 décembre 2025"), "ses dates en lettres");
ok(await p2.getByTestId("exercices").getByText("aujourd'hui").count() === 0, "il ne couvre pas aujourd'hui");
await capture(p2, "d4-01-exercice-passe");

// Prolonger : plus court refuse, puis d'un mois.
await exercices.first().getByRole("button", { name: /Prolonger/ }).click();
await p2.getByLabel("Prolonger jusqu'au").fill("2025-11-30");
await p2.getByRole("button", { name: "Valider" }).click();
await p2.waitForTimeout(1000);
ok((await p2.getByTestId("exercices").getByRole("status").innerText()).includes("va déjà jusqu'au 31 décembre 2025"),
  "une prolongation plus courte : refusée, dite");
await p2.getByLabel("Prolonger jusqu'au").fill("2026-01-31");
await p2.getByRole("button", { name: "Valider" }).click();
await p2.waitForTimeout(1000);
ok((await p2.getByTestId("exercices").getByRole("status").innerText()).includes("Prolongé jusqu'au 31 janvier 2026"), "prolongé");
const ligne = await exercices.first().innerText();
ok(ligne.includes("au 31 janvier 2026") && ligne.includes("fin prévue le 31 décembre 2025"), "la ligne dit la prolongation et la fin prévue");
ok((await p2.getByTestId("debut-suivant").innerText()) === "1er février 2026", "le suivant commence le lendemain, prolongation comprise");

// Clore : deux temps, puis on n'ecrit plus rien — pas meme aujourd'hui.
await exercices.first().getByRole("button", { name: "Clore" }).click();
ok(await p2.getByText("ça ne se défait pas").count() === 1, "la clôture demande confirmation");
await p2.getByRole("button", { name: "Confirmer la clôture" }).click();
await p2.waitForTimeout(1000);
ok((await p2.getByTestId("exercices").getByRole("status").innerText()).includes("Exercice clos"), "clos");
ok(await exercices.first().getByRole("button", { name: /Prolonger|Clore/ }).count() === 0, "un exercice clos n'a plus de boutons");
refus = await depense();
ok(refus.includes("aucun exercice ouvert"), `aujourd'hui : aucun exercice ouvert (« ${refus.slice(0, 80)}… »)`);

// Ouvrir le suivant, du 1er fevrier 2026 au 31 janvier 2027.
await p2.getByLabel("Fin de l'exercice suivant").fill("2027-01-31");
await p2.getByRole("button", { name: /Ouvrir l'exercice suivant/ }).click();
await p2.waitForTimeout(1200);
ok((await p2.getByTestId("exercices").getByRole("status").innerText()).includes("Exercice ouvert du 1er février 2026 au 31 janvier 2027"), "le suivant est ouvert");
ok(await exercices.count() === 2, "deux exercices");
ok(await exercices.nth(1).getByText("aujourd'hui").count() === 1, "le nouveau couvre aujourd'hui");
await capture(p2, "d4-02-suivant-ouvert");
refus = await depense();
ok(!/dates de travail|exercice/.test(refus), `la dépense n'est plus refusée pour ses dates (« ${refus.slice(0, 80)} »)`);
erreurs.splice(n0);

// L'Historique du dossier dit qui a prolonge, clos, ouvert.
await p2.getByText("Historique", { exact: true }).first().click();
await p2.waitForTimeout(1500);
const hist = await p2.getByRole("main").innerText();
ok(["Exercice prolongé", "Exercice clos", "Exercice ouvert"].every(t => hist.includes(t)), "l'Historique nomme les trois gestes");
await capture(p2, "d4-03-historique");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
