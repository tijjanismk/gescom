// lib/modeles/defauts.ts — les modèles livrés avec Gescom.
//
// Ils ne sont pas un exemple : ce sont ceux qui impriment tant que
// personne n'a rien modifié. Ils doivent donc être corrects tout de
// suite, sur une quincaillerie de Bamako — mentions légales, NIF, RCCM,
// TVA à 18 %, montant en lettres pour les pièces qui circulent.
//
// Ils vivent dans le code et non en base, pour une raison : c'est ce
// qui rend « Réinitialiser » possible. Après une mise en page ratée un
// soir de clôture, il faut un chemin de retour qui ne dépende pas
// d'une sauvegarde que personne n'a faite.

import type {
  Bloc,
  Champ,
  Colonne,
  ContenuModele,
  FormatValeur,
  Modele,
} from "./types";

let compteur = 0;
function id(prefixe: string): string {
  compteur += 1;
  return `${prefixe}-${compteur}`;
}

function champ(
  libelle: string,
  chemin: string,
  format: FormatValeur = "texte",
  masquerSiVide = true,
): Champ {
  return { id: id("c"), libelle, chemin, format, masquerSiVide };
}

function colonne(
  libelle: string,
  chemin: string,
  largeur: number,
  alignement: Colonne["alignement"],
  format: FormatValeur,
): Colonne {
  return { id: id("col"), libelle, chemin, largeur, alignement, format };
}

const PAGE = {
  margeMm: 12,
  taillePt: 10,
  police: "Arial, Helvetica, sans-serif",
  couleurAccent: "#1a1a1a",
};

function contenu(blocs: Bloc[], page = PAGE): ContenuModele {
  return { version: 1, page, blocs };
}

// =====================================================================
//  Facture A4
// =====================================================================

function facturePapier(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "logo",
      hauteurMm: 20,
      afficherSociete: true,
      alignement: "gauche",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "{{piece.type_libelle}} N° {{piece.numero}}",
      alignement: "centre",
      taillePt: 16,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 2,
      items: [
        champ("Date", "piece.date_piece", "date", false),
        champ("Client", "tiers.nom", "texte", false),
        champ("Échéance", "piece.date_echeance", "date"),
        champ("Code", "tiers.code"),
        champ("Établi par", "piece.auteur_nom"),
        champ("Téléphone", "tiers.telephone"),
        champ("", "", "texte"),
        champ("NIF", "tiers.nif"),
      ],
    },
    {
      id: id("b"),
      type: "tableau",
      visible: true,
      source: "lignes",
      zebre: true,
      siVide: "Aucune ligne sur cette pièce.",
      colonnes: [
        colonne("Désignation", "article_nom", 40, "gauche", "texte"),
        colonne("Unité", "unite_libelle", 12, "gauche", "texte"),
        colonne("Qté", "quantite", 10, "droite", "nombre"),
        colonne("P.U.", "prix_unitaire", 17, "droite", "montant"),
        colonne("Montant", "montant_ttc", 21, "droite", "montant"),
      ],
    },
    {
      id: id("b"),
      type: "totaux",
      visible: true,
      accentuerDernier: true,
      items: [
        champ("Total HT", "totaux.total_ht", "montant", false),
        champ("Remise", "totaux.remise", "montant"),
        champ("TVA", "totaux.total_tva", "montant"),
        champ("Déjà payé", "totaux.total_paye", "montant"),
        champ("NET À PAYER", "totaux.total_ttc", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu: "Arrêtée la présente facture à la somme de {{totaux.total_en_lettres}}.",
      alignement: "gauche",
      taillePt: 9.5,
      italique: true,
      cadre: true,
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu: "{{piece.note}}",
      alignement: "gauche",
      taillePt: 9,
      italique: false,
      cadre: false,
    },
    {
      id: id("b"),
      type: "signatures",
      visible: true,
      gauche: "Le client",
      droite: "Pour l'entreprise",
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu:
        "{{societe.nom}} — NIF {{societe.nif}} — RCCM {{societe.rccm}}\n"
        + "Marchandise vendue ne peut être ni reprise ni échangée sans ce document.",
      alignement: "centre",
      taillePt: 8,
      italique: true,
      cadre: false,
    },
  ];
}

// =====================================================================
//  Bon de sortie — le même document, sans un seul montant
// =====================================================================

function bonSortie(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "logo",
      hauteurMm: 16,
      afficherSociete: true,
      alignement: "gauche",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "BON DE SORTIE — {{piece.numero}}",
      alignement: "centre",
      taillePt: 15,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 2,
      items: [
        champ("Date", "piece.date_piece", "date", false),
        champ("Client", "tiers.nom", "texte", false),
        champ("Téléphone", "tiers.telephone"),
        champ("Code", "tiers.code"),
      ],
    },
    {
      id: id("b"),
      type: "tableau",
      visible: true,
      source: "lignes",
      zebre: false,
      siVide: "Aucun article à délivrer.",
      // Aucune colonne de prix : le magasinier n'a pas à connaître les
      // montants, et le client n'a pas à les voir deux fois. C'est tout
      // l'objet de ce document.
      colonnes: [
        colonne("Désignation", "article_nom", 62, "gauche", "texte"),
        colonne("Unité", "unite_libelle", 20, "gauche", "texte"),
        colonne("Quantité", "quantite", 18, "droite", "nombre"),
      ],
    },
    {
      id: id("b"),
      type: "espace",
      visible: true,
      hauteurMm: 6,
    },
    {
      id: id("b"),
      type: "signatures",
      visible: true,
      gauche: "Le magasinier",
      droite: "Le client (reçu la marchandise)",
    },
  ];
}

