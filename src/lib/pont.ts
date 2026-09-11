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

/**
 * Sommes-nous dans la coque Tauri, ou dans un navigateur nu ?
 *
 * Dans un navigateur — `npm run dev` ouvert dans Chrome — il n'y a NI
 * base locale NI commandes Rust : seul le serveur peut répondre. Le
 * mode monoposte n'y a donc aucun sens, et laisser `appeler` tomber sur
 * `invokeTauri` produirait sur chaque écran une erreur qui ne dit pas
 * ce qui manque.
 *
 * Cela sert au développement, et à une chose de plus : **vérifier le
 * chemin réseau sans seconde machine**. Le navigateur joue la caisse,
 * le serveur tourne à côté, et les outils de développement montrent
 * chaque requête.
 */
function estDansTauri(): boolean {
  return typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

/**
 * L'adresse du serveur quand on est dans un navigateur.
 *
 * Par ordre : `?serveur=…` dans l'URL, puis ce que le navigateur avait
 * retenu, puis le poste local. Le paramètre d'URL passe devant parce
 * que c'est lui qu'on change pendant un essai.
 */
function serveurDeMiseAuPoint(defaut: string): string {
  try {
    const p = new URLSearchParams(window.location.search).get("serveur");
    if (p && p.trim()) return p.trim();
  } catch {
    // Pas de `location` : contexte de test.
  }
  return defaut && defaut.trim() ? defaut : "127.0.0.1:7300";
}

export type ModeReseau = "monoposte" | "poste";

export interface EtatReseau {
  mode: ModeReseau;
  /** « 192.168.1.10:7300 », sans schéma. */
  serveur: string;
  /** Nom lisible de ce poste, tel que le serveur l'affichera. */
  posteNom: string;
  /** Empreinte stable, tirée une fois côté Rust. */
  posteEmpreinte: string;
  jeton: string | null;
  posteId: string | null;
  utilisateurId: string | null;
  caisseParUtilisateur: boolean;
}

const DEFAUT: EtatReseau = {
  mode: "monoposte",
  serveur: "",
  posteNom: "",
  posteEmpreinte: "",
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

/**
 * Aligne le pont sur ce que Rust sait.
 *
 * Le mode et l'adresse vivent dans `poste.json`, à côté des données —
 * pas dans le navigateur. Le `localStorage` n'en est qu'un miroir, et
 * il s'efface : cache vidé, profil WebView2 recréé, et un poste caisse
 * se croirait monoposte. Il ouvrirait alors sa base locale, vide, et le
 * commerçant conclurait que ses données ont disparu.
 *
 * Rust est donc la source, le navigateur la copie. À appeler au
 * démarrage, avant le premier appel de commande.
 */
export async function synchroniserConfig(): Promise<EtatReseau> {
  // Navigateur nu : il n'y a pas de `poste.json` a lire, et pas de base
  // locale a ouvrir. On force le mode reseau, sinon chaque ecran
  // echouerait sur un `invoke` qui n'existe pas.
  if (!estDansTauri()) {
    const serveur = serveurDeMiseAuPoint(etat.serveur);
    const change = etat.mode !== "poste" || serveur !== etat.serveur;
    etat = {
      ...etat,
      mode: "poste",
      serveur,
      posteNom: etat.posteNom || "Navigateur (mise au point)",
      posteEmpreinte: etat.posteEmpreinte || "navigateur-dev",
      ...(change ? { jeton: null, posteId: null, utilisateurId: null } : {}),
    };
    enregistrer();
    console.info(
      `[pont] Hors Tauri : mode poste sur ${serveur}. ` +
      "Changer avec ?serveur=adresse:port",
    );
    return { ...etat };
  }

  try {
    const c = await invokeTauri<{
      mode: string;
      serveur: string;
      poste_nom: string;
      poste_empreinte: string;
    }>("lire_config_reseau");

    const mode: ModeReseau = c.mode === "poste" ? "poste" : "monoposte";
    // Le serveur a changé depuis la dernière fois : le jeton qu'on
    // garde a été émis par l'autre, il ne vaut plus rien.
    const change = mode !== etat.mode || c.serveur !== etat.serveur;
    etat = {
      ...etat,
      mode,
      serveur: c.serveur,
      posteNom: c.poste_nom,
      posteEmpreinte: c.poste_empreinte,
      ...(change ? { jeton: null, posteId: null, utilisateurId: null } : {}),
    };
    enregistrer();
  } catch (e) {
    // Pas de pont Tauri (aperçu, test) : on reste sur ce que le
    // navigateur avait. Échouer ici empêcherait l'application de
    // s'ouvrir pour un réglage qui, en monoposte, ne sert à rien.
    console.error("Configuration réseau :", e);
  }
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
 * Les commandes qui s'exécutent TOUJOURS sur cette machine.
 *
 * Une caisse en réseau envoie tout au serveur — sauf ce qui n'a de sens
 * que devant l'utilisateur :
 *
 * - **imprimer** : c'est la caisse qui a l'imprimante. Le serveur est
 *   un service sans écran ; lui demander d'imprimer ne produirait rien,
 *   ou du papier sur le poste du patron ;
 * - **ouvrir un fichier** avec l'application du système ;
 * - **le réglage réseau lui-même** : il vit dans le `poste.json` de
 *   cette machine. Le demander au serveur serait circulaire — et
 *   `tester_serveur` doit justement pouvoir échouer sans réseau ;
 * - **import/export de modèles** : ils passent par un chemin de fichier
 *   local, qui ne désigne rien chez le serveur.
 *
 * Cette liste est **fermée**. Une commande absente part au serveur :
 * c'est le bon défaut, parce qu'une commande métier oubliée ici
 * écrirait dans la base locale de la caisse — vide — et le commerçant
 * verrait sa saisie disparaître sans message.
 */
const LOCALES = new Set([
  "imprimer_piece",
  "imprimer_facture",
  "ouvrir_avec_systeme",
  "lire_config_reseau",
  "definir_config_reseau",
  "tester_serveur",
  "exporter_modeles",
  "importer_modeles",
]);

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
  if (!enReseau() || LOCALES.has(commande)) {
    // Dans un navigateur, ces commandes-la n'existent pas : elles
    // ouvrent une fenetre, lisent un fichier, parlent au systeme. Le
    // dire franchement vaut mieux qu'une erreur de pont illisible.
    if (LOCALES.has(commande) && !estDansTauri()) {
      throw `« ${commande} » n'est disponible que dans l'application ` +
        "installée, pas dans un navigateur.";
    }
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
  posteNom = etat.posteNom,
  posteEmpreinte = etat.posteEmpreinte,
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
