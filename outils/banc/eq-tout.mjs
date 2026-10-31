// EQ-1 a EQ-6 fusionnes (D47 a D52) : un parcours dans Gescom Equipe qui
// passe par chaque ajout, contre le serveur fusionne. Ce que les
// scenarios Rust de chaque lot ne voient pas : les ajouts ensemble.
//   - Paie -> Prets : preter par l'ecran, l'echeancier annonce (EQ-1) ;
//   - l'accueil : le pret a sa tuile, il ne gonfle pas les avances, et la
//     tuile ouvre l'onglet Prets (EQ-3 + EQ-1) ;
//   - Paie -> Etat du mois : l'echeance du pret dans sa colonne (EQ-2) ;
//   - Personnel -> Jours travailles : une absence pour maladie, sa lettre,
//     et le compte de l'annee sur la fiche (EQ-6) ;
//   - Suivi clients -> A relancer : un client parti, et « est revenu » (EQ-4) ;
//   - la fiche 360 : le message WhatsApp a ecrire (EQ-5).
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const awa = `Awa ${s}`;
const moussa = `Moussa ${s}`;
const fanta = `Fanta ${s}`;

const iso = d => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const auj = new Date();
const suivant = new Date(auj.getFullYear(), auj.getMonth() + 1, 1);
const moisSuivant = iso(suivant).slice(0, 7);
const finSuivant = iso(new Date(suivant.getFullYear(), suivant.getMonth() + 1, 0));
const jourAbsent = iso(auj);
const net = t => t.replace(/\s+/g, " ").trim();
// Le chiffre d'une tuile de l'accueil : « Prêts en cours 1 1 personne… ».
const chiffre = async id => Number((net(await page.getByTestId(id).innerText()).match(/en cours (\d+)/) ?? [])[1]);
// Ce qu'elle dit rester a retenir, en francs (0 si rien).
const aRetenir = async id => Number(((net(await page.getByTestId(id).innerText()).match(/([\d ]+) F à retenir/) ?? [])[1] ?? "0").replace(/ /g, ""));

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
const ids = await page.evaluate(async ({ awa, moussa, fanta }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const a = await appeler("creer_employe", { fiche: { nom: awa, fonction: "vendeuse", salaire_mensuel: 60000, telephone: "76 11 22 33" } });
  const m = await appeler("creer_employe", { fiche: { nom: moussa, fonction: "manœuvre", tarif_journalier: 2500 } });
  await appeler("donner_avance", { employeId: m.id, montant: 5000 });
  const c = await appeler("creer_client_rapide", { nom: fanta, telephone: "66 44 55 66" });
  return { awa: a.id, moussa: m.id, fanta: c.id };
}, { awa, moussa, fanta });
erreurs.splice(0);
// Un refus (403) se dit avec la commande refusee, pour savoir laquelle.
// Sauf les polices que Vite sert par /@fs/ : dans un worktree dont
// node_modules pointe vers le depot principal, Vite les refuse (hors de
// ses dossiers permis) — l'environnement, pas l'ecran.
const police = u => u.includes("/@fs/") && u.endsWith(".woff2");
let polices403 = 0;
page.on("response", async r => {
  if (r.status() !== 403) return;
  if (police(r.url())) { polices403++; return; }
  erreurs.push(`403 sur ${r.request().postData()?.slice(0, 120) ?? r.url()} : ${(await r.text().catch(() => "")).slice(0, 160)}`);
});
const menu = nom => page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: new RegExp(nom) });
const tuile = id => page.getByTestId(id);

// --- L'accueil avant le pret (relu : l'avance vient d'etre donnee) ------
await page.reload();
await page.waitForTimeout(2500);
const avancesAvant = net(await tuile("jour-avances").innerText());
ok(await tuile("jour-prets").count() === 1, "le patron voit la tuile « Prêts en cours »");
const pretsAvant = await chiffre("jour-prets");
const resteAvant = await aRetenir("jour-prets");

// --- EQ-1 : preter par l'ecran -------------------------------------------
await menu("Paie").click();
await page.getByRole("tab", { name: "Prêts" }).click();
await page.waitForTimeout(1200);
await page.getByLabel("À qui").selectOption(ids.awa);
await page.getByLabel("Montant du prêt").fill("60000");
await page.getByLabel("Retenue par mois").fill("10000");
await page.waitForTimeout(300);
ok(net(await page.getByTestId("echeancier").innerText()).startsWith("6 échéances"), "l'échéancier annonce 6 échéances");
await page.getByTestId("nouveau-pret").getByRole("button", { name: /Prêter/ }).click();
await page.waitForTimeout(1500);
ok(await page.getByTestId("pret").filter({ hasText: awa }).count() === 1, "le prêt d'Awa est dans la liste");
await capture(page, "eq-tout-01-pret");

// --- EQ-3 : la tuile du pret, a part des avances -------------------------
await menu("Accueil").click();
await page.waitForTimeout(1500);
ok(net(await tuile("jour-avances").innerText()) === avancesAvant, "le prêt ne gonfle pas la tuile des avances");
const pretsApres = await chiffre("jour-prets");
ok(pretsApres === pretsAvant + 1, `la tuile des prêts compte un prêt de plus (${pretsAvant} → ${pretsApres})`);
ok(await aRetenir("jour-prets") - resteAvant === 60000, "et 60 000 F de plus à retenir");
await capture(page, "eq-tout-02-accueil");
await tuile("jour-prets").click();
await page.waitForTimeout(1200);
ok(await page.getByRole("tab", { name: "Prêts" }).getAttribute("aria-selected") === "true", "la tuile ouvre Paie → Prêts");

