// lib/documents.ts — les réglages des documents imprimés (v3, A-1).
//
// Le miroir TypeScript de `coeur::documents` : genres, formats, et ce
// que l'écran lit une fois avant d'imprimer. Les règles vivent en Rust ;
// ici on ne fait que nommer et charger.

import { appeler as invoke } from "@/lib/pont";

export type Genre =
  | "facture" | "devis" | "bon_commande" | "bon_livraison"
  | "recu" | "releve" | "ticket" | "bulletin";

export type Choix = "auto" | "oui" | "non";

export type FormatDocument = "a4" | "a5" | "thermique_80" | "thermique_58";

export interface SignatureDoc {
  libelle: string;
  /** data URL du cachet ou de la signature scannée, ou null. */
  image?: string | null;
}

export interface ReglageGenre {
  format: FormatDocument;
  colonne_remise: Choix;
  colonne_tva: Choix;
  recap_tva: Choix;
  montant_lettres: boolean;
  reference_article: boolean;
  /** null : le « pied de facture » de la société. */
  mention: string | null;
  signatures: SignatureDoc[];
  /** Le filigrane de la société sur ce genre (25/09) ; absent = oui. */
  filigrane?: boolean;
}

/**
 * Le filigrane (25/09) : une image ou un mot au milieu de chaque page,
 * derrière le texte, estompé et flouté pour ne pas se mêler aux
 * tableaux. Réglé une fois pour tous les documents.
 */
export interface FiligraneDoc {
  /** data URL ; l'emporte sur le texte. */
  image?: string | null;
  texte?: string | null;
  /** Pour cent, 3 à 30. */
  opacite: number;
  /** Pixels, 0 à 6. */
  flou: number;
  /** Pour cent de la largeur de la page, 20 à 100. */
  taille: number;
}

export const FILIGRANE_BORNES = {
  opacite: [3, 30], flou: [0, 6], taille: [20, 100],
} as const;

export type Coordonnee =
  | "adresse" | "telephone" | "telephone2" | "email" | "site_web" | "nif" | "rccm";

export interface ReglagesDocuments {
  ordre: Genre[];
  genres: Record<Genre, ReglageGenre>;
  coordonnees: Coordonnee[];
  filigrane?: FiligraneDoc | null;
}

export const LIBELLES_GENRE: Record<Genre, string> = {
  facture: "Facture",
  devis: "Devis",
  bon_commande: "Bon de commande",
  bon_livraison: "Bon de livraison",
  recu: "Reçu de paiement",
  releve: "Relevé",
  ticket: "Ticket de caisse",
  bulletin: "Bulletin de paie",
};

/** Ce que couvre chaque genre, dit au commerçant. */
export const PORTEE_GENRE: Record<Genre, string> = {
  facture: "Factures, factures d'acompte, avoirs — client et fournisseur.",
  devis: "Devis et factures proforma.",
  bon_commande: "Commandes client et bons de commande fournisseur.",
  bon_livraison: "Bons de livraison, de réception, de sortie et d'échange.",
  recu: "Le reçu remis à chaque règlement.",
  releve: "Relevés de créance et historique des règlements.",
  ticket: "Le ticket du point de vente, sur rouleau.",
  bulletin: "Le bulletin remis avec la paie (Gescom Équipe).",
};

export const LIBELLES_FORMAT: Record<FormatDocument, string> = {
  a4: "A4",
  a5: "A5",
  thermique_80: "Ticket 80 mm",
  thermique_58: "Ticket 58 mm",
};

export const LIBELLES_COORDONNEE: Record<Coordonnee, string> = {
  adresse: "Adresse",
  telephone: "Téléphone",
  telephone2: "Téléphone 2",
  email: "Email",
  site_web: "Site web",
  nif: "NIF",
  rccm: "RCCM",
};

export const SIGNATURES_MAX = 3;

/** Le genre d'un type de pièce — même table que `coeur::genre_de_piece`. */
export function genreDePiece(typePiece: string): Genre {
  switch (typePiece) {
    case "devis": case "proforma": return "devis";
    case "commande_client": case "bon_commande_fournisseur": return "bon_commande";
    case "bon_livraison": case "bon_reception": return "bon_livraison";
    default: return "facture";
  }
}

export function lireReglagesDocuments(): Promise<ReglagesDocuments> {
  return invoke<ReglagesDocuments>("lire_reglages_documents");
}

/**
 * Choisir une image sur ce poste, sans passer par une boîte de dialogue
 * Tauri : un `<input type="file">` marche dans la fenêtre comme dans un
 * navigateur. Rend le nom et la data URL, ou null si on annule.
 */
export function choisirImage(
  accepte = "image/png,image/jpeg,image/webp",
): Promise<{ nom: string; dataUrl: string } | null> {
  return new Promise((resoudre, rejeter) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accepte;
    input.style.display = "none";
    input.onchange = () => {
      const f = input.files?.[0];
      input.remove();
      if (!f) return resoudre(null);
      const lecteur = new FileReader();
      lecteur.onload = () => resoudre({ nom: f.name, dataUrl: String(lecteur.result) });
      lecteur.onerror = () => rejeter("Lecture du fichier impossible.");
      lecteur.readAsDataURL(f);
    };
    // Annulation : certains moteurs ne déclenchent rien. Rien à nettoyer
    // de plus qu'un input caché.
    document.body.appendChild(input);
    input.click();
  });
}

/** Le contenu base64 seul, sans l'entête `data:…;base64,`. */
export function contenuBase64(dataUrl: string): string {
  const i = dataUrl.indexOf("base64,");
  return i >= 0 ? dataUrl.slice(i + 7) : dataUrl;
}
