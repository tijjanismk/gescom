// lib/modeles/contexte.ts — normaliser les données pour les modèles.
//
// Chaque écran a sa forme de données, héritée de l'endroit où elle a
// été écrite : la pièce range le tiers sous `client_nom` même quand
// c'est un fournisseur, le reçu le range sous `tiers.nom`. Un modèle ne
// doit pas connaître ces accidents d'histoire : il lit `tiers.nom`,
// point.
//
// Ce fichier est donc la seule couche qui sait d'où vient quoi. Si un
// chemin change dans le catalogue des champs, c'est ici qu'on le
// raccorde — pas dans les modèles, qui sont chez le commerçant et qu'on
// ne peut pas éditer à distance.

import { montantEnLettres } from "./rendu";

const LIBELLE_PIECE: Record<string, string> = {
  facture: "FACTURE",
  facture_acompte: "FACTURE D'ACOMPTE",
  devis: "DEVIS",
  bon_commande: "BON DE COMMANDE",
  bon_livraison: "BON DE LIVRAISON",
  bon_reception: "BON DE RÉCEPTION",
  avoir: "AVOIR",
  proforma: "PROFORMA",
};

export function libellePiece(type: string): string {
  return LIBELLE_PIECE[type] ?? type.replace(/_/g, " ").toUpperCase();
}

/** La forme rendue par `lire_donnees_piece`. */
export interface DonneesPieceBrutes {
  piece: Record<string, unknown>;
  lignes: Record<string, unknown>[];
  societe: Record<string, unknown>;
  totaux: Record<string, number>;
}

export function contextePiece(d: DonneesPieceBrutes): Record<string, unknown> {
  const devise = (d.societe?.devise as string) || "FCFA";
  const totaux = d.totaux ?? {};

  return {
    societe: { devise, ...d.societe },
    piece: {
      ...d.piece,
      type_libelle: libellePiece(String(d.piece?.type_piece ?? "")),
    },
    // Le tiers est aplati depuis les clés `client_*`, qui portent aussi
    // les fournisseurs — un héritage de l'époque où seules les pièces
    // client s'imprimaient.
    tiers: {
      nom: d.piece?.client_nom ?? "",
      code: d.piece?.client_code ?? "",
      telephone: d.piece?.client_telephone ?? "",
      adresse: d.piece?.client_adresse ?? "",
      nif: d.piece?.client_nif ?? "",
      type: d.piece?.tiers_type ?? "client",
    },
    lignes: (d.lignes ?? []).map((l) => ({
      ...l,
      // Le TTC par ligne n'est pas stocké : il n'a de sens qu'à
      // l'impression, et le recalculer ici évite qu'un modèle le
      // reconstitue à sa façon.
      montant_ttc:
        Number(l.montant_ht ?? 0) + Number(l.montant_tva ?? 0),
    })),
    totaux: {
      ...totaux,
      remise: totaux.remise_montant ?? 0,
      total_en_lettres: montantEnLettres(totaux.total_ttc ?? 0, devise),
    },
  };
}

/** La forme rendue par l'écran de règlement (`DonneesRecu`). */
export interface DonneesRecuBrutes {
  cote: "client" | "fournisseur";
  montant: number;
  mode: string;
  date: string;
  note?: string | null;
  auteur: string;
  reference: string;
  reste_du: number | null;
  est_annulation?: boolean;
  tiers: Record<string, unknown>;
  societe: Record<string, unknown>;
}

const MOYENS: Record<string, string> = {
  especes: "Espèces",
  orange_money: "Orange Money",
  moov_money: "Moov Money",
  cheque: "Chèque",
  avoir: "Avoir",
  virement: "Virement",
};

