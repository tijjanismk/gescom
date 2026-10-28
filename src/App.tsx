import { poserDroits, relireDroits } from "@/lib/droits";
import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Layout } from "@/components/Layout";
import { PageLogin, UtilisateurConnecte } from "@/pages/PageLogin";
import { PageServeur } from "@/pages/PageServeur";
import { PageRolePoste, PageServeurLocal, garderLaBase } from "@/pages/PageRolePoste";
import { ModalChangerMdp } from "@/components/ModalChangerMdp";
import { AssistantDossier } from "@/pages/AssistantDossier";
import { Dashboard } from "@/pages/Dashboard";
import { Ventes } from "@/pages/Ventes";
import { Pieces } from "@/pages/Pieces";
import { Achats } from "@/pages/Achats";
import { Stock } from "@/pages/Stock";
import { Clients } from "@/pages/Clients";
import { FicheClient } from "@/pages/FicheClient";
import { Fournisseurs } from "@/pages/Fournisseurs";
import { FicheFournisseur } from "@/pages/FicheFournisseur";
import { Caisse } from "@/pages/Caisse";
import { Parametres } from "@/pages/Parametres";
import {
  synchroniserConfig, ecouterCanal, enReseau,
  surSessionPerdue, sessionUtilisable, serveurConfigure, noterPage, etatReseau,
} from "@/lib/pont";
import { Retours } from "@/pages/Retours";
import { Relances } from "@/pages/Relances";
import { Rapports } from "@/pages/Rapports";
import { Journal } from "@/pages/Journal";
import { Historique, type FiltreHistorique } from "@/pages/Historique";
import { peut } from "@/lib/droits";
import { Transferts } from "@/pages/Transferts";
import { Cheques } from "@/pages/Cheques";

// =====================================================================
//  Session persistante — localStorage + expiration 8h
// =====================================================================

const CLE_SESSION = "gescom_session";
const DUREE_SESSION_MS = 8 * 60 * 60 * 1000;

interface SessionStockee {
  utilisateur: UtilisateurConnecte;
  connecte_le: number;
  page_active: string;
  nav_params?: any;
}

function sauvegarderSession(u: UtilisateurConnecte, page: string, params?: any) {
  const s: SessionStockee = {
    utilisateur: u,
    connecte_le: Date.now(),
    page_active: page,
    nav_params: params ?? null,
  };
  localStorage.setItem(CLE_SESSION, JSON.stringify(s));
}

function lireSession(): SessionStockee | null {
  try {
    const raw = localStorage.getItem(CLE_SESSION);
    if (!raw) return null;
    const s: SessionStockee = JSON.parse(raw);
    if (Date.now() - s.connecte_le > DUREE_SESSION_MS) {
      localStorage.removeItem(CLE_SESSION);
      return null;
    }
    // Une session d'AVANT les permissions n'en porte aucune. Restauree
    // telle quelle, `peut()` repond non a tout : le patron se retrouve
    // devant une application vide — sans menu, sans onglet — alors que
    // ses droits sont intacts en base. Elle a ete ecrite par une
    // version qui ne connaissait pas encore ce champ : on la jette et
    // on redemande le mot de passe, ce qui prend cinq secondes.
    if (!Array.isArray(s.utilisateur?.permissions)) {
      localStorage.removeItem(CLE_SESSION);
      return null;
    }
    return s;
  } catch { return null; }
}

function supprimerSession() {
  localStorage.removeItem(CLE_SESSION);
}

// =====================================================================
//  Contexte global
// =====================================================================

export let UTILISATEUR_ACTIF: UtilisateurConnecte | null = null;

// =====================================================================
//  Dépôt actif — scénario « un poste, plusieurs magasins »
// =====================================================================
//
//  null = vue consolidée, tous les dépôts. C'est le défaut : le patron
//  ouvre l'application et voit l'ensemble de ses points de vente.
//
//  Choisir un dépôt filtre le tableau de bord, le journal et le stock.
//  Les ventes se font toujours sur le dépôt sélectionné dans l'écran
//  Ventes, qui reste maître de son propre choix.

const CLE_DEPOT = "gescom_depot_actif";

export let DEPOT_ACTIF: string | null = (() => {
  try { return localStorage.getItem(CLE_DEPOT) || null; }
  catch { return null; }
})();