// --- EQ-2 : l'echeance dans sa colonne ----------------------------------
await page.evaluate(async ({ id, du, au }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const f = await appeler("preparer_fiche_paie", { employeId: id, du, au, prorata: false });
  await appeler("valider_fiche_paie", { ficheId: f.id });
}, { id: ids.awa, du: `${moisSuivant}-01`, au: finSuivant });
await page.getByRole("tab", { name: "État du mois" }).click();
await page.waitForTimeout(1000);
await page.getByLabel("Mois de l'état").fill(moisSuivant);
await page.waitForTimeout(1500);
const entetes = await page.getByTestId("tableau-etat").locator("thead th").allInnerTexts();
ok(entetes.includes("Prêts"), `l'état a une colonne « Prêts » (${entetes.join(" | ")})`);
const ligne = page.getByTestId("ligne-etat").filter({ hasText: awa });
const cases = await ligne.locator("td").allInnerTexts();
const col = c => net(cases[entetes.indexOf(c)] ?? "");
ok(col("Prêts") === "10 000 F", `l'échéance d'Awa est sous « Prêts » (${col("Prêts")})`);
ok(col("Avances") === "—", "pas sous « Avances »");
ok(col("Net") === "50 000 F", "le net : 60 000 − 10 000");
await capture(page, "eq-tout-03-etat");

// --- EQ-6 : une absence pour maladie ------------------------------------
await page.evaluate(async ({ id, jour }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("marquer_presence", { employeId: id, jour, etat: "absent", motif: "maladie" });
}, { id: ids.moussa, jour: jourAbsent });
await menu("Personnel").click();
await page.getByRole("tab", { name: "Jours travaillés" }).click().catch(async () => {
  await page.getByRole("button", { name: "Jours travaillés" }).click();
});
await page.waitForTimeout(1500);
const caseAbsente = page.getByRole("button", { name: `${moussa} le ${jourAbsent} : absent, maladie` });
ok(await caseAbsente.count() === 1, "la case du jour dit « absent, maladie »");
ok((await caseAbsente.innerText()).trim() === "M", "et porte la lettre M");
await capture(page, "eq-tout-04-absence");
await page.getByRole("tab", { name: "Fiches" }).click().catch(async () => {
  await page.getByRole("button", { name: "Fiches", exact: true }).click();
});
await page.waitForTimeout(1500);
const absences = page.getByTestId("employe").filter({ hasText: moussa }).getByTestId("absences-annee");
ok(await absences.count() === 1 && /maladie/i.test(await absences.innerText()), "la fiche compte la maladie de l'année");

// --- EQ-4 : un client parti, puis revenu --------------------------------
await page.evaluate(async ({ id }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("marquer_client_parti", { clientId: id, raison: "prix" });
}, { id: ids.fanta });
await menu("Suivi clients").click();
await page.getByRole("tab", { name: "À relancer" }).click().catch(async () => {
  await page.getByRole("button", { name: "À relancer" }).click();
});
await page.waitForTimeout(1500);
ok(await page.getByTestId("clients-a-relancer").count() === 1, "l'onglet « À relancer » s'ouvre");
// La ligne des partis se deplie : « Partis : N (prix …) — voir la liste ».
const lignePartis = page.getByTestId("clients-partis").getByRole("button", { name: /Partis :/ });
ok(/prix/i.test(await lignePartis.innerText()), "la ligne des partis compte la raison « prix »");
await lignePartis.click();
await page.waitForTimeout(500);
const revenu = page.getByRole("button", { name: `${fanta} est revenu` });
ok(await revenu.count() === 1, "Fanta est parmi les partis, avec « est revenu »");
await capture(page, "eq-tout-05-partis");
await revenu.click();
await page.waitForTimeout(1500);
ok(await page.getByRole("button", { name: `${fanta} est revenu` }).count() === 0, "revenue : elle sort des partis");

// --- EQ-5 : le message WhatsApp sur la fiche 360 ------------------------
await page.getByRole("tab", { name: "Clients" }).click().catch(async () => {
  await page.getByRole("button", { name: "Clients", exact: true }).click();
});
await page.waitForTimeout(1200);
await page.getByText(fanta).first().click();
await page.waitForTimeout(1500);
ok(await page.getByLabel("Message WhatsApp").count() + await page.getByRole("button", { name: /WhatsApp/ }).count() > 0,
  "la fiche 360 offre d'écrire sur WhatsApp");
await capture(page, "eq-tout-06-whatsapp");

// Chaque police refusee laisse aussi un « Failed to load resource » : on
// en retire autant, pas plus.
const vraies = erreursUtiles(erreurs).filter(e => !(e.includes("status of 403") && polices403-- > 0));
ok(vraies.length === 0, `aucune erreur dans la page${vraies.length ? " : " + vraies.join(" | ") : ""}`);
console.log(`\n${ok.total()} vérifications`);
await b.close();
