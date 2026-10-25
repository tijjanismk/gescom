// Retour du 25/09 : sur une pièce de plusieurs pages, l'en-tête n'était
// qu'en haut de la première et le pied qu'au bas de la dernière. Ici :
// une facture de 70 lignes imprimée en PDF par Chromium (le moteur de la
// fenêtre), chaque page lue pixel par pixel — l'en-tête en haut, le pied
// en bas, collés au bord du papier, le filigrane au milieu. Puis les
// signatures soulignées, et le filigrane réglé par l'écran.
import { navigateur, connecter, capture, verifieur, erreursUtiles, png, parametres, TRAVAIL, CAPTURES } from "./pw.mjs";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);

const b64 = f => fs.readFileSync(f).toString("base64");
const BLEU = [20, 60, 200], VERT = [20, 140, 60], ROUGE = [220, 20, 20];
const entete = b64(png("i2-entete.png", 1200, 150, BLEU));
const pied = b64(png("i2-pied.png", 1200, 90, VERT));
const marque = "data:image/png;base64," + b64(png("i2-filigrane.png", 300, 300, ROUGE));

const docs = await page.evaluate(async ({ entete, pied, marque }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const { genererImpression } = await import("/src/lib/genererPDF.ts");
  const { chargerHabillage } = await import("/src/lib/impression.ts");
  await appeler("retablir_reglage_document", { genre: "facture" });
  await appeler("sauvegarder_entete", { nom: "entete.png", contenu: entete });
  await appeler("sauvegarder_pied", { nom: "pied.png", contenu: pied });
  // Net (flou 0) et assez fort pour se mesurer ; la vraie boutique le floute.
  await appeler("enregistrer_filigrane_documents", { reglage: { texte: null, opacite: 25, flou: 0, taille: 40 } });
  await appeler("poser_image_filigrane", { image: marque });

  const arts = await appeler("lire_articles_avec_unites", {});
  const cli = await appeler("creer_client_rapide", { nom: "Diarra Moussa" });
  const lignes = Array.from({ length: 70 }, (_, i) => {
    const a = arts[i % arts.length];
    return { article_id: a.id, unite_vente_id: a.unites[0].id, quantite: 1 + (i % 3),
      prix_unitaire: a.unites[0].prix_reference || 1000, remise_pct: 0, taux_tva: 0 };
  });
  const p = await appeler("creer_piece", { clientId: cli.id, typePiece: "facture", lignes, remiseGlobale: 0,
    dateEcheance: null, note: null, pieceOrigineId: null, depotId: null });
  const d = await appeler("lire_donnees_piece", { pieceId: p.id });
  const h = await chargerHabillage("facture");
  // Et la facture suivie de son bon de sortie, dans le même document.
  const court = await appeler("creer_piece", { clientId: cli.id, typePiece: "facture", lignes: lignes.slice(0, 3),
    remiseGlobale: 0, dateEcheance: null, note: null, pieceOrigineId: null, depotId: null });
  const dc = await appeler("lire_donnees_piece", { pieceId: court.id });
  return { facture: genererImpression(d, "a4", h), avecBon: genererImpression(dc, "a5_et_bon", h) };
}, { entete, pied, marque });
const html = docs.facture;

// Le PDF, comme la boîte d'impression le ferait (format et marges de la page CSS).
const pdf = path.join(CAPTURES, "i2-facture-70-lignes.pdf");
fs.writeFileSync(path.join(CAPTURES, "i2-facture-70-lignes.html"), html);
const p2 = await b.newPage();
await p2.setContent(html.replace(/<script>[\s\S]*?<\/script>/g, ""), { waitUntil: "load" });
await p2.pdf({ path: pdf, preferCSSPageSize: true });
await p2.close();

// Chaque page en PPM (en-tête « P6 L H 255 » puis RVB brut) : lisible sans bibliothèque.
function enPages(fichierPdf, nom) {
  for (const f of fs.readdirSync(TRAVAIL)) if (f.startsWith(nom)) fs.rmSync(path.join(TRAVAIL, f));
  execFileSync("pdftoppm", ["-r", "40", fichierPdf, path.join(TRAVAIL, nom)]);
  return fs.readdirSync(TRAVAIL).filter(f => f.startsWith(nom) && f.endsWith(".ppm")).sort();
}
const pages = enPages(pdf, "i2-page");
const lire = f => {
  const buf = fs.readFileSync(path.join(TRAVAIL, f));
  const txt = buf.subarray(0, 40).toString("latin1").split(/\s+/);
  const [l, h] = [Number(txt[1]), Number(txt[2])];
  const debut = buf.length - l * h * 3;
  return { l, h, px: (x, y) => { const i = debut + (y * l + x) * 3; return [buf[i], buf[i + 1], buf[i + 2]]; } };
};
const proche = (c, ref) => c.every((v, i) => Math.abs(v - ref[i]) < 40);