export function contexteRecu(d: DonneesRecuBrutes): Record<string, unknown> {
  const devise = (d.societe?.devise as string) || "FCFA";
  return {
    societe: { devise, ...d.societe },
    tiers: d.tiers,
    piece: {
      numero: d.reference,
      date_piece: d.date,
      type_libelle: d.est_annulation
        ? "ANNULATION DE RÈGLEMENT"
        : d.cote === "client"
          ? "REÇU DE RÈGLEMENT"
          : "REÇU DE PAIEMENT",
      auteur_nom: d.auteur,
      note: d.note ?? "",
    },
    paiement: {
      montant: d.montant,
      mode: MOYENS[d.mode] ?? d.mode,
      date: d.date,
      reference: d.reference,
      encaisse_par: d.auteur,
    },
    totaux: {
      total_ttc: d.montant,
      reste_du: d.reste_du ?? 0,
      total_en_lettres: montantEnLettres(d.montant, devise),
    },
    lignes: [],
  };
}

/** La forme rendue par l'état de créance (`DonneesReleve`). */
export interface DonneesReleveBrutes {
  tiers: Record<string, unknown>;
  lignes: Record<string, unknown>[];
  total_du: number;
  avoirs: number;
  net_du: number;
  societe: Record<string, unknown>;
  periode?: string;
}

export function contexteReleve(d: DonneesReleveBrutes): Record<string, unknown> {
  const devise = (d.societe?.devise as string) || "FCFA";
  return {
    societe: { devise, ...d.societe },
    tiers: d.tiers,
    piece: {
      type_libelle: "ÉTAT DE CRÉANCE",
      date_piece: new Date().toISOString(),
    },
    // `factures` et non `lignes` : un relevé ne liste pas des articles.
    // Le nom du chemin doit dire ce qu'on parcourt, sinon l'éditeur
    // propose des colonnes qui n'ont aucun sens ici.
    factures: d.lignes,
    releve: {
      periode: d.periode ?? "",
      solde: d.net_du,
      nb_factures: d.lignes?.length ?? 0,
    },
    totaux: {
      total_ttc: d.total_du,
      remise: d.avoirs,
      reste_du: d.net_du,
      total_en_lettres: montantEnLettres(d.net_du, devise),
    },
  };
}

export interface MouvementCaisse {
  heure: string;
  motif: string;
  libelle: string;
  moyen: string;
  entree: number;
  sortie: number;
}

export interface DonneesJournalBrutes {
  date: string;
  fond_ouverture: number;
  entrees: number;
  sorties: number;
  solde_theorique: number;
  especes_comptees: number | null;
  ecart: number | null;
  ouvert_par: string;
  mouvements: MouvementCaisse[];
  societe: Record<string, unknown>;
}

export function contexteJournal(d: DonneesJournalBrutes): Record<string, unknown> {
  const devise = (d.societe?.devise as string) || "FCFA";
  return {
    societe: { devise, ...d.societe },
    piece: {
      type_libelle: "JOURNAL DE CAISSE",
      date_piece: d.date,
      auteur_nom: d.ouvert_par,
    },
    tiers: { nom: (d.societe?.nom as string) ?? "" },
    mouvements: d.mouvements,
    journal: {
      date: d.date,
      fond_ouverture: d.fond_ouverture,
      entrees: d.entrees,
      sorties: d.sorties,
      solde_theorique: d.solde_theorique,
      especes_comptees: d.especes_comptees,
      ecart: d.ecart,
      ouvert_par: d.ouvert_par,
    },
    totaux: {
      total_ttc: d.solde_theorique,
      reste_du: d.ecart ?? 0,
      total_en_lettres: montantEnLettres(d.solde_theorique, devise),
    },
  };
}

// =====================================================================
//  Un jeu d'essai, pour l'éditeur
// =====================================================================

/**
 * L'aperçu ne doit jamais montrer une page vide.
 *
 * Sans données, le commerçant règle sa mise en page à l'aveugle : les
 * colonnes semblent bien larges tant qu'aucun nom d'article ne les
 * remplit. Ces valeurs sont volontairement longues — « Ciment CIMAF
 * 42,5 R sac de 50 kg » est un libellé réel, et c'est lui qui révèle
 * une colonne trop étroite.
 */