export function definirDepotActif(id: string | null) {
  DEPOT_ACTIF = id;
  try {
    if (id) localStorage.setItem(CLE_DEPOT, id);
    else localStorage.removeItem(CLE_DEPOT);
  } catch { /* mode privé : le choix ne survit pas au redémarrage */ }
}

// =====================================================================
//  App
// =====================================================================

function App() {
  const sessionInitiale = lireSession();

  const [utilisateur, setUtilisateur] = useState<UtilisateurConnecte | null>(
    sessionInitiale?.utilisateur ?? null
  );
  const [pageActive, setPageActive] = useState(
    sessionInitiale?.page_active ?? "dashboard"
  );
  const [navParams, setNavParams] = useState<any>(
    sessionInitiale?.nav_params ?? null
  );
  const [modalMdp, setModalMdp] = useState(false);
  // Horodatage du dernier evenement recu des autres postes. Sert de
  // signal aux ecrans qui veulent se rafraichir ; ne declenche aucun
  // rechargement par lui-meme.
  const [derniereActivite, setDerniereActivite] = useState(0);
  // Sert uniquement à forcer le rendu quand le dépôt change ;
  // la valeur de référence reste DEPOT_ACTIF.
  const [depotActif, setDepotActif] = useState<string | null>(DEPOT_ACTIF);

  // Attention : NE PAS remettre cette affectation dans un useEffect.
  //
  // La page ouverte accompagne les erreurs envoyees au serveur (B-2).
  useEffect(() => { noterPage(pageActive); }, [pageActive]);

  // Les effets des ENFANTS s'executent avant ceux du parent. Parametres
  // lisait donc UTILISATEUR_ACTIF encore null au premier rendu et
  // concluait « employe » : seuls 4 onglets apparaissaient, et il
  // fallait rafraichir pour voir les 12.
  //
  // L'affectation doit avoir lieu pendant le rendu, avant que le moindre
  // enfant ne soit monte.
  //
  // La garde compare l'OBJET, pas l'identifiant. Elle comparait
  // `?.id !== utilisateur.id` : une session restauree du navigateur
  // avec une liste de permissions vide n'etait alors JAMAIS remplacee
  // par une identite fraiche, puisque c'est le meme identifiant. Le
  // patron se retrouvait avec les seuls onglets non proteges — cinq —
  // et « aucun reglage », sans qu'aucune reconnexion n'y change rien.
  if (utilisateur && UTILISATEUR_ACTIF !== utilisateur) {
    UTILISATEUR_ACTIF = utilisateur;
    poserDroits(utilisateur);
  }

  useEffect(() => {
    if (utilisateur?.doit_changer_mdp) setModalMdp(true);
  }, []);

  // Le mode reseau vit dans `poste.json`, pas dans le navigateur. On
  // aligne le pont AVANT tout le reste : un poste caisse dont le cache
  // a ete vide se croirait sinon monoposte, ouvrirait sa base locale
  // vide, et le commercant conclurait que ses donnees ont disparu.
  const [pontPret, setPontPret] = useState(false);
  // `serveurConfigure()` lit l'etat d'un module, que React ne surveille
  // pas. Ce compteur force le rendu quand l'adresse vient d'etre
  // enregistree — sinon l'ecran de branchement resterait affiche.
  const [, setRevisionReseau] = useState(0);
  useEffect(() => {
    synchroniserConfig().finally(() => setPontPret(true));
  }, []);
  // Le serveur de cette machine a repondu (poste qui garde la base).
  const [serveurLocalPret, setServeurLocalPret] = useState(false);
  // On a dit « caisse d'un autre poste » : l'ecran de branchement.
  const [brancherAilleurs, setBrancherAilleurs] = useState(false);
  const garderBase = async () => {
    await garderLaBase();
    setBrancherAilleurs(false);
    setServeurLocalPret(false);
    setRevisionReseau((n) => n + 1);
  };
  // Le serveur est-il joint ? Avant, rien ne doit l'appeler : pendant
  // que la fenetre lance celui de cette machine, chaque appel echouait
  // en « Serveur injoignable » dans la console, pour rien.
  const modePont = etatReseau().mode;
  const serveurJoint = pontPret && !brancherAilleurs
    && (modePont === "poste" || (modePont === "monoposte" && serveurLocalPret));

  // Le jeton du serveur est la SOURCE.
  //
  // L'application gardait sa propre memoire de « connecte »
  // (localStorage, 8 h), independante du jeton. Les deux pouvaient se
  // contredire : les jetons vivent dans la memoire du serveur, un
  // redemarrage les efface. L'ecran restait alors ouvert, complet, et
  // chaque action repondait « Jeton absent » sans qu'aucun chemin ne
  // ramene a la page de connexion.
  useEffect(() => {
    if (!pontPret) return;

    // Au demarrage : un utilisateur restaure sans jeton n'est pas
    // connecte, quoi qu'en dise le cache du navigateur.
    if (utilisateur && !sessionUtilisable()) {
      handleDeconnecter();
      return;
    }
    return surSessionPerdue(() => handleDeconnecter());
  }, [pontPret, utilisateur?.id]);

  // D43 : une base neuve n'a pas encore son dossier. Tant que le patron
  // n'a pas fait l'assistant, le serveur ne sert que lui : l'écran le
  // montre à la place de tout le reste. « inconnu » le temps de demander.
  const [assistant, setAssistant] = useState<"inconnu" | "a_faire" | "fait">("inconnu");
  useEffect(() => {
    if (!serveurJoint || !utilisateur) return;
    let fini = false;
    invoke<{ a_faire: boolean }>("lire_assistant")
      .then(r => { if (!fini) setAssistant(r.a_faire ? "a_faire" : "fait"); })
      // Un serveur d'avant D43 ne connaît pas la commande : pas d'assistant.
      .catch(() => { if (!fini) setAssistant("fait"); });
    return () => { fini = true; };
  }, [serveurJoint, utilisateur?.id]);

  // Les droits gardés par le navigateur datent de la connexion : on les
  // relit une fois le pont prêt (lib/droits.ts, relireDroits).
  useEffect(() => {
    if (!serveurJoint || !utilisateur || (enReseau() && !sessionUtilisable())) return;
    let annule = false;
    relireDroits(utilisateur).then(u => {
      if (annule || !u) return;
      UTILISATEUR_ACTIF = u;
      poserDroits(u);
      setUtilisateur(u);
      sauvegarderSession(u, pageActive, navParams);
    });
    return () => { annule = true; };
  }, [serveurJoint, utilisateur?.id]);

  // Le canal : ce que les autres caisses viennent de faire.
  //
  // On ne recharge pas l'ecran a chaque evenement — ce serait ecraser
  // une saisie en cours. On garde le compte, et les ecrans qui s'y
  // interessent le liront. Le serveur ne renvoie jamais au poste ce
  // qu'il a lui-meme provoque.
  useEffect(() => {
    if (!serveurJoint || !utilisateur || !enReseau()) return;
    const arreter = ecouterCanal((evenements) => {
      console.info("[canal]", evenements.length, "evenement(s)");
      setDerniereActivite(Date.now());
    });
    return arreter;
  }, [serveurJoint, utilisateur?.id]);

  /**
   * Sauvegarde hebdomadaire.
   *
   * Au démarrage, pas sur une minuterie : le poste d'un commerçant est
   * éteint le soir, une tâche à heure fixe ne partirait jamais.
   *
   * L'échec est AFFICHÉ. Une clé USB débranchée fait rater la copie ;
   * si on l'avalait en silence, l'interrupteur « activé » redeviendrait
   * le mensonge qu'il était — l'utilisateur se croirait protégé sans
   * l'être. C'est tout l'intérêt de la fonction.
   */
  useEffect(() => {
    // Seulement pour qui peut sauvegarder : pour les autres, le serveur
    // refusait à chaque connexion — une ligne REFUS au journal et une
    // erreur dans la console, pour rien (trouvé par le banc, v3 C-1).
    // Et seulement une fois le serveur joint, avec une session : sinon la
    // fenetre qui relance son serveur appelait dans le vide.
    if (!serveurJoint || !utilisateur || !sessionUtilisable() || !peut("sauvegarde:lancer")) return;
    invoke<{ effectuee: boolean; chemin?: string; erreur?: string }>(
      "sauvegarde_auto_si_necessaire",
    )
      .then(r => {
        if (r.erreur) {
          message(r.erreur, { title: "Sauvegarde automatique", kind: "warning" });
        } else if (r.effectuee) {
          console.info("Sauvegarde hebdomadaire effectuée :", r.chemin);
        }
      })
      .catch(e => console.error("Sauvegarde automatique :", e));
  }, [serveurJoint, utilisateur?.id]);

  function naviguer(page: string, params?: any) {
    setPageActive(page);
    setNavParams(params ?? null);
    if (utilisateur) sauvegarderSession(utilisateur, page, params ?? null);
  }

  /** L'Historique filtré sur une fiche, avec le chemin du retour. */
  function ouvrirHistorique(filtre: FiltreHistorique, page: string, params: unknown) {
    naviguer("historique", { filtre, retour: { page, params } });
  }

  function handleConnecte(u: UtilisateurConnecte) {
    UTILISATEUR_ACTIF = u;
    poserDroits(u);
    setUtilisateur(u);
    sauvegarderSession(u, "dashboard");
    setPageActive("dashboard");
    setNavParams(null);
    if (u.doit_changer_mdp) setModalMdp(true);
  }

  function handleDeconnecter() {
    setAssistant("inconnu");
    supprimerSession();
    UTILISATEUR_ACTIF = null;
    poserDroits(null);
    setUtilisateur(null);
    setPageActive("dashboard");
    setNavParams(null);
  }

  function rendrePage() {
    if (!utilisateur) return null;
    switch (pageActive) {
      case "dashboard":
        return (
          <Dashboard
            onAnomalies={peut("journal:lire") ? () =>
              ouvrirHistorique(
                { type_evenement: "anomalie", a_verifier: true, libelle: "Anomalies à vérifier" },
                "dashboard", null)
              : undefined}
            onExercices={peut("dossiers:gerer") ? () =>
              naviguer("parametres", { onglet: "dossiers" })
              : undefined}
          />
        );
      case "ventes":     return <Ventes />;
      case "pieces":
        return (
          <Pieces
            onOuvrirFicheClient={clientId =>
              naviguer("fiche_client", { clientId })}
            onOuvrirFicheFournisseur={fournisseurId =>
              naviguer("fiche_fournisseur", { fournisseurId })}
            onHistorique={peut("journal:lire") ? (pieceId, numero) =>
              ouvrirHistorique({ piece_id: pieceId, libelle: `Pièce : ${numero}` }, "pieces", null)
              : undefined}
          />
        );
      case "achats":     return <Achats />;
      case "stock":
        return (
          <Stock
            onHistorique={peut("journal:lire") ? (articleId, nom) =>
              ouvrirHistorique({ article_id: articleId, libelle: `Article : ${nom}` }, "stock", null)
              : undefined}
          />
        );
      case "clients":
        return (
          <Clients
            onOuvrirFiche={clientId =>
              naviguer("fiche_client", { clientId })}
          />
        );
      case "fiche_client":
        return navParams?.clientId ? (
          <FicheClient
            clientId={navParams.clientId}
            onRetour={() => naviguer("clients")}
            onHistorique={peut("journal:lire") ? (nom: string) =>
              ouvrirHistorique(
                { tiers_id: navParams.clientId, libelle: `Client : ${nom}` },
                "fiche_client", { clientId: navParams.clientId })
              : undefined}
          />
        ) : (
          <Clients
            onOuvrirFiche={clientId =>
              naviguer("fiche_client", { clientId })}
          />
        );
      case "fournisseurs":
        return (
          <Fournisseurs
            onOuvrirFiche={fournisseurId =>
              naviguer("fiche_fournisseur", { fournisseurId })}
          />
        );
      case "fiche_fournisseur":
        return navParams?.fournisseurId ? (
          <FicheFournisseur
            fournisseurId={navParams.fournisseurId}
            onRetour={() => naviguer("fournisseurs")}
            onHistorique={peut("journal:lire") ? (nom: string) =>
              ouvrirHistorique(
                { tiers_id: navParams.fournisseurId, libelle: `Fournisseur : ${nom}` },
                "fiche_fournisseur", { fournisseurId: navParams.fournisseurId })
              : undefined}
          />
        ) : (
          <Fournisseurs
            onOuvrirFiche={fournisseurId =>
              naviguer("fiche_fournisseur", { fournisseurId })}
          />
        );
      case "caisse":     return <Caisse />;
      case "retours":    return <Retours />;
      case "relances":   return <Relances />;
      case "journal":    return <Journal />;
      case "historique":
        return (
          <Historique
            filtreInitial={navParams?.filtre}
            onRetour={navParams?.retour
              ? () => naviguer(navParams.retour.page, navParams.retour.params)
              : undefined}
          />
        );
      case "transferts": return <Transferts />;
      case "cheques":    return <Cheques />;
      case "rapports":   return <Rapports />;
      case "parametres": return <Parametres ongletInitial={navParams?.onglet} />;
      default:           return <Dashboard />;
    }
  }

  if (!pontPret) {
    return (
      <div className="flex h-screen items-center justify-center
                      text-sm text-muted-foreground">
        Démarrage…
      </div>
    );
  }

  // Avant meme la connexion : OU est le serveur ? (D22/D24, revises le
  // 25/10/2026.) La fenetre ne parle qu'a lui, quel que soit le role du
  // poste. Sans lui, il n'y a personne a qui demander un mot de passe,
  // et les Parametres sont derriere la connexion : ces ecrans sont le
  // seul chemin, il ne faut pas qu'ils puissent coincer le poste.
  const modeReseau = etatReseau().mode;
  const brancher = () => { setBrancherAilleurs(false); setRevisionReseau((n) => n + 1); };
  // Premier demarrage : le role se demande, il ne se devine pas.
  if (brancherAilleurs || modeReseau === "a_choisir") {
    return brancherAilleurs
      ? <PageServeur onConfigure={brancher} onGarderBase={garderBase} />
      : <PageRolePoste onGarderBase={garderBase} onCaisse={() => setBrancherAilleurs(true)} />;
  }
  // Ce poste garde la base : son serveur est ici, on le joint ou on le
  // lance — rien a saisir.
  if (modeReseau === "monoposte" && !serveurLocalPret) {
    return <PageServeurLocal onPret={() => setServeurLocalPret(true)}
                             onCaisse={() => setBrancherAilleurs(true)} />;
  }
  if (!serveurConfigure()) {
    return <PageServeur onConfigure={brancher} onGarderBase={garderBase} />;
  }

  if (!utilisateur) {
    return <PageLogin onConnecte={handleConnecte} />;
  }

  if (assistant === "inconnu") {
    return (
      <div className="flex h-screen items-center justify-center text-sm text-muted-foreground">
        Ouverture du dossier…
      </div>
    );
  }
  if (assistant === "a_faire") {
    return (
      <AssistantDossier
        utilisateur={utilisateur}
        onDeconnecter={handleDeconnecter}
        onTermine={(nom) => {
          // Le mot de passe d'usine est remplacé : plus de fenêtre
          // « changer le mot de passe » par-dessus le tableau de bord.
          const u = { ...utilisateur, nom: nom || utilisateur.nom, doit_changer_mdp: false };
          setModalMdp(false);
          setUtilisateur(u);
          UTILISATEUR_ACTIF = u;
          poserDroits(u);
          sauvegarderSession(u, "dashboard");
          setPageActive("dashboard");
          setAssistant("fait");
        }}
      />
    );
  }

  // Fiche client/fournisseur → surligner l'onglet parent dans la sidebar
  const pageNavActive =
    pageActive === "fiche_client" ? "clients" :
    pageActive === "fiche_fournisseur" ? "fournisseurs" :
    pageActive;

  return (
    <>
      <Layout
        pageActive={pageNavActive}
        onNaviguer={naviguer}
        depotActif={depotActif}
        onChangerDepot={(id) => { definirDepotActif(id); setDepotActif(id); }}
        role={utilisateur.role}
        utilisateur={utilisateur}
        onChangerMdp={() => setModalMdp(true)}
        onDeconnecter={handleDeconnecter}
        activiteReseau={derniereActivite}
      >
        {rendrePage()}
      </Layout>

      <ModalChangerMdp
        ouvert={modalMdp}
        utilisateurId={utilisateur.id}
        obligatoire={utilisateur.doit_changer_mdp}
        onFermer={() => setModalMdp(false)}
        onChange={() => {
          setModalMdp(false);
          const u = { ...utilisateur, doit_changer_mdp: false };
          setUtilisateur(u);
          UTILISATEUR_ACTIF = u;
          poserDroits(u);
          sauvegarderSession(u, pageActive, navParams);
        }}
      />
    </>
  );
}

export default App;