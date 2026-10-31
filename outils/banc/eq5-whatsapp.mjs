// EQ-5 (D51) : WhatsApp sans service payant. Le bouton n'envoie rien :
// il ouvre `https://wa.me/<numero>?text=…`. On remplace window.open pour
// relire le lien ouvert — numero, texte — sans lancer WhatsApp.
//   - Paie -> fiche validee -> Bulletin -> WhatsApp : le resume (periode,
//     net, verse, reste du) vers le telephone de l'employe ;
//   - un numero douteux : bouton desactive, la raison a cote ;
//   - Suivi clients -> fiche 360 : le message ecrit, WhatsApp ouvert,
//     l'echange `whatsapp` note au Oui seulement.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const awa = `Awa ${s}`;
const bakary = `Bakary ${s}`;
const fanta = `Fanta ${s}`;
const oumar = `Oumar ${s}`;
const d = new Date();
const mois = d.toISOString().slice(0, 7);
const fin = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();
const jj = iso => iso.split("-").reverse().join("/");

await page.addInitScript(() => {
  window.__liens = [];
  window.open = (u) => { window.__liens.push(String(u)); return null; };
});
const liens = () => page.evaluate(() => window.__liens);
const net = t => t.replace(/\s/g, " ");

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
await page.evaluate(async ({ awa, bakary, fanta, oumar, mois, fin }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const fiche = async (nom, telephone) => {
    const e = await appeler("creer_employe", { fiche: { nom, fonction: "vendeuse", salaire_mensuel: 60000, telephone } });
    const f = await appeler("preparer_fiche_paie", { employeId: e.id, du: `${mois}-01`, au: `${mois}-${fin}`, prorata: false });
    await appeler("valider_fiche_paie", { ficheId: f.id });
    return f.id;
  };
  const f = await fiche(awa, "00223 76 11 22 33");
  await appeler("verser_paie", { ficheId: f, montant: 20000, moyen: "orange_money", duTiroir: false });
  await fiche(bakary, "76 11 22");
  await appeler("creer_client_rapide", { nom: fanta, telephone: "+223 66 44 55 66" });
  await appeler("creer_client_rapide", { nom: oumar, telephone: "06 12 34 56 78" });
}, { awa, bakary, fanta, oumar, mois, fin });
erreurs.splice(0);

// --- Le bulletin -----------------------------------------------------
await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Paie/ }).click();
await page.getByRole("tab", { name: "Fiches du mois" }).click();
await page.waitForTimeout(1500);
await page.getByTestId("fiche").filter({ hasText: awa }).click();
await page.waitForTimeout(1200);
await page.getByRole("button", { name: /Bulletin/ }).click();
await page.waitForTimeout(2000);
const boutonBulletin = page.getByRole("button", { name: /Envoyer le résumé par WhatsApp/ });
ok(await boutonBulletin.isEnabled(), "fiche validée, numéro « 00223 76 11 22 33 » : le bouton WhatsApp est actif");
ok((await boutonBulletin.getAttribute("aria-label")).includes("+223 76 11 22 33"), "il dit vers quel numéro");
await boutonBulletin.click();
await page.waitForTimeout(500);
let l = await liens();
ok(l.length === 1, "un lien ouvert, un seul");
const u = new URL(l[0]);
ok(u.origin === "https://wa.me" && u.pathname === "/22376112233", `le lien porte le bon numéro (${u.origin}${u.pathname})`);
const texte = net(u.searchParams.get("text") ?? "");
ok(texte.startsWith(`Bonjour ${awa},`), "le texte salue l'employé");
ok(texte.includes(`Votre paie du ${jj(`${mois}-01`)} au ${jj(`${mois}-${fin}`)} (bulletin PAIE-`), "la période et le numéro du bulletin");
ok(texte.includes("Net à payer : 60 000 F") && texte.includes("Déjà versé : 20 000 F") && texte.includes("Reste dû : 40 000 F"), "net, versé, reste dû");
await capture(page, "eq5-01-bulletin");
await page.getByRole("button", { name: "Fermer le bulletin" }).click();

