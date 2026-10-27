// Un jeu de donnees d'essai pour tester Gescom a la main, pose PAR LE
// SERVEUR (ses commandes, ses regles) — jamais en ecrivant dans la base.
//
//   node donnees-essai.mjs [adresse]      (defaut 127.0.0.1:40000)
//
// Sur une base NEUVE (mot de passe d'usine admin / admin123, laisse tel
// quel : l'ecran le fera changer a la premiere connexion). Ce que ca pose :
// le catalogue de catalogue_test.csv, 12 clients, 5 fournisseurs, une
// caisse ouverte, des ventes au comptoir sur trois semaines (passages et
// clients), des ventes a credit dont certaines a moitie reglees, des
// achats (comptant et a credit), des depenses, un article qui ne suit
// pas son stock.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ICI = path.dirname(fileURLToPath(import.meta.url));
const SERVEUR = `http://${process.argv[2] ?? "127.0.0.1:40000"}`;

const r = await fetch(`${SERVEUR}/connexion`, {
  method: "POST", headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ identifiant: "admin", mot_de_passe: "admin123", poste_nom: "Donnees d'essai",
    poste_empreinte: "donnees-essai", version_protocole: 1 }),
});
const session = await r.json();
if (!session.jeton) throw new Error("Connexion refusée (base neuve attendue) : " + JSON.stringify(session));
const jeton = session.jeton;

async function rpc(commande, params = {}) {
  const rep = await fetch(`${SERVEUR}/rpc`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${jeton}` },
    body: JSON.stringify({ commande, params }),
  });
  const corps = await rep.json().catch(() => null);
  if (!corps || corps.etat !== "ok") throw new Error(`${commande} : ${corps?.message ?? rep.status}`);
  return corps.donnee;
}
const ignorer = async (f) => { try { return await f(); } catch { return null; } };
const jour = (n) => new Date(Date.now() - n * 86400000).toISOString().slice(0, 10);
let graine = 7;
const hasard = (n) => { graine = (graine * 16807) % 2147483647; return graine % n; };

// ---- Catalogue.
const csv = fs.readFileSync(path.join(ICI, "..", "..", "catalogue_test.csv"), "utf8").replace(/^﻿/, "");
const imp = await rpc("importer_articles_csv", { contenu: csv, mettreAJour: false });
console.log("catalogue :", JSON.stringify(imp).slice(0, 120));

// ---- Tiers.
const NOMS = ["Amadou Traoré", "Fatoumata Diarra", "Moussa Keïta", "Aminata Coulibaly", "Boubacar Sidibé",
  "Kadiatou Konaté", "Ibrahim Touré", "Mariam Sangaré", "Seydou Diallo", "Awa Camara", "Oumar Cissé", "Salimata Bagayoko"];
const clients = [];
for (const [i, nom] of NOMS.entries()) {
  const c = await rpc("creer_client_rapide", { nom, telephone: `76 ${String(10 + i).padStart(2, "0")} 45 ${String(20 + i * 3).padStart(2, "0")}` });
  clients.push(c.id ?? c);
}
const fournisseurs = [];
for (const nom of ["Sotelma Distribution", "Quincaillerie Ba & Fils", "Grossiste Marché Rose", "Ciments du Mali", "Diakité Import"]) {
  const f = await rpc("creer_fournisseur", { nom, telephone: null, adresse: "Bamako", nif: null, email: null, estVoisin: false });
  fournisseurs.push(f.id ?? f);
}
console.log(`tiers : ${clients.length} clients, ${fournisseurs.length} fournisseurs`);

// ---- La caisse, le magasin, les articles.
await ignorer(() => rpc("ouvrir_session_caisse", { fondOuverture: 50000 }));
const depot = (await rpc("lire_depot_defaut")).id;
const comptant = (await rpc("lire_client_generique")).id;
const articles = (await rpc("lire_articles_avec_unites", {})).filter(a => a.unites?.length && a.stock > 5);
const ligne = (a, q) => {
  const u = a.unites[0];
  return { article_id: a.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
    quantite: q, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference };
};
const panier = () => {
  const n = 1 + hasard(3);
  return Array.from({ length: n }, () => ligne(articles[hasard(articles.length)], 1 + hasard(3)));
};
const total = (lignes) => lignes.reduce((s, l) => s + Math.round(l.prix_pratique * l.quantite), 0);
async function vendre(client, mode, lignes, quand, paye) {
  const v = await rpc("creer_vente", { clientId: client, depotId: depot, modeReglement: mode, lignes,
    montantPaye: paye, modePaiement: "especes", dateVente: quand });
  // Comme l'ecran : la facture du ticket.
  await ignorer(() => rpc("creer_facture_depuis_vente", { venteId: v.vente_id, clientId: client, modeReglement: mode }));
  return v;
}

// ---- Ventes au comptoir sur trois semaines : passants et clients.
let nb = 0;
for (let d = 20; d >= 0; d--) {
  for (let k = 0; k < 1 + hasard(3); k++) {
    const l = panier();
    const client = hasard(3) === 0 ? clients[hasard(clients.length)] : comptant;
    await vendre(client, "comptant", l, d ? jour(d) : null, total(l));
    nb++;
  }
}
console.log(`ventes au comptant : ${nb}`);

// ---- Credits : huit clients, certains a moitie regles.
for (let i = 0; i < 8; i++) {
  const l = panier();
  const v = await vendre(clients[i], "credit", l, jour(18 - i * 2), 0);
  if (i % 2 === 0) await rpc("regler_creance", { venteId: v.vente_id, montant: Math.round(total(l) / 2), mode: "especes" });
}
console.log("ventes à crédit : 8, dont 4 à moitié réglées");

// ---- Achats : comptant et a credit.
for (let i = 0; i < 4; i++) {
  const a = articles[hasard(articles.length)];
  const u = a.unites[0];
  const prix = Math.round(u.prix_reference * 0.8);
  await rpc("enregistrer_achat", {
    fournisseurId: fournisseurs[i], depotId: depot, modeReglement: i < 2 ? "comptant" : "credit",
    modePaiement: "especes", acompte: null, note: null, pieceOrigineId: null, dateReception: null,
    lignes: [{ article_id: a.id, unite_vente_id: u.id, quantite: 10, facteur: u.facteur, prix_achat: prix }],
  });
}
console.log("achats : 4 (2 comptant, 2 à crédit)");

// ---- Depenses du jour.
for (const [libelle, categorie, montant] of [["Loyer boutique", "loyer", 75000], ["Taxi livraison", "transport", 3500],
  ["Facture EDM", "electricite", 18000], ["Thé et sucre", "autre", 1500]]) {
  await ignorer(() => rpc("enregistrer_depense", { montant, libelle, categorie, moyen: "especes" }));
}
console.log("dépenses : 4");

// ---- Un article qui ne suit pas son stock (D36).
const sachet = await rpc("creer_article_complet", { nom: "Sachet plastique", categorieId: null, uniteBase: "pièce", prixReference: 25 });
await rpc("changer_suivi_stock", { articleId: sachet, suivre: false });
console.log("article non suivi : Sachet plastique");
console.log("PRET");
