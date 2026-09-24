// lib/exemples-documents.ts — un document d'exemple par genre (v3, A-2).
//
// Paramètres → Documents montre l'effet d'un réglage AVANT de
// l'enregistrer : le vrai générateur, des données d'exemple, la société
// réelle. Deux lignes, une remise, de la TVA — de quoi voir chaque case.

import { genererImpression, type DonneesPiece, type FormatImpression } from "@/lib/genererPDF";
import { genererRecuHTML } from "@/lib/genererRecu";
import { genererReleveHTML } from "@/lib/genererReleve";
import { genererBulletinHTML } from "@/lib/genererBulletin";
import type { Habillage } from "@/lib/impression";
import type { Genre } from "@/lib/documents";

const TYPE_EXEMPLE: Record<Genre, string> = {
  facture: "facture", devis: "devis", bon_commande: "commande_client",
  bon_livraison: "bon_livraison", recu: "facture", releve: "facture", ticket: "facture", bulletin: "facture",
};

function pieceExemple(genre: Genre, societe: Record<string, unknown>): DonneesPiece {
  const maintenant = new Date().toISOString();
  return {
    piece: {
      type_piece: TYPE_EXEMPLE[genre], numero: "EXEMPLE-0001", date_piece: maintenant,
      date_echeance: null, remise_globale: 0, note: null, tiers_type: "client",
      client_nom: "Coulibaly Awa", client_code: "CLIENT00042", client_telephone: "76 12 34 56",
    },
    lignes: [
      { article_nom: "Ciment CPA 50 kg", article_reference: "2000000000017", unite_libelle: "sac",
        quantite: 10, prix_unitaire: 5500, remise_pct: 5, remise_montant: 2750,
        taux_tva: 0.18, montant_tva: 9405, montant_ht: 52250 },
      { article_nom: "Fer à béton 10 mm", article_reference: null, unite_libelle: "barre",
        quantite: 20, prix_unitaire: 3000, remise_pct: 0, remise_montant: 0,
        taux_tva: 0, montant_tva: 0, montant_ht: 60000 },
    ],
    societe,
    totaux: {
      total_ht: 112250, total_tva: 9405, remise_globale: 0, remise_montant: 0,
      total_net: 112250, total_ttc: 121655, total_paye: 50000, reste_du: 71655,
    },
  };
}

/** Le document d'exemple du genre, habillé comme on le règle. */
export function documentExemple(
  genre: Genre, format: FormatImpression, h: Habillage, societe: Record<string, unknown>,
): string {
  if (genre === "recu") {
    return genererRecuHTML({
      cote: "client", montant: 50000, mode: "especes", date: new Date().toISOString(),
      auteur: "Le caissier", reference: "FAC-EXEMPLE-0001", reste_du: 71655,
      tiers: { nom: "Coulibaly Awa", code: "CLIENT00042" },
      societe: societe as never,
    }, h);
  }
  if (genre === "releve") {
    return genererReleveHTML({
      tiers: { nom: "Coulibaly Awa", code: "CLIENT00042" },
      lignes: [
        { date: new Date().toISOString(), numero: "FAC-EXEMPLE-0001", total: 121655, paye: 50000, reste: 71655 },
      ],
      total_du: 71655, avoirs: 0, net_du: 71655,
      societe: societe as never,
    }, "client", h);
  }
  if (genre === "bulletin") {
    const auj = new Date().toISOString();
    return genererBulletinHTML({
      numero: "PAIE-EXEMPLE-0001", statut: "validee", nom: "Coulibaly Awa", fonction: "vendeuse",
      du: auj.slice(0, 8) + "01", au: auj.slice(0, 10), valide_le: auj,
      brut: 81250, retenues: 12000, net: 69250, reporte: 0, verse: 50000, reste: 19250,
      lignes: [
        { genre: "base", libelle: "Salaire du mois", montant: 60000 },
        { genre: "commission", libelle: "2,5 % de 850 000 F de ventes signées", montant: 21250 },
        { genre: "retenue", libelle: "Casse d'un carton", montant: -2000 },
        { genre: "avance", libelle: "Avance du 05", montant: -10000 },
      ],
      versements: [{ montant: 50000, moyen: "especes", date: auj }],
      personne: { telephone: "76 12 34 56" },
      societe,
    }, h);
  }
  return genererImpression(pieceExemple(genre, societe), format, h);
}
