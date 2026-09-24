// G-1 : Equipe -> Paie -> Avances. Hors caisse (decision du 24/09) :
// caisse fermee, l'avance se donne quand meme, au nom de la personne,
// sous son plafond ; le tiroir du jour n'en sait rien ; une avance
// donnee par erreur s'annule.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const awa = `Awa ${s}`;

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
const id = await page.evaluate(async nom => {
  const { appeler } = await import("/src/lib/pont.ts");
  // Caisse fermée : la paie n'en a pas besoin.
  try {
    const ss = await appeler("lire_sessions_caisse", {});
    for (const x of ss.filter(x => x.statut === "ouverte")) await appeler("fermer_session_caisse", { sessionId: x.id, especesComptees: 0 });
  } catch { /* rien d'ouvert */ }
  const e = await appeler("creer_employe", { fiche: { nom, fonction: "vendeuse", salaire_mensuel: 60000, avance_max: 15000 } });
  return e.id;
}, awa);

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Paie/ }).click();
await page.waitForTimeout(1200);
// toLocaleString("fr-FR") groupe par une espace fine insécable : on la
// ramène à une espace simple pour comparer.
const net = t => t.replace(/\s/g, " ");
const statut = async () => net(await page.getByRole("status").innerText());
const donner = async (m, moyen = "especes") => {
  await page.getByLabel("À qui").selectOption(id);
  await page.getByLabel("Montant de l'avance").fill(String(m));
  await page.getByLabel("Donnée en").selectOption(moyen);
  await page.getByRole("button", { name: /Donner l'avance/ }).click();
  await page.waitForTimeout(1200);
};

await donner(10000);
ok((await statut()).includes(`Avance de 10 000 F à ${awa} notée`), "caisse fermée : l'avance se donne quand même");
const n1 = erreurs.length;
await donner(6000);
ok((await statut()).includes("dépasserait son plafond de 15 000 F"), "au-delà du plafond : refusé, avec les chiffres");
erreurs.splice(n1);
await donner(5000, "orange_money");
ok(net(await page.getByTestId("en-cours-par-personne").innerText()).includes(`${awa} : 15 000 F en cours`), "15 000 F en cours pour Awa");
ok(await page.getByTestId("avance").filter({ hasText: awa }).count() === 2, "deux avances en cours");
await capture(page, "g1-01-avances");

// Annuler la seconde.
await page.getByRole("button", { name: `Annuler l'avance de ${awa}` }).first().click();
await page.waitForTimeout(1200);
ok((await statut()).includes("annulée : elle ne sera pas retenue"), "annulée");
ok((await page.getByTestId("en-cours-par-personne").innerText()).includes("en cours") && await page.getByTestId("avance").filter({ hasText: awa }).count() === 1, "une seule avance en cours");
await page.getByLabel("Voir aussi les retenues et annulées").check();
await page.waitForTimeout(1000);
ok((await page.getByTestId("liste-avances").innerText()).includes("annulée"), "l'annulée reste visible, marquée");

// Le tiroir du jour n'en sait rien.
const mouvements = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("ouvrir_session_caisse", { fondOuverture: 0 });
  const ss = await appeler("lire_sessions_caisse", {});
  return ss.find(x => x.statut === "ouverte")?.sorties_especes ?? null;
});
ok(mouvements === 0, `la caisse du jour n'est pas touchée (${mouvements})`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
