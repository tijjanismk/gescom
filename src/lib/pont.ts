// lib/pont.ts — le seul endroit qui sait OÙ tourne le code métier.
//
// ## Pourquoi ce fichier existe
//
// Le v1 appelait `invoke("creer_vente", …)` depuis 40 fichiers. Le pont
// Tauri résolvait ce nom vers une fonction Rust du même processus. En
// v2, le même appel doit pouvoir partir sur le réseau vers un serveur —
// sans que les 325 appels changent d'une ligne.
//
// D'où la forme retenue : `appeler` a exactement la signature de
// `invoke`, et les écrans l'importent sous ce nom. Un poste caisse et
// un monoposte exécutent donc le même code d'interface. Deux chemins
// d'appel distincts auraient signifié deux comportements à tester, et
// c'est toujours celui qu'on teste le moins qui casse en clientèle.
//
// ## Ce que le pont ne fait pas
//
// Il ne met rien en cache et ne rejoue rien. Un appel qui échoue
// échoue — le commerçant le voit et recommence. Une file d'attente
// hors-ligne rejouerait des ventes dans un ordre que le stock ne
// suivrait plus ; c'est le genre de confort qui produit des chiffres
// faux sans jamais afficher d'erreur.

import { invoke as invokeTauri } from "@tauri-apps/api/core";

// Doit rester égal à `VERSION_PROTOCOLE` dans src-tauri/noyau/src/lib.rs.
export const VERSION_PROTOCOLE = 1;

const CLE_RESEAU = "gescom_reseau";

export type ModeReseau = "monoposte" | "poste";

export interface EtatReseau {
  mode: ModeReseau;
  /** « 192.168.1.10:7300 », sans schéma. */
  serveur: string;
  jeton: string | null;
  posteId: string | null;
  utilisateurId: string | null;
  caisseParUtilisateur: boolean;
}

const DEFAUT: EtatReseau = {
  mode: "monoposte",
  serveur: "",
  jeton: null,
  posteId: null,
  utilisateurId: null,
  caisseParUtilisateur: false,
};

let etat: EtatReseau = charger();

function charger(): EtatReseau {
  try {
    const brut = localStorage.getItem(CLE_RESEAU);
    if (!brut) return { ...DEFAUT };
    return { ...DEFAUT, ...JSON.parse(brut) };
  } catch {
    // localStorage inaccessible (fenêtre privée, stockage bloqué) :
    // le monoposte reste utilisable, c'est le comportement du v1.
    return { ...DEFAUT };
  }
}

function enregistrer() {
  try {
    localStorage.setItem(CLE_RESEAU, JSON.stringify(etat));
  } catch {
    /* voir charger() */
  }
}

export function etatReseau(): EtatReseau {
  return { ...etat };
}

export function enReseau(): boolean {
  return etat.mode === "poste";
}

export function definirServeur(mode: ModeReseau, serveur: string) {
  const neuf = serveur.trim();
  // Changer de serveur invalide le jeton : il a été émis par l'autre.
  // Le comparer AVANT d'écrire, sinon on compare la nouvelle valeur à
  // elle-même et le jeton périmé survit au changement.
  const change = mode === "monoposte" || neuf !== etat.serveur;
  etat = { ...etat, mode, serveur: neuf };
  if (change) {
    etat.jeton = null;
    etat.posteId = null;
    etat.utilisateurId = null;
  }
  enregistrer();
}

