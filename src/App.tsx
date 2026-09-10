import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Layout } from "@/components/Layout";
import { PageLogin, UtilisateurConnecte } from "@/pages/PageLogin";
import { ModalChangerMdp } from "@/components/ModalChangerMdp";
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
import { Modeles } from "@/pages/Modeles";
import { assurerModelesParDefaut } from "@/lib/modeles/service";
import { PageLicence, BandeauEssai, autorise } from "@/pages/PageLicence";
import type { EtatLicence } from "@/pages/PageLicence";
import { enReseau } from "@/lib/pont";
import { Retours } from "@/pages/Retours";
import { Relances } from "@/pages/Relances";
import { Rapports } from "@/pages/Rapports";
import { Journal } from "@/pages/Journal";
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
  // Sert uniquement à forcer le rendu quand le dépôt change ;
  // la valeur de référence reste DEPOT_ACTIF.
  const [depotActif, setDepotActif] = useState<string | null>(DEPOT_ACTIF);

  // ⚠️ NE PAS remettre cette affectation dans un useEffect.
  //
  // Les effets des ENFANTS s'executent avant ceux du parent. Parametres
  // lisait donc UTILISATEUR_ACTIF encore null au premier rendu et
  // concluait « employe » : seuls 4 onglets apparaissaient, et il
  // fallait rafraichir pour voir les 12.
  //
  // L'affectation doit avoir lieu pendant le rendu, avant que le moindre
  // enfant ne soit monte.
  if (utilisateur && UTILISATEUR_ACTIF?.id !== utilisateur.id) {
    UTILISATEUR_ACTIF = utilisateur;
  }

  useEffect(() => {
    if (utilisateur?.doit_changer_mdp) setModalMdp(true);
  }, []);

  // Les modeles d'usine sont poses au premier demarrage, jamais
  // reecrits ensuite. Ici et pas dans l'ecran Modeles : une facture
  // doit pouvoir s'imprimer par un modele sans que personne ne soit
  // jamais alle voir l'atelier.
  //
  // L'echec est silencieux et volontairement : une base en lecture
  // seule ou un disque plein ne doit pas empecher d'ouvrir la caisse.
  // L'impression retombe alors sur le generateur historique.
  useEffect(() => {
    assurerModelesParDefaut().catch((e) =>
      console.error("Modeles par defaut :", e),
    );
  }, []);

  // ---- Licence ----
  //
  // En mode POSTE CAISSE, on ne verifie rien ici : la licence vit sur
  // le serveur, qui compte les postes a la connexion. Demander une
  // licence a chaque caisse obligerait le commercant qui en achete
  // trois a activer trois machines, et ses caisses s'arreteraient au
  // bout de trente jours.
  const [licence, setLicence] = useState<EtatLicence | null>(null);
  const [licenceLue, setLicenceLue] = useState(enReseau());

  useEffect(() => {
    if (enReseau()) return;
    invoke<EtatLicence>("lire_etat_licence")
      .then(setLicence)
      .catch((e) => {
        // Une verification qui echoue ne doit pas fermer la boutique :
        // le blocage doit venir d'un refus explicite, jamais d'un bug
        // de lecture de fichier.
        console.error("Licence :", e);
        setLicence(null);
      })
      .finally(() => setLicenceLue(true));
  }, []);

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
    if (!utilisateur) return;
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
  }, [utilisateur?.id]);

  function naviguer(page: string, params?: any) {
    setPageActive(page);
    setNavParams(params ?? null);
    if (utilisateur) sauvegarderSession(utilisateur, page, params ?? null);
  }

  function handleConnecte(u: UtilisateurConnecte) {
    UTILISATEUR_ACTIF = u;
    setUtilisateur(u);
    sauvegarderSession(u, "dashboard");
    setPageActive("dashboard");
    setNavParams(null);
    if (u.doit_changer_mdp) setModalMdp(true);
  }

  function handleDeconnecter() {
    supprimerSession();
    UTILISATEUR_ACTIF = null;
    setUtilisateur(null);
    setPageActive("dashboard");
    setNavParams(null);
  }

  function rendrePage() {
    if (!utilisateur) return null;
    switch (pageActive) {
      case "dashboard":  return <Dashboard />;
      case "ventes":     return <Ventes />;
      case "pieces":
        return (
          <Pieces
            onOuvrirFicheClient={clientId =>
              naviguer("fiche_client", { clientId })}
            onOuvrirFicheFournisseur={fournisseurId =>
              naviguer("fiche_fournisseur", { fournisseurId })}
          />
        );
      case "achats":     return <Achats />;
      case "stock":      return <Stock />;
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
      case "transferts": return <Transferts />;
      case "cheques":    return <Cheques />;
      case "rapports":   return <Rapports />;
      case "modeles":    return <Modeles />;
      case "parametres": return <Parametres />;
      default:           return <Dashboard />;
    }
  }

  // L'ecran de licence passe AVANT la connexion : sans droit d'usage,
  // il n'y a pas de raison de demander un mot de passe.
  if (licenceLue && licence && !autorise(licence)) {
    return <PageLicence etat={licence} onActive={setLicence} />;
  }

  if (!utilisateur) {
    return <PageLogin onConnecte={handleConnecte} />;
  }

  // Le rappel n'apparait que dans la derniere semaine. Plus tot, c'est
  // du harcelement commercial dans un logiciel de comptoir.
  const bandeau =
    licence?.etat === "essai"
    && typeof licence.jours_restants === "number"
    && licence.jours_restants <= 7
      ? <BandeauEssai jours={licence.jours_restants} />
      : null;

  // Fiche client/fournisseur → surligner l'onglet parent dans la sidebar
  const pageNavActive =
    pageActive === "fiche_client" ? "clients" :
    pageActive === "fiche_fournisseur" ? "fournisseurs" :
    pageActive;

  return (
    <>
      {bandeau}
      <Layout
        pageActive={pageNavActive}
        onNaviguer={naviguer}
        depotActif={depotActif}
        onChangerDepot={(id) => { definirDepotActif(id); setDepotActif(id); }}
        role={utilisateur.role}
        utilisateur={utilisateur}
        onChangerMdp={() => setModalMdp(true)}
        onDeconnecter={handleDeconnecter}
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
          sauvegarderSession(u, pageActive, navParams);
        }}
      />
    </>
  );
}

export default App;