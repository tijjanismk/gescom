// lib/modeles/types.ts — la forme d'un modèle de document.
//
// Un modèle est une liste de BLOCS empilés du haut vers le bas. Pas de
// positionnement libre en x/y : sur un document commercial, ce qui doit
// pouvoir bouger, c'est l'ordre et le contenu, pas les coordonnées. Un
// canevas libre laisserait poser un total à cheval sur le pied de page,
// et la facture sortirait fausse sans que l'écran l'ait montré.
//
// Chaque bloc lit ses données par un CHEMIN (`piece.numero`,
// `totaux.total_ttc`). Le chemin est une chaîne : c'est ce qui permet
// d'ajouter un champ au document sans toucher au code.

export type GenreDocument =
  | "facture"
  | "bon_sortie"
  | "recu_paiement"
  | "releve_creance"
  | "journal_caisse";

export const GENRES: { id: GenreDocument; nom: string; description: string }[] = [
  {
    id: "facture",
    nom: "Facture / pièce",
    description: "Facture, devis, bon de livraison, avoir — la pièce commerciale.",
  },
  {
    id: "bon_sortie",
    nom: "Bon de sortie",
    description: "Le même document sans aucun montant, remis au magasinier.",
  },
  {
    id: "recu_paiement",
    nom: "Reçu de paiement",
    description: "Remis au client contre son règlement.",
  },
  {
    id: "releve_creance",
    nom: "Relevé de créance",
    description: "L'état des sommes dues par un client.",
  },
  {
    id: "journal_caisse",
    nom: "Journal de caisse",
    description: "La clôture du jour : entrées, sorties, écart.",
  },
];

export type FormatPapier = "a4" | "a5" | "thermique_58" | "thermique_80";

export const FORMATS: { id: FormatPapier; nom: string; largeurMm: number }[] = [
  { id: "a4", nom: "A4", largeurMm: 210 },
  { id: "a5", nom: "A5", largeurMm: 148 },
  { id: "thermique_80", nom: "Thermique 80 mm", largeurMm: 80 },
  { id: "thermique_58", nom: "Thermique 58 mm", largeurMm: 58 },
];

export type Alignement = "gauche" | "centre" | "droite";
export type FormatValeur = "texte" | "montant" | "nombre" | "date" | "date_heure";

export interface Colonne {
  id: string;
  libelle: string;
  /** Chemin relatif à la ligne : `article_nom`, `quantite`… */
  chemin: string;
  /** Pourcentage de la largeur du tableau. */
  largeur: number;
  alignement: Alignement;
  format: FormatValeur;
}

export interface Champ {
  id: string;
  libelle: string;
  chemin: string;
  format: FormatValeur;
  /** Masque la ligne si la valeur est vide — un NIF absent ne doit pas
   *  imprimer « NIF : » suivi de rien. */
  masquerSiVide: boolean;
}

interface BlocBase {
  id: string;
  /** Décoché, le bloc reste dans le modèle mais ne s'imprime pas. On
   *  éteint une mention légale le temps d'une saison sans la perdre. */
  visible: boolean;
}

export type Bloc =
  | (BlocBase & {
      type: "entete";
      /** Clé de l'image en base64 côté données : `logo`, `entete`. */
      image: "logo" | "entete" | "aucune";
      hauteurMm: number;
      afficherSociete: boolean;
      alignement: Alignement;
    })
  | (BlocBase & {
      type: "titre";
      texte: string;
      alignement: Alignement;
      taillePt: number;
      trait: boolean;
    })
  | (BlocBase & {
      type: "champs";
      titre: string;
      colonnes: 1 | 2 | 3;
      items: Champ[];
    })
  | (BlocBase & {
      type: "tableau";
      /** Chemin de la liste : `lignes`, `mouvements`… */
      source: string;
      colonnes: Colonne[];
      zebre: boolean;
      /** Texte affiché quand la liste est vide. */
      siVide: string;
    })
  | (BlocBase & {
      type: "totaux";
      items: Champ[];
      /** Le dernier total est mis en évidence. */
      accentuerDernier: boolean;
    })
  | (BlocBase & {
      type: "texte";
      contenu: string;
      alignement: Alignement;
      taillePt: number;
      italique: boolean;
      cadre: boolean;
    })
  | (BlocBase & {
      type: "signatures";
      gauche: string;
      droite: string;
    })
  | (BlocBase & { type: "trait" })
  | (BlocBase & { type: "espace"; hauteurMm: number })
  | (BlocBase & { type: "saut_page" });

export type TypeBloc = Bloc["type"];

export interface ReglagesPage {
  margeMm: number;
  taillePt: number;
  police: string;
  couleurAccent: string;
}

export interface ContenuModele {
  version: 1;
  page: ReglagesPage;
  blocs: Bloc[];
}

export interface Modele {
  id: string;
  genre: GenreDocument;
  nom: string;
  format: FormatPapier;
  contenu: ContenuModele;
  est_defaut: boolean;
  actif: boolean;
  modifie_le?: string;
}

export interface BilanImport {
  ajoutes: number;
  remplaces: number;
  ignores: string[];
}

// =====================================================================
//  Le catalogue des champs disponibles
// =====================================================================
//
// Ce n'est pas de la documentation : c'est ce que l'éditeur propose
// dans ses listes déroulantes. Un chemin absent d'ici reste saisissable
// à la main — l'éditeur ne doit pas empêcher d'atteindre une donnée
// qu'on aurait oublié de déclarer.

