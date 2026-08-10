import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ShoppingCart, TrendingUp, AlertTriangle, Wallet, Loader2
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface ResumeDashboard {
  ventes_du_jour: number;
  nb_ventes_jour: number;
  caisse_du_jour: number;
  creances_ouvertes: number;
  nb_clients_creance: number;
  articles_a_regulariser: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function formaterDate(): string {
  return new Date().toLocaleDateString("fr-ML", {
    weekday: "long", day: "numeric", month: "long", year: "numeric"
  });
}

export function Dashboard() {
  const [resume, setResume] = useState<ResumeDashboard | null>(null);
  const [chargement, setChargement] = useState(true);

  useEffect(() => {
    async function charger() {
      try {
        const data = await invoke<ResumeDashboard>("lire_resume_dashboard");
        setResume(data);
      } catch (e) {
        console.error("Erreur dashboard :", e);
      } finally {
        setChargement(false);
      }
    }
    charger();
  }, []);

  const indicateurs = resume ? [
    {
      titre: "Ventes du jour",
      valeur: formaterMontant(resume.ventes_du_jour),
      sousTitre: `${resume.nb_ventes_jour} vente${resume.nb_ventes_jour > 1 ? "s" : ""}`,
      icone: ShoppingCart,
      couleur: "text-blue-500",
    },
    {
      titre: "Caisse du jour",
      valeur: formaterMontant(resume.caisse_du_jour),
      sousTitre: "Encaissé aujourd'hui",
      icone: Wallet,
      couleur: "text-green-500",
    },
    {
      titre: "Créances ouvertes",
      valeur: formaterMontant(resume.creances_ouvertes),
      sousTitre: `${resume.nb_clients_creance} client${resume.nb_clients_creance > 1 ? "s" : ""}`,
      icone: TrendingUp,
      couleur: "text-orange-500",
    },
    {
      titre: "Articles à régulariser",
      valeur: resume.articles_a_regulariser.toString(),
      sousTitre: "Stock négatif",
      icone: AlertTriangle,
      couleur: resume.articles_a_regulariser > 0 ? "text-red-500" : "text-muted-foreground",
    },
  ] : [];

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-semibold">Tableau de bord</h1>
        <p className="text-muted-foreground text-sm mt-1 capitalize">{formaterDate()}</p>
      </div>

      {chargement ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4 mb-6">
            {indicateurs.map(ind => {
              const Icone = ind.icone;
              return (
                <Card key={ind.titre}>
                  <CardHeader className="flex flex-row items-center justify-between pb-2">
                    <CardTitle className="text-sm font-medium text-muted-foreground">
                      {ind.titre}
                    </CardTitle>
                    <Icone className={`h-4 w-4 ${ind.couleur}`} />
                  </CardHeader>
                  <CardContent>
                    <p className="text-2xl font-bold">{ind.valeur}</p>
                    <p className="text-xs text-muted-foreground mt-1">{ind.sousTitre}</p>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}