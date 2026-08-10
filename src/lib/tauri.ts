import { invoke } from "@tauri-apps/api/core";

// =====================================================================
//  Types — miroir des structs Rust
// =====================================================================

export interface Client {
  id: string;
  code: string;
  nom: string;
  telephone?: string;
}

export interface UniteVente {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

export interface Article {
  id: string;
  nom: string;
  unite_base: string;
  unites: UniteVente[];
}

export interface Depot {
  id: string;
  nom: string;
}

export interface ResultatVente {
  vente_id: string;
}

export interface ParamsLigne {
  article_id: string;
  unite_vente_id: string;
  depot_source_id: string;
  source_approvisionnement: "stock" | "fournisseur_secondaire";
  quantite: number;
  facteur: number;
  prix_reference: number;
  prix_pratique: number;
}

// =====================================================================
//  Utilisateur courant — hardcodé pour l'instant
// =====================================================================

export const UTILISATEUR_ID   = "user-patron";
export const UTILISATEUR_ROLE = "patron";

// =====================================================================
//  Lecture
// =====================================================================

export async function lireClients(): Promise<Client[]> {
  return await invoke("lire_clients");
}

export async function lireClientGenerique(): Promise<Client> {
  return await invoke("lire_client_generique");
}

export async function lireDepotDefaut(): Promise<Depot> {
  return await invoke("lire_depot_defaut");
}

export async function lireArticlesAvecUnites(): Promise<Article[]> {
  return await invoke("lire_articles_avec_unites");
}

// =====================================================================
//  Création rapide
// =====================================================================

export async function creerClientRapide(
  nom: string,
  telephone?: string,
): Promise<Client> {
  return await invoke("creer_client_rapide", {
    nom,
    telephone: telephone ?? null,
    utilisateurId: UTILISATEUR_ID,
  });
}

export async function creerArticleRapide(
  nom: string,
  uniteBase: string,
  prixReference: number,
): Promise<Article> {
  return await invoke("creer_article_rapide", {
    nom,
    uniteBase,
    prixReference,
    utilisateurId: UTILISATEUR_ID,
  });
}

// =====================================================================
//  Ventes
// =====================================================================

export async function creerVente(
  clientId: string,
  depotId: string,
  modeReglement: "comptant" | "credit",
  lignes: ParamsLigne[],
): Promise<ResultatVente> {
  return await invoke("creer_vente", {
    clientId,
    depotId,
    modeReglement,
    lignes,
    utilisateurId:   UTILISATEUR_ID,
    utilisateurRole: UTILISATEUR_ROLE,
  });
}

export async function enregistrerPaiement(
  venteId: string,
  montant: number,
  mode: string,
  avoirId?: string,
): Promise<string> {
  return await invoke("enregistrer_paiement", {
    venteId,
    montant,
    mode,
    avoirId: avoirId ?? null,
    utilisateurId:   UTILISATEUR_ID,
    utilisateurRole: UTILISATEUR_ROLE,
  });
}