export interface ChampDisponible {
  chemin: string;
  libelle: string;
  format: FormatValeur;
}

export const CHAMPS_SOCIETE: ChampDisponible[] = [
  { chemin: "societe.nom", libelle: "Nom de la société", format: "texte" },
  { chemin: "societe.adresse", libelle: "Adresse", format: "texte" },
  { chemin: "societe.telephone", libelle: "Téléphone", format: "texte" },
  { chemin: "societe.telephone2", libelle: "Téléphone 2", format: "texte" },
  { chemin: "societe.email", libelle: "E-mail", format: "texte" },
  { chemin: "societe.nif", libelle: "NIF", format: "texte" },
  { chemin: "societe.rccm", libelle: "RCCM", format: "texte" },
  { chemin: "societe.devise", libelle: "Devise", format: "texte" },
];

export const CHAMPS_PIECE: ChampDisponible[] = [
  { chemin: "piece.numero", libelle: "Numéro", format: "texte" },
  { chemin: "piece.type_libelle", libelle: "Type de pièce", format: "texte" },
  { chemin: "piece.date_piece", libelle: "Date", format: "date" },
  { chemin: "piece.date_echeance", libelle: "Échéance", format: "date" },
  { chemin: "piece.statut", libelle: "Statut", format: "texte" },
  { chemin: "piece.note", libelle: "Note", format: "texte" },
  { chemin: "piece.auteur_nom", libelle: "Établi par", format: "texte" },
  { chemin: "tiers.nom", libelle: "Nom du tiers", format: "texte" },
  { chemin: "tiers.code", libelle: "Code client", format: "texte" },
  { chemin: "tiers.telephone", libelle: "Téléphone du tiers", format: "texte" },
  { chemin: "tiers.adresse", libelle: "Adresse du tiers", format: "texte" },
  { chemin: "tiers.nif", libelle: "NIF du tiers", format: "texte" },
];

export const CHAMPS_TOTAUX: ChampDisponible[] = [
  { chemin: "totaux.total_ht", libelle: "Total HT", format: "montant" },
  { chemin: "totaux.remise", libelle: "Remise", format: "montant" },
  { chemin: "totaux.total_tva", libelle: "TVA", format: "montant" },
  { chemin: "totaux.total_ttc", libelle: "Total TTC", format: "montant" },
  { chemin: "totaux.total_paye", libelle: "Déjà payé", format: "montant" },
  { chemin: "totaux.reste_du", libelle: "Reste dû", format: "montant" },
  { chemin: "totaux.total_en_lettres", libelle: "Montant en lettres", format: "texte" },
];

export const COLONNES_LIGNES: ChampDisponible[] = [
  { chemin: "article_nom", libelle: "Désignation", format: "texte" },
  { chemin: "unite_libelle", libelle: "Unité", format: "texte" },
  { chemin: "quantite", libelle: "Quantité", format: "nombre" },
  { chemin: "prix_unitaire", libelle: "Prix unitaire", format: "montant" },
  { chemin: "remise_pct", libelle: "Remise %", format: "nombre" },
  { chemin: "montant_ht", libelle: "Montant HT", format: "montant" },
  { chemin: "taux_tva", libelle: "Taux TVA", format: "nombre" },
  { chemin: "montant_tva", libelle: "Montant TVA", format: "montant" },
  { chemin: "montant_ttc", libelle: "Montant TTC", format: "montant" },
];

/** Ce que chaque genre expose en plus du tronc commun. */
export const CHAMPS_PAR_GENRE: Record<GenreDocument, ChampDisponible[]> = {
  facture: [],
  bon_sortie: [],
  recu_paiement: [
    { chemin: "paiement.montant", libelle: "Montant reçu", format: "montant" },
    { chemin: "paiement.mode", libelle: "Mode de règlement", format: "texte" },
    { chemin: "paiement.date", libelle: "Date du règlement", format: "date_heure" },
    { chemin: "paiement.reference", libelle: "Référence", format: "texte" },
    { chemin: "paiement.encaisse_par", libelle: "Encaissé par", format: "texte" },
  ],
  releve_creance: [
    { chemin: "releve.periode", libelle: "Période", format: "texte" },
    { chemin: "releve.solde", libelle: "Solde dû", format: "montant" },
    { chemin: "releve.nb_factures", libelle: "Nombre de factures", format: "nombre" },
  ],
  journal_caisse: [
    { chemin: "journal.date", libelle: "Date du journal", format: "date" },
    { chemin: "journal.fond_ouverture", libelle: "Fond d'ouverture", format: "montant" },
    { chemin: "journal.entrees", libelle: "Total entrées", format: "montant" },
    { chemin: "journal.sorties", libelle: "Total sorties", format: "montant" },
    { chemin: "journal.solde_theorique", libelle: "Solde théorique", format: "montant" },
    { chemin: "journal.especes_comptees", libelle: "Espèces comptées", format: "montant" },
    { chemin: "journal.ecart", libelle: "Écart", format: "montant" },
    { chemin: "journal.ouvert_par", libelle: "Ouverte par", format: "texte" },
  ],
};

/** Le nom de la liste qu'un tableau doit parcourir, selon le genre. */
export const SOURCE_PAR_GENRE: Record<GenreDocument, string> = {
  facture: "lignes",
  bon_sortie: "lignes",
  recu_paiement: "lignes",
  releve_creance: "factures",
  journal_caisse: "mouvements",
};
