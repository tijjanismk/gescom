// A-2 : le generateur unique lit les reglages. Rendu genre par genre,
// dans la vraie page (modules servis par Vite), avec les vraies donnees.
import { navigateur, connecter, capture, verifieur, erreursUtiles, png, CAPTURES } from "./pw.mjs";
import fs from "node:fs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const cachet = "data:image/png;base64," + fs.readFileSync(png("cachet.png", 120, 120, [200, 30, 30])).toString("base64");

const res = await page.evaluate(async (cachet) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const { genererImpression } = await import("/src/lib/genererPDF.ts");
  const { genererRecuHTML } = await import("/src/lib/genererRecu.ts");
  const { chargerHabillage } = await import("/src/lib/impression.ts");
  const { genreDePiece } = await import("/src/lib/documents.ts");

  // Réglages : facture A4 avec tout, BL A5 à trois signatures.
  for (const g of ["facture","devis","bon_commande","bon_livraison","recu","releve","ticket"]) await appeler("retablir_reglage_document", { genre: g });
  await appeler("enregistrer_reglage_document", { genre: "facture", reglage: {
    format: "a4", colonne_remise: "oui", colonne_tva: "auto", recap_tva: "oui",
    montant_lettres: true, reference_article: true, mention: "Payable à 30 jours. <b>Merci.</b>",
    signatures: [{ libelle: "Le client" }, { libelle: "Pour la société" }] } });
  await appeler("poser_image_signature", { genre: "facture", rang: 1, image: cachet });
  await appeler("enregistrer_reglage_document", { genre: "bon_livraison", reglage: {
    format: "a5", colonne_remise: "non", colonne_tva: "non", recap_tva: "non",
    montant_lettres: false, reference_article: false, mention: null,
    signatures: [{ libelle: "Le magasinier" }, { libelle: "Le transporteur" }, { libelle: "Reçu par" }] } });
  await appeler("enregistrer_coordonnees_documents", { coordonnees: ["adresse", "telephone", "nif"] });
  await appeler("sauvegarder_parametres_societe", { nom: "Quincaillerie Traoré", adresse: "ACI 2000, Bamako",
    telephone: "76 00 11 22", telephone2: null, email: "q@traore.ml", nif: "NIF-0042", rccm: "RCCM-99",
    siteWeb: null, piedFacture: "Merci de votre confiance" });
  // Pas d'image d'en-tête pour voir les coordonnées ; on la remettra.
  await appeler("supprimer_entete").catch(() => {});
  await appeler("supprimer_pied").catch(() => {});

  const arts = await appeler("lire_articles_avec_unites", {});
  const a0 = arts[0], a1 = arts[1];
  await appeler("sauvegarder_code_barre_article", { articleId: a0.id, codeBarre: "2000000000017" }).catch(() => {});
  const cli = await appeler("creer_client_rapide", { nom: "Coulibaly Awa" });
  const ligne = (a, q, remise, tva) => ({ article_id: a.id, unite_vente_id: a.unites[0].id, quantite: q,
    prix_unitaire: a.unites[0].prix_reference || 1000, remise_pct: remise, taux_tva: tva });
  const creer = (type) => appeler("creer_piece", { clientId: cli.id, typePiece: type,
    lignes: [ligne(a0, 3, 10, 0.18), ligne(a1, 2, 0, 0)], remiseGlobale: 0, dateEcheance: null, note: null,
    pieceOrigineId: null, depotId: null });
  const out = {};
  for (const type of ["facture", "devis", "bon_livraison", "commande_client"]) {
    const p = await creer(type);
    const d = await appeler("lire_donnees_piece", { pieceId: p.id });
    const h = await chargerHabillage(genreDePiece(type));
    const fmt = h.reglage?.format ?? "a4";
    out[type] = { format: fmt, html: genererImpression(d, fmt, h) };
  }
  // Ticket 80 mm de la facture
  const pf = await creer("facture");
  const df = await appeler("lire_donnees_piece", { pieceId: pf.id });
  out.ticket = { format: "thermique_80", html: genererImpression(df, "thermique_80", await chargerHabillage("facture")) };
  // Un reçu (A5, historique) : données fabriquées, habillage réel
  const hr = await chargerHabillage("recu");
  out.recu_fournisseur = { format: "a5", html: genererRecuHTML({ cote: "fournisseur", montant: 125000, mode: "especes",
    date: new Date().toISOString(), auteur: "Patron", reference: "FAF-2026-00001", reste_du: 0,
    tiers: { nom: "Grossiste Diallo" }, societe: { nom: "Quincaillerie Traoré", adresse: "ACI 2000", telephone: "76", nif: "NIF-0042" } }, hr) };
  return out;
}, cachet);

// Chaque document dans une iframe à sa largeur réelle, capture.
const largeur = { a4: 794, a5: 559, thermique_80: 302 };
for (const [nom, { format, html }] of Object.entries(res)) {
  fs.writeFileSync(`${CAPTURES}/a2-${nom}.html`, html);
  const p2 = await b.newPage({ viewport: { width: largeur[format] ?? 794, height: 1123 } });
  await p2.setContent(html.replace(/<script>[\s\S]*?<\/script>/g, ""));
  await p2.screenshot({ path: `${CAPTURES}/a2-${nom}.png`, fullPage: true });
  await p2.close();
}
const f = res.facture.html, bl = res.bon_livraison.html, dv = res.devis.html;
ok(res.facture.format === "a4" && res.bon_livraison.format === "a5", "format par défaut du genre (A4 / A5)");
ok(f.includes(">Remise</th>") && f.includes(">Réf.</th>") && f.includes("2000000000017"), "facture : colonnes remise et référence");
ok(f.includes("doc-recap-tva") && f.includes("18 %"), "facture : récapitulatif TVA");
ok(/Arrêtée la présente facture/.test(f), "facture : montant en lettres");
ok(f.includes("Payable à 30 jours. <b>Merci.</b>"), "facture : mention réglée (HTML brut)");
ok(f.includes(">Le client<") && f.includes(">Pour la société<") && f.includes("data:image/png;base64"), "facture : signatures et cachet");
ok(f.includes("NIF : NIF-0042") && !f.includes("RCCM") && !f.includes("q@traore.ml"), "coordonnées cochées seulement");
ok(!bl.includes(">Remise</th>") && !bl.includes(">TVA</th>") && !bl.includes("doc-recap-tva"), "BL : colonnes jamais");
ok(!bl.includes("Arrêté"), "BL : pas de montant en lettres");
ok(bl.includes(">Le magasinier<") && bl.includes(">Le transporteur<") && bl.includes(">Reçu par<"), "BL : trois signatures");
ok(dv.includes(">Le vendeur<") && dv.includes("Merci de votre confiance"), "devis : usine + pied de facture");
ok(!res.ticket.html.includes("doc-signatures"), "ticket : pas de signatures");
ok(res.recu_fournisseur.html.includes(">Le bénéficiaire<"), "reçu fournisseur : « Le caissier » retourné");
console.log(`${ok.total()} vérifications — erreurs :`, erreursUtiles(erreurs));
await b.close();