export function contexteExemple(genre: string): Record<string, unknown> {
  const societe = {
    nom: "Établissements du Fleuve",
    adresse: "Rue 224, Porte 87 — Hamdallaye ACI 2000, Bamako",
    telephone: "+223 76 12 34 56",
    telephone2: "+223 66 98 76 54",
    email: "contact@ets-fleuve.ml",
    nif: "084512345 X",
    rccm: "MA.BKO.2019.B.1234",
    devise: "FCFA",
  };
  const tiers = {
    nom: "Amadou Traoré",
    code: "CLI00027",
    telephone: "+223 91 28 71 39",
    adresse: "Badalabougou Est, Bamako",
    nif: "",
    type: "client",
  };
  const lignes = [
    {
      article_nom: "Ciment CIMAF 42,5 R — sac de 50 kg",
      unite_libelle: "Sac",
      quantite: 62,
      prix_unitaire: 5500,
      remise_pct: 0,
      montant_ht: 341000,
      taux_tva: 0.18,
      montant_tva: 61380,
      montant_ttc: 402380,
    },
    {
      article_nom: "Tôle ondulée BG28 — 3 m",
      unite_libelle: "Feuille",
      quantite: 54,
      prix_unitaire: 7000,
      remise_pct: 5,
      montant_ht: 359100,
      taux_tva: 0.18,
      montant_tva: 64638,
      montant_ttc: 423738,
    },
    {
      article_nom: "Robinet laiton 1/2 pouce",
      unite_libelle: "Pièce",
      quantite: 12,
      prix_unitaire: 3500,
      remise_pct: 0,
      montant_ht: 42000,
      taux_tva: 0.18,
      montant_tva: 7560,
      montant_ttc: 49560,
    },
  ];
  const total_ht = 742100;
  const total_tva = 133578;
  const total_ttc = 875678;

  const base: Record<string, unknown> = {
    societe,
    tiers,
    piece: {
      numero: "FAC-2026-00042",
      type_piece: "facture",
      type_libelle: "FACTURE",
      date_piece: new Date().toISOString(),
      date_echeance: new Date(Date.now() + 30 * 864e5).toISOString(),
      statut: "emis",
      note: "Livraison sur chantier Sébénikoro, à la charge du client.",
      auteur_nom: "Awa Coulibaly",
    },
    lignes,
    factures: [
      { date: new Date().toISOString(), numero: "FAC-2026-00031", total: 615000, paye: 247500, reste: 367500 },
      { date: new Date().toISOString(), numero: "FAC-2026-00038", total: 430000, paye: 0, reste: 430000 },
    ],
    mouvements: [
      { heure: "08:15", motif: "Ouverture", libelle: "Fond de caisse", moyen: "Espèces", entree: 50000, sortie: 0 },
      { heure: "10:42", motif: "Vente", libelle: "FAC-2026-00040", moyen: "Espèces", entree: 128000, sortie: 0 },
      { heure: "14:07", motif: "Dépense", libelle: "Transport livraison", moyen: "Espèces", entree: 0, sortie: 12000 },
    ],
    paiement: {
      montant: 247500,
      mode: "Espèces",
      date: new Date().toISOString(),
      reference: "FAC-2026-00031",
      encaisse_par: "Awa Coulibaly",
    },
    releve: { periode: "Depuis le 1er janvier 2026", solde: 797500, nb_factures: 2 },
    journal: {
      date: new Date().toISOString(),
      fond_ouverture: 50000,
      entrees: 128000,
      sorties: 12000,
      solde_theorique: 166000,
      especes_comptees: 154000,
      ecart: -12000,
      ouvert_par: "Awa Coulibaly",
    },
    totaux: {
      total_ht,
      remise: 18900,
      remise_montant: 18900,
      total_tva,
      total_ttc,
      total_paye: 200000,
      reste_du: 675678,
      total_en_lettres: montantEnLettres(total_ttc, "FCFA"),
    },
  };

  if (genre === "recu_paiement") {
    (base.piece as Record<string, unknown>).type_libelle = "REÇU DE RÈGLEMENT";
  } else if (genre === "releve_creance") {
    (base.piece as Record<string, unknown>).type_libelle = "ÉTAT DE CRÉANCE";
  } else if (genre === "journal_caisse") {
    (base.piece as Record<string, unknown>).type_libelle = "JOURNAL DE CAISSE";
  } else if (genre === "bon_sortie") {
    (base.piece as Record<string, unknown>).type_libelle = "BON DE SORTIE";
  }
  return base;
}
