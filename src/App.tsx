import { useState } from "react";
import { Layout } from "@/components/Layout";
import { SelecteurProfil } from "@/components/SelecteurProfil";
import { Dashboard } from "@/pages/Dashboard";
import { Ventes } from "@/pages/Ventes";
import { Achats } from "@/pages/Achats";
import { Stock } from "@/pages/Stock";
import { Clients } from "@/pages/Clients";
import { Fournisseurs } from "@/pages/Fournisseurs";
import { Caisse } from "@/pages/Caisse";
import { Parametres } from "@/pages/Parametres";

// Contexte du profil actif — partagé avec les pages qui en ont besoin
export type Role = "patron" | "employe";

export const UTILISATEUR_ROLE_ACTIF: { role: Role } = { role: "patron" };

function App() {
  const [role, setRole] = useState<Role | null>(null);
  const [pageActive, setPageActive] = useState("dashboard");

  // Sélecteur de profil au démarrage
  if (!role) {
    return (
      <SelecteurProfil
        onSelectionner={r => {
          UTILISATEUR_ROLE_ACTIF.role = r;
          setRole(r);
        }}
      />
    );
  }

  function rendrePage() {
    switch (pageActive) {
      case "dashboard":   return <Dashboard />;
      case "ventes":      return <Ventes />;
      case "achats":      return <Achats />;
      case "stock":       return <Stock />;
      case "clients":     return <Clients />;
      case "fournisseurs": return <Fournisseurs />;
      case "caisse":      return <Caisse />;
      case "parametres":  return <Parametres />;
      default:            return <Dashboard />;
    }
  }

  return (
    <Layout
      pageActive={pageActive}
      onNaviguer={setPageActive}
      role={role}
    >
      {rendrePage()}
    </Layout>
  );
}

export default App;