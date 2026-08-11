import { useState } from "react";
import { Layout } from "@/components/Layout";
import { Dashboard } from "@/pages/Dashboard";
import {Ventes} from '@/pages/Ventes'
import { Stock } from "@/pages/Stock";
import { Clients } from "@/pages/Clients";
import { Caisse } from "@/pages/Caisse";
import {Parametres} from "@/pages/Parametres";
import { Fournisseurs } from "@/pages/Fournisseurs";

// On ajoutera les autres pages ici au fur et à mesure


function App() {
  const [pageActive, setPageActive] = useState("dashboard");

  function rendrePage() {
    switch (pageActive) {
      case "dashboard":
        return <Dashboard />;
      case "ventes":
        return <Ventes />;
      case "stock":    return <Stock />;
      case "clients":  return <Clients />;
      case "fournisseurs": return <Fournisseurs />;
      case "caisse":   return <Caisse />;
      case "parametres":
        return <Parametres />;
      default:
        return <Dashboard />;
    }
  }

  return (
    <Layout pageActive={pageActive} onNaviguer={setPageActive}>
      {rendrePage()}
    </Layout>
  );
}

export default App;