// =====================================================================
//  Reçu de règlement
// =====================================================================

function recu(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "logo",
      hauteurMm: 15,
      afficherSociete: true,
      alignement: "centre",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "{{piece.type_libelle}}",
      alignement: "centre",
      taillePt: 15,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 1,
      items: [
        champ("Reçu de", "tiers.nom", "texte", false),
        champ("Téléphone", "tiers.telephone"),
        champ("Date", "paiement.date", "date_heure", false),
        champ("Mode de règlement", "paiement.mode", "texte", false),
        champ("Imputé sur", "paiement.reference"),
        champ("Encaissé par", "paiement.encaisse_par"),
      ],
    },
    {
      id: id("b"),
      type: "totaux",
      visible: true,
      accentuerDernier: true,
      items: [
        champ("Reste dû après ce règlement", "totaux.reste_du", "montant"),
        champ("MONTANT REÇU", "paiement.montant", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu: "Reçu la somme de {{totaux.total_en_lettres}}.",
      alignement: "centre",
      taillePt: 10,
      italique: true,
      cadre: true,
    },
    {
      id: id("b"),
      type: "signatures",
      visible: true,
      gauche: "",
      droite: "Le caissier",
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      // La mention compte : un reçu produit devant l'administration à
      // la place d'une facture n'a pas la même valeur, et le client
      // doit le savoir en le recevant.
      contenu: "Ce reçu atteste du règlement ci-dessus. Il ne vaut pas facture.",
      alignement: "centre",
      taillePt: 8,
      italique: true,
      cadre: false,
    },
  ];
}

// =====================================================================
//  État de créance
// =====================================================================

function releve(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "logo",
      hauteurMm: 18,
      afficherSociete: true,
      alignement: "gauche",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "ÉTAT DE CRÉANCE",
      alignement: "centre",
      taillePt: 16,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 2,
      items: [
        champ("Client", "tiers.nom", "texte", false),
        champ("Édité le", "piece.date_piece", "date", false),
        champ("Code", "tiers.code"),
        champ("Période", "releve.periode"),
        champ("Téléphone", "tiers.telephone"),
        champ("Factures non soldées", "releve.nb_factures", "nombre"),
      ],
    },
    {
      id: id("b"),
      type: "tableau",
      visible: true,
      source: "factures",
      zebre: true,
      siVide: "Aucune facture non soldée. Le compte est à jour.",
      colonnes: [
        colonne("Date", "date", 16, "gauche", "date"),
        colonne("Pièce", "numero", 26, "gauche", "texte"),
        colonne("Total", "total", 20, "droite", "montant"),
        colonne("Réglé", "paye", 18, "droite", "montant"),
        colonne("Reste", "reste", 20, "droite", "montant"),
      ],
    },
    {
      id: id("b"),
      type: "totaux",
      visible: true,
      accentuerDernier: true,
      items: [
        champ("Total facturé", "totaux.total_ttc", "montant", false),
        champ("Avoirs à déduire", "totaux.remise", "montant"),
        champ("TOTAL DÛ", "totaux.reste_du", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu:
        "Document récapitulatif des factures non soldées à ce jour. "
        + "Les règlements postérieurs à la date d'édition n'y figurent pas.",
      alignement: "gauche",
      taillePt: 8.5,
      italique: true,
      cadre: false,
    },
    {
      id: id("b"),
      type: "signatures",
      visible: true,
      gauche: "Le client",
      droite: "Pour l'entreprise",
    },
  ];
}

// =====================================================================
//  Journal de caisse
// =====================================================================

