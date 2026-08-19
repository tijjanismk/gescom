import { useState } from "react";
import { Layout } from "@/components/Layout";
import { PageLogin, UtilisateurConnecte } from "@/pages/PageLogin";
import { ModalChangerMdp } from "@/components/ModalChangerMdp";
import { Dashboard } from "@/pages/Dashboard";
import { Ventes } from "@/pages/Ventes";
import { Achats } from "@/pages/Achats";
import { Stock } from "@/pages/Stock";
import { Clients } from "@/pages/Clients";
import { Fournisseurs } from "@/pages/Fournisseurs";
import { Caisse } from "@/pages/Caisse";
import { Parametres } from "@/pages/Parametres";
import { Retours } from "@/pages/Retours";

// Contexte utilisateur global — accessible par les commandes Tauri via le rôle
export let UTILISATEUR_ACTIF: UtilisateurConnecte | null = null;

function App() {
  const [utilisateur, setUtilisateur] = useState<UtilisateurConnecte | null>(null);
  const [pageActive, setPageActive] = useState("dashboard");
  const [modalMdp, setModalMdp] = useState(false);

  function handleConnecte(u: UtilisateurConnecte) {
    UTILISATEUR_ACTIF = u;
    setUtilisateur(u);

    // Forcer le changement de mot de passe à la première connexion.
    if (u.doit_changer_mdp) {
      setModalMdp(true);
    }
  }

  function rendrePage() {
    if (!utilisateur) return null;
    switch (pageActive) {
      case "dashboard":    return <Dashboard />;
      case "ventes":       return <Ventes />;
      case "achats":       return <Achats />;
      case "stock":        return <Stock />;
      case "clients":      return <Clients />;
      case "fournisseurs": return <Fournisseurs />;
      case "caisse":       return <Caisse />;
      case "retours":      return <Retours />;
      case "parametres":   return <Parametres />;
      default:             return <Dashboard />;
    }
  }

  // Page de login
  if (!utilisateur) {
    return <PageLogin onConnecte={handleConnecte} />;
  }

  return (
    <>
      <Layout
        pageActive={pageActive}
        onNaviguer={setPageActive}
        role={utilisateur.role}
        utilisateur={utilisateur}
        onChangerMdp={() => setModalMdp(true)}
      >
        {rendrePage()}
      </Layout>

      {/* Modal changement de mot de passe */}
      <ModalChangerMdp
        ouvert={modalMdp}
        utilisateurId={utilisateur.id}
        obligatoire={utilisateur.doit_changer_mdp}
        onFermer={() => setModalMdp(false)}
        onChange={() => {
          setModalMdp(false);
          // Marquer que le mdp a été changé dans l'état local.
          setUtilisateur(prev => prev ? { ...prev, doit_changer_mdp: false } : prev);
          UTILISATEUR_ACTIF = utilisateur
            ? { ...utilisateur, doit_changer_mdp: false }
            : null;
        }}
      />
    </>
  );
}

export default App;