function racine(): string {
  const a = etat.serveur.replace(/^https?:\/\//, "").replace(/\/+$/, "");
  return `http://${a.includes(":") ? a : `${a}:7300`}`;
}

// =====================================================================
//  L'appel de commande
// =====================================================================

/**
 * Appelle une commande métier, ici ou sur le serveur.
 *
 * Rejette avec une **chaîne**, jamais avec un `Error` : c'est ce que
 * fait `invoke` de Tauri, et les 40 écrans affichent déjà `String(e)`.
 * Un `Error` y apparaîtrait préfixé de « Error: » sur chaque message
 * métier montré au commerçant.
 */
export async function appeler<T = unknown>(
  commande: string,
  params?: Record<string, unknown>,
): Promise<T> {
  if (!enReseau()) {
    return invokeTauri<T>(commande, params);
  }

  let reponse: Response;
  try {
    reponse = await fetch(`${racine()}/rpc`, {
      method: "POST",
      headers: entetes(),
      body: JSON.stringify({ commande, params: params ?? {} }),
    });
  } catch {
    throw `Serveur injoignable (${etat.serveur}). Vérifier le réseau ` +
      `et que Gescom Serveur est démarré sur le poste principal.`;
  }

  const corps = await reponse.json().catch(() => null);

  if (reponse.status === 401) {
    // La session est tombée : le jeton local ne vaut plus rien, et le
    // garder ferait échouer chaque appel suivant de la même façon sans
    // que l'écran de connexion ne reparaisse jamais.
    oublierSession();
    throw corps?.message ?? "Session expirée — se reconnecter.";
  }
  if (corps && corps.etat === "erreur") throw corps.message;
  if (corps && corps.etat === "ok") return corps.donnee as T;
  if (!reponse.ok) throw `Erreur serveur (${reponse.status}).`;
  return corps as T;
}

function entetes(): Record<string, string> {
  const h: Record<string, string> = { "Content-Type": "application/json" };
  if (etat.jeton) h["Authorization"] = `Bearer ${etat.jeton}`;
  return h;
}

// =====================================================================
//  Connexion au serveur
// =====================================================================

export interface Identite {
  jeton: string;
  utilisateur_id: string;
  utilisateur_nom: string;
  role: string;
  doit_changer_mdp: boolean;
  poste_id: string;
  expire_le: string;
  caisse_par_utilisateur: boolean;
}

export async function connecterServeur(
  identifiant: string,
  motDePasse: string,
  posteNom: string,
  posteEmpreinte: string,
): Promise<Identite> {
  let reponse: Response;
  try {
    reponse = await fetch(`${racine()}/connexion`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        identifiant,
        mot_de_passe: motDePasse,
        poste_nom: posteNom,
        poste_empreinte: posteEmpreinte,
        version_protocole: VERSION_PROTOCOLE,
      }),
    });
  } catch {
    throw `Serveur injoignable (${etat.serveur}).`;
  }

  const corps = await reponse.json().catch(() => null);
  if (!reponse.ok) throw corps?.message ?? `Connexion refusée (${reponse.status}).`;

  const identite = corps as Identite;
  etat = {
    ...etat,
    jeton: identite.jeton,
    posteId: identite.poste_id,
    utilisateurId: identite.utilisateur_id,
    caisseParUtilisateur: identite.caisse_par_utilisateur,
  };
  enregistrer();
  return identite;
}

export async function deconnecterServeur() {
  if (etat.jeton) {
    await fetch(`${racine()}/deconnexion`, {
      method: "POST",
      headers: entetes(),
    }).catch(() => undefined);
  }
  oublierSession();
}

function oublierSession() {
  etat = { ...etat, jeton: null, posteId: null, utilisateurId: null };
  enregistrer();
}

export async function sante(adresse?: string): Promise<unknown> {
  const base = adresse
    ? `http://${adresse.replace(/^https?:\/\//, "").replace(/\/+$/, "")}`
    : racine();
  const r = await fetch(`${base}/sante`);
  if (!r.ok) throw `Le serveur répond ${r.status}.`;
  return r.json();
}

// =====================================================================
//  Canal — ce que les autres postes viennent de faire
// =====================================================================

export interface Evenement {
  seq: number;
  genre: string;
  entite: string;
  entite_id?: string;
  poste_id: string;
  horodatage: string;
}

/**
 * Écoute les changements des autres postes.
 *
 * Longue attente : le serveur garde la requête ouverte jusqu'à trente
 * secondes s'il n'a rien à dire. Interroger toutes les deux secondes
 * donnerait le même résultat avec cinquante fois plus de requêtes, et
 * un écran qui clignote pendant une saisie.
 *
 * Retourne la fonction qui arrête l'écoute — à appeler au démontage,
 * sinon chaque changement de page laisse une boucle de plus derrière
 * elle.
 */
export function ecouterCanal(surEvenements: (e: Evenement[]) => void): () => void {
  if (!enReseau()) return () => undefined;

  let vivant = true;
  let depuis = -1;

  (async () => {
    while (vivant) {
      try {
        const url = depuis >= 0 ? `${racine()}/canal?depuis=${depuis}` : `${racine()}/canal`;
        const r = await fetch(url, { headers: entetes() });
        if (!vivant) return;
        if (r.status === 401) {
          oublierSession();
          return;
        }
        const lot = await r.json();
        depuis = lot.seq_max ?? depuis;
        if (lot.evenements?.length) surEvenements(lot.evenements);
      } catch {
        // Réseau coupé : on patiente avant de réessayer, sinon la
        // boucle sature le processeur du poste tant que le câble est
        // débranché.
        await new Promise((r) => setTimeout(r, 5000));
      }
    }
  })();

  return () => {
    vivant = false;
  };
}