function journal(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "aucune",
      hauteurMm: 0,
      afficherSociete: true,
      alignement: "gauche",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "JOURNAL DE CAISSE — {{journal.date}}",
      alignement: "centre",
      taillePt: 15,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 2,
      items: [
        champ("Ouverte par", "journal.ouvert_par", "texte", false),
        champ("Fond d'ouverture", "journal.fond_ouverture", "montant", false),
        champ("Total entrées", "journal.entrees", "montant", false),
        champ("Total sorties", "journal.sorties", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "tableau",
      visible: true,
      source: "mouvements",
      zebre: true,
      siVide: "Aucun mouvement de caisse sur la journée.",
      colonnes: [
        colonne("Heure", "heure", 12, "gauche", "texte"),
        colonne("Motif", "motif", 18, "gauche", "texte"),
        colonne("Libellé", "libelle", 30, "gauche", "texte"),
        colonne("Moyen", "moyen", 14, "gauche", "texte"),
        colonne("Entrée", "entree", 13, "droite", "montant"),
        colonne("Sortie", "sortie", 13, "droite", "montant"),
      ],
    },
    {
      id: id("b"),
      type: "totaux",
      visible: true,
      accentuerDernier: true,
      items: [
        champ("Solde théorique", "journal.solde_theorique", "montant", false),
        champ("Espèces comptées", "journal.especes_comptees", "montant"),
        // L'écart est le chiffre qu'on cherche. Il reste visible même à
        // zéro : une clôture juste doit se voir aussi, sinon on ne sait
        // pas si le contrôle a été fait.
        champ("ÉCART", "journal.ecart", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "signatures",
      visible: true,
      gauche: "Le caissier",
      droite: "Le responsable",
    },
  ];
}

// =====================================================================
//  Ticket thermique 80 mm
// =====================================================================

function ticket(): Bloc[] {
  return [
    {
      id: id("b"),
      type: "entete",
      visible: true,
      image: "logo",
      hauteurMm: 12,
      afficherSociete: true,
      alignement: "centre",
    },
    {
      id: id("b"),
      type: "titre",
      visible: true,
      texte: "{{piece.numero}}",
      alignement: "centre",
      taillePt: 11,
      trait: true,
    },
    {
      id: id("b"),
      type: "champs",
      visible: true,
      titre: "",
      colonnes: 1,
      items: [
        champ("Date", "piece.date_piece", "date_heure", false),
        champ("Client", "tiers.nom", "texte", false),
        champ("Caissier", "piece.auteur_nom"),
      ],
    },
    {
      id: id("b"),
      type: "tableau",
      visible: true,
      source: "lignes",
      zebre: false,
      siVide: "",
      // Quatre colonnes au plus sur 80 mm : au-delà, la désignation se
      // réduit à trois mots et le ticket devient illisible.
      colonnes: [
        colonne("Article", "article_nom", 46, "gauche", "texte"),
        colonne("Qté", "quantite", 14, "droite", "nombre"),
        colonne("Montant", "montant_ttc", 40, "droite", "montant"),
      ],
    },
    {
      id: id("b"),
      type: "totaux",
      visible: true,
      accentuerDernier: true,
      items: [
        champ("Déjà payé", "totaux.total_paye", "montant"),
        champ("TOTAL", "totaux.total_ttc", "montant", false),
      ],
    },
    {
      id: id("b"),
      type: "texte",
      visible: true,
      contenu: "Merci de votre confiance.\n{{societe.telephone}}",
      alignement: "centre",
      taillePt: 8.5,
      italique: false,
      cadre: false,
    },
  ];
}

// =====================================================================
//  Le catalogue
// =====================================================================

/** Identifiants stables : ce sont eux que « Réinitialiser » retrouve. */
export const ID_FACTURE_A4 = "std-facture-a4";
export const ID_FACTURE_T80 = "std-facture-t80";
export const ID_BON_SORTIE = "std-bon-sortie";
export const ID_RECU = "std-recu-paiement";
export const ID_RELEVE = "std-releve-creance";
export const ID_JOURNAL = "std-journal-caisse";

export function modelesParDefaut(): Modele[] {
  // Le compteur repart à zéro : les identifiants de blocs doivent être
  // les mêmes d'un appel à l'autre, sinon « Réinitialiser » produit un
  // modèle différent à chaque fois et l'écran croit à une modification.
  compteur = 0;

  const commun = { est_defaut: true, actif: false };
  return [
    {
      ...commun,
      id: ID_FACTURE_A4,
      genre: "facture",
      nom: "Facture A4 — standard",
      format: "a4",
      contenu: contenu(facturePapier()),
    },
    {
      ...commun,
      id: ID_FACTURE_T80,
      genre: "facture",
      nom: "Ticket 80 mm",
      format: "thermique_80",
      contenu: contenu(ticket(), { ...PAGE, margeMm: 3, taillePt: 8.5 }),
    },
    {
      ...commun,
      id: ID_BON_SORTIE,
      genre: "bon_sortie",
      nom: "Bon de sortie A4",
      format: "a4",
      contenu: contenu(bonSortie()),
    },
    {
      ...commun,
      id: ID_RECU,
      genre: "recu_paiement",
      nom: "Reçu A5",
      format: "a5",
      contenu: contenu(recu(), { ...PAGE, margeMm: 10 }),
    },
    {
      ...commun,
      id: ID_RELEVE,
      genre: "releve_creance",
      nom: "État de créance A4",
      format: "a4",
      contenu: contenu(releve()),
    },
    {
      ...commun,
      id: ID_JOURNAL,
      genre: "journal_caisse",
      nom: "Journal de caisse A4",
      format: "a4",
      contenu: contenu(journal()),
    },
  ];
}

/** Le modèle d'usine d'un identifiant donné, ou `null`. */
export function modeleDUsine(id: string): Modele | null {
  return modelesParDefaut().find((m) => m.id === id) ?? null;
}

/** Les genres qui doivent être actifs par défaut, dans l'ordre. */
export const ACTIFS_PAR_DEFAUT = [
  ID_FACTURE_A4,
  ID_BON_SORTIE,
  ID_RECU,
  ID_RELEVE,
  ID_JOURNAL,
];
