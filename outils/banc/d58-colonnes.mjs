// D58 (01-02/11/2026) : les colonnes des pièces se cochent et suivent la
// pièce. Depuis la fiche client — la même fenêtre « Nouvelle pièce » que
// Pièces : menu « Colonnes », d'office ni TVA ni remise, la TVA cochée
// pour ce devis. Le devis garde ses colonnes ; converti depuis la fiche,
// la commande garde les siennes ; modifié sans toucher au menu, rien ne
// bouge. Ce que les scénarios Rust ne voient pas : le menu de l'écran
// arrive bien au serveur.
//
//   serveur (sqlite ou multiposte) : node d58-colonnes.mjs
import { navigateur, connecter, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const s = Date.now() % 100000;
const nom = `Client D58 ${s}`;
const AVEC_TVA = ["designation", "quantite", "prix_unitaire", "tva", "total"];
const appel = (cmd, args) => page.evaluate(async ({ cmd, args }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return appeler(cmd, args);
}, { cmd, args });

const client = await appel("creer_client_rapide", { nom, telephone: "76 58 58 58" });
const pieces = async () => appel("lire_pieces_client", { clientId: client.id, typeFiltre: null });
const colonnes = async id => (await appel("lire_donnees_piece", { pieceId: id })).piece.colonnes;
const reglageDevis = (await appel("lire_reglages_documents", {})).genres.devis.colonnes;
erreurs.splice(0);
// Un 403 se dit avec son adresse. Sauf les polices que Vite sert par
// /@fs/ : dans un worktree dont node_modules est celui du dépôt
// principal, Vite les refuse (comme eq-tout.mjs) — l'environnement.
const police = u => u.includes("/@fs/") && u.endsWith(".woff2");
page.on("response", r => {
  if (r.status() === 403 && !police(r.url())) erreurs.push(`403 sur ${r.url()}`);
});
const sansConsole403 = e => !/status of 403/.test(e);

// La fiche 360 : Clients, la recherche ne laisse que lui, son bouton « Fiche ».
const ouvrirFiche = async () => {
  await page.getByRole("navigation").getByText("Clients", { exact: true }).first().click();
  await page.waitForTimeout(1500);
  await page.getByPlaceholder("Nom ou téléphone...").fill(nom);
  await page.waitForTimeout(1500);
  await page.locator("main").getByRole("button", { name: "Fiche", exact: true }).first().click();
  await page.waitForTimeout(2000);
};

// ---- La fiche client, « Nouvelle pièce ».
await ouvrirFiche();
await page.getByRole("button", { name: "Nouvelle pièce" }).first().click();
await page.waitForTimeout(1500);
const boite = page.getByRole("dialog");
ok(await boite.getByText("Nouvelle pièce client").count() === 1, "la fiche ouvre la même fenêtre que Pièces");
ok((await boite.innerText()).includes(nom), "le client est déjà choisi");

await boite.getByPlaceholder("Rechercher un article à ajouter...").fill("Sucre");
await page.waitForTimeout(800);
await boite.getByRole("button", { name: /Sucre/ }).first().click();
await page.waitForTimeout(500);
const entetes = async () => (await boite.locator("thead").innerText()).replace(/\s+/g, " ");
ok(!(await entetes()).includes("TVA %") && !(await entetes()).includes("Remise %"),
  `d'office, ni TVA ni remise (réglage du devis : ${JSON.stringify(reglageDevis)})`);

await boite.getByRole("button", { name: "Colonnes" }).click();
const menu = boite.getByRole("group", { name: "Colonnes de cette pièce" });
ok(await menu.count() === 1, "le menu « Colonnes » s'ouvre");
await menu.getByRole("checkbox", { name: "TVA %" }).check();
await boite.getByRole("button", { name: "Colonnes" }).click();
ok((await entetes()).includes("TVA %"), "TVA % cochée : la colonne apparaît");
await capture(page, "d58-01-fiche-client-colonnes");
await boite.getByRole("button", { name: /^Créer/ }).click();
await page.waitForTimeout(2500);

const devis = (await pieces()).find(p => p.type_piece === "devis");
ok(!!devis, `le devis est créé (${devis?.numero})`);
ok(JSON.stringify(await colonnes(devis.id)) === JSON.stringify(AVEC_TVA), "le devis garde ses colonnes");

// ---- La conversion, depuis la fiche : devis -> commande.
await page.reload();
await page.waitForTimeout(2000);
await ouvrirFiche();
const n = erreurs.length;
await page.locator("main").getByRole("button", { name: /^Pièces \(/ }).first().click();
await page.waitForTimeout(800);
await page.locator("main").getByRole("button", { name: "Commande", exact: true }).first().click();
await page.waitForTimeout(2500);
// Dans un navigateur, la boîte « Conversion réussie » de Tauri n'existe
// pas : son échec n'est pas une panne d'écran.
erreurs.splice(n);
const cmd = (await pieces()).find(p => p.type_piece === "commande_client");
ok(!!cmd, `la commande est créée depuis la fiche (${cmd?.numero})`);
ok(JSON.stringify(await colonnes(cmd.id)) === JSON.stringify(AVEC_TVA), "la commande garde les colonnes du devis");

// ---- Modifier sans toucher au menu : les colonnes ne bougent pas.
await appel("modifier_piece", { pieceId: cmd.id, note: "Livrer lundi", dateEcheance: null, remiseGlobale: null,
  lignes: null, datePiece: null, colonnes: null });
ok(JSON.stringify(await colonnes(cmd.id)) === JSON.stringify(AVEC_TVA), "modifiée sans le menu : mêmes colonnes");

// ---- Pièces -> Modifier la commande : le menu part de SES colonnes.
await page.getByText("Pièces", { exact: true }).first().click();
await page.waitForTimeout(2500);
await page.locator("tr", { hasText: cmd.numero }).getByTitle("Modifier").first().click();
await page.waitForTimeout(1500);
const modif = page.getByRole("dialog");
await modif.getByRole("button", { name: "Colonnes" }).click();
const menuModif = modif.getByRole("group", { name: "Colonnes de cette pièce" });
ok(await menuModif.getByRole("checkbox", { name: "TVA %" }).isChecked()
  && !(await menuModif.getByRole("checkbox", { name: "Remise %" }).isChecked()),
  "Modifier : le menu montre les colonnes de la pièce, pas celles du genre");
await menuModif.getByRole("checkbox", { name: "Remise %" }).check();
// Et pour les prochaines commandes (le patron du banc en a le droit).
await menuModif.getByRole("checkbox", { name: /Garder pour les prochaines/ }).check();
await capture(page, "d58-02-modifier-colonnes");
// Ouvert, le menu couvre le bas de la petite fenêtre : Échap le ferme,
// pas la fenêtre.
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
ok(await menuModif.count() === 0 && await modif.count() === 1, "Échap ferme le menu, la fenêtre reste");
await modif.getByRole("button", { name: "Sauvegarder" }).click();
await page.waitForTimeout(2000);
const REMISE_TVA = ["designation", "quantite", "prix_unitaire", "remise", "tva", "total"];
ok(JSON.stringify(await colonnes(cmd.id)) === JSON.stringify(REMISE_TVA), "Modifier : Remise % cochée, gardée avec la pièce");
const reglageCommande = (await appel("lire_reglages_documents", {})).genres.bon_commande.colonnes;
ok(JSON.stringify(reglageCommande) === JSON.stringify(REMISE_TVA),
  "« Garder pour les prochaines » depuis Modifier : le réglage des bons de commande suit");
// Le banc rejoue sur la même base : le genre revient à l'usine.
await appel("retablir_reglage_document", { genre: "bon_commande" });

// ---- Le papier d'une pièce convertie : commande -> bon de livraison.
const bl = await appel("convertir_piece", { pieceId: cmd.id, nouveauType: "bon_livraison" });
ok(JSON.stringify(await colonnes(bl.id)) === JSON.stringify(REMISE_TVA), `le bon de livraison ${bl.numero} garde les colonnes`);
await page.reload();
await page.waitForTimeout(2000);
await page.getByText("Pièces", { exact: true }).first().click();
await page.waitForTimeout(2500);
await page.locator("tr", { hasText: bl.numero }).getByTitle("Aperçu").first().click();
await page.waitForTimeout(2500);
const papier = page.frameLocator('iframe[title="Aperçu de la pièce"]');
const entetesPapier = (await papier.locator("table.doc-lignes thead").innerText()).replace(/\s+/g, " ");
ok(entetesPapier.includes("Remise"), `le papier du bon montre la colonne Remise (${entetesPapier})`);
ok(!/TVA|HT/.test(entetesPapier), "et ni TVA ni HT : aucune ligne n'en porte (la loi)");
await capture(page, "d58-03-papier-bl-converti");
await page.getByRole("button", { name: "Fermer" }).first().click();

const restes = erreursUtiles(erreurs).filter(sansConsole403);
ok(restes.length === 0, `aucune erreur d'écran${restes.length ? " : " + restes.join(" | ") : ""}`);
console.log(`${ok.total()} vérifications`);
await b.close();