// Un numéro douteux : désactivé, la raison à côté.
await page.getByTestId("fiche").filter({ hasText: bakary }).click();
await page.waitForTimeout(1200);
await page.getByRole("button", { name: /Bulletin/ }).click();
await page.waitForTimeout(2000);
ok(await page.getByRole("button", { name: /Envoyer le résumé par WhatsApp/ }).isDisabled(), "« 76 11 22 » : le bouton est désactivé");
ok((await page.getByTestId("whatsapp-raison").innerText()).includes("6 chiffres : un numéro malien en a 8"), "et la raison s'affiche");
await capture(page, "eq5-02-numero-douteux");
await page.getByRole("button", { name: "Fermer le bulletin" }).click();
ok((await liens()).length === 1, "rien d'autre ne s'est ouvert");

// --- La fiche 360 ----------------------------------------------------
const ouvrirClient = async (nom) => {
  await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Suivi clients/ }).click();
  await page.waitForTimeout(800);
  // Une fiche 360 deja ouverte cache les onglets : on en revient.
  if (await page.getByTestId("fiche-360").count()) await page.getByRole("button", { name: "Retour" }).click();
  await page.getByRole("tab", { name: "Clients" }).click();
  await page.getByLabel("Rechercher un client").fill(nom);
  await page.waitForTimeout(1000);
  await page.getByTestId("resultat-client").filter({ hasText: nom }).first().click();
  await page.waitForTimeout(1200);
};

await ouvrirClient(oumar);
ok(await page.getByRole("button", { name: /Écrire sur WhatsApp/ }).isDisabled(), "client au « 06 12 34 56 78 » : bouton désactivé");
ok((await page.getByTestId("whatsapp-raison").innerText()).includes("10 chiffres sans indicatif"), "on ne devine pas le pays, et on le dit");

await ouvrirClient(fanta);
const echanges0 = await page.getByTestId("echange").count();
await page.getByRole("button", { name: /Écrire sur WhatsApp/ }).click();
const zone = page.getByLabel("Message WhatsApp");
ok(net(await zone.inputValue()) === `Bonjour ${fanta},\n\n`.replace(/\s/g, " "), "le message commence par le bonjour");
ok(await page.getByRole("button", { name: "Ouvrir WhatsApp" }).isDisabled(), "rien d'écrit après le bonjour : on n'ouvre pas");
await zone.fill(`Bonjour ${fanta},\n\nVotre commande de tissu est arrivée & vous attend.`);
await page.getByRole("button", { name: "Ouvrir WhatsApp" }).click();
await page.waitForTimeout(500);
l = await liens();
const v = new URL(l[l.length - 1]);
ok(v.pathname === "/22366445566", "le lien du client porte son numéro (+223 66 44 55 66)");
ok(v.searchParams.get("text") === `Bonjour ${fanta},\n\nVotre commande de tissu est arrivée & vous attend.`, "et le texte tel qu'écrit, « & » compris");
ok(await page.getByText("Le message est-il parti ?").count() === 1, "la fiche demande si le message est parti");
ok(await page.getByTestId("echange").count() === echanges0, "rien n'est noté au clic");
await capture(page, "eq5-03-parti");

await page.getByRole("button", { name: "Non" }).click();
ok(await page.getByTestId("echange").count() === echanges0, "« Non » : rien n'est noté");
ok((await zone.inputValue()).includes("tissu"), "et le texte reste, pour réessayer");
await page.getByRole("button", { name: "Ouvrir WhatsApp" }).click();
await page.waitForTimeout(500);
await page.getByRole("button", { name: "Oui, noter l'échange" }).click();
await page.waitForTimeout(1500);
ok((await page.getByRole("status").first().innerText()).includes("Échange WhatsApp noté."), "« Oui » : l'échange est noté");
const note = page.getByTestId("echange").filter({ hasText: "commande de tissu" });
ok(await note.count() === 1 && await page.getByTestId("echange").count() === echanges0 + 1, "un échange de plus dans le fil, avec le texte envoyé");
ok(await page.getByRole("button", { name: /Écrire sur WhatsApp/ }).isEnabled(), "la zone se referme");
await capture(page, "eq5-04-echange-note");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