ok(pages.length >= 2, `la facture de 70 lignes tient sur ${pages.length} pages`);
pages.forEach((f, n) => {
  const { l, h, px } = lire(f);
  // Le dernier pixel de droite est à cheval sur le bord (210 mm ne tombe
  // pas juste en pixels) : on lit l'avant-dernier.
  const haut = [px(0, 1), px(l >> 1, 2), px(l - 2, 1)];
  const bas = [px(0, h - 3), px(l >> 1, h - 3), px(l - 2, h - 3)];
  ok(haut.every(c => proche(c, BLEU)), `page ${n + 1} : l'en-tête en haut, bord à bord`);
  ok(bas.every(c => proche(c, VERT)), `page ${n + 1} : le pied en bas, bord à bord`);
  // Le filigrane : une teinte rouge, légère, au milieu de la page.
  let teinte = 0;
  for (let dy = -4; dy <= 4; dy++) for (let dx = -4; dx <= 4; dx++) {
    const [r, g] = px((l >> 1) + dx, (h >> 1) + dy);
    if (r > 200 && g < 225 && r - g > 15) teinte++;
  }
  ok(teinte > 20, `page ${n + 1} : le filigrane au milieu (${teinte}/81 points teintés)`);
});

// Facture + bon de sortie : deux pages A5, les bandeaux sur les deux.
const pdfBon = path.join(CAPTURES, "i2-facture-et-bon.pdf");
const p3 = await b.newPage();
await p3.setContent(docs.avecBon.replace(/<script>[\s\S]*?<\/script>/g, ""), { waitUntil: "load" });
await p3.pdf({ path: pdfBon, preferCSSPageSize: true });
await p3.close();
const pagesBon = enPages(pdfBon, "i2-bon");
ok(pagesBon.length === 2, `facture et bon de sortie : ${pagesBon.length} pages`);
ok(pagesBon.every(f => { const { l, h, px } = lire(f); return proche(px(0, 1), BLEU) && proche(px(l - 2, h - 3), VERT); }),
  "le bon de sortie a aussi l'en-tête et le pied");
ok(docs.avecBon.includes("BON DE SORTIE"), "le bon suit la facture dans le même document");

ok(/doc-sig-libelle[^>]*text-decoration:underline[^>]*>Pour acquit</.test(html)
  && /text-decoration:underline[^>]*>Le fournisseur</.test(html), "« Pour acquit » et « Le fournisseur » soulignés");
ok(!html.includes('<div style="border-bottom:1px solid #000"></div>'), "plus de trait séparé à distance du libellé");

// Par l'écran : Paramètres → Documents, un mot en filigrane, enregistré, relu.
await parametres(page, "Documents");
ok(await page.getByRole("heading", { name: "Filigrane" }).count() === 1, "Paramètres → Documents : la section Filigrane");
ok(await page.getByText("L'image posée l'emporte sur le mot.").count() === 1, "l'image posée est montrée, et qu'elle l'emporte");
await page.getByLabel("Ou un mot").fill("COPIE");
await page.getByLabel("Intensité").fill("12");
await page.getByRole("button", { name: /Enregistrer le filigrane/ }).click();
await page.waitForTimeout(800);
ok(await page.getByText("Filigrane enregistré.").count() === 1, "filigrane enregistré par l'écran");
await capture(page, "i2-reglage-filigrane");
const relu = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_reglages_documents")).filigrane;
});
ok(relu.texte === "COPIE" && relu.opacite === 12 && relu.image, "relu du serveur : le mot, l'intensité, l'image gardée");
ok(await page.getByText("Filigrane", { exact: true }).count() >= 1, "chaque document a sa case « Filigrane »");

// Ne rien laisser aux parcours suivants.
await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("poser_image_filigrane", { image: null });
  await appeler("enregistrer_filigrane_documents", { reglage: { texte: null, opacite: 8, flou: 1, taille: 60 } });
  await appeler("supprimer_entete").catch(() => {});
  await appeler("supprimer_pied").catch(() => {});
});

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
