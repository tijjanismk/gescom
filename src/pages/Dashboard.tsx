import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  TrendingUp, Wallet, Users, AlertTriangle,
  Loader2, RefreshCw, ShoppingCart, Package
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

interface ResumeDashboard {
  ventes_jour: number;
  nb_ventes_jour: number;
  caisse_solde: number;
  caisse_ouverte: boolean;
  total_creances: number;
  nb_clients_creance: number;
  articles_a_regulariser: number;
}

interface VenteRecente {
  client_nom: string;
  total: number;
  statut: string;
  date_vente: string;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function fmtHeure(iso: string): string {
  return new Date(iso).toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}

interface KpiCardProps {
  titre: string;
  valeur: string;
  sous_titre?: string;
  icone: React.ComponentType<{ className?: string }>;
  couleur?: string;
  alerte?: boolean;
}

function KpiCard({ titre, valeur, sous_titre, icone: Icone, couleur = "text-foreground", alerte }: KpiCardProps) {
  return (
    <Card className={alerte ? "border-orange-200" : ""}>
      <CardContent className="pt-5">
        <div className="flex items-start justify-between">
          <div>
            <p className="text-xs text-muted-foreground mb-1">{titre}</p>
            <p className={`text-2xl font-bold ${couleur}`}>{valeur}</p>
            {sous_titre && (
              <p className="text-xs text-muted-foreground mt-1">{sous_titre}</p>
            )}
          </div>
          <div className={`p-2 rounded-lg ${alerte ? "bg-orange-100" : "bg-muted"}`}>
            <Icone className={`h-5 w-5 ${alerte ? "text-orange-500" : "text-muted-foreground"}`} />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

export function Dashboard() {
  const [resume, setResume] = useState<ResumeDashboard | null>(null);
  const [ventesRecentes, setVentesRecentes] = useState<VenteRecente[]>([]);
  const [chargement, setChargement] = useState(true);

  async function charger() {
    setChargement(true);
    try {
      const [r, v] = await Promise.all([
        invoke<ResumeDashboard>("lire_resume_dashboard"),
        invoke<VenteRecente[]>("lire_ventes_du_jour"),
      ]);
      setResume(r);
      setVentesRecentes(v);
    } catch (e) {
      console.error("Erreur dashboard :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  if (chargement) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold">Tableau de bord</h1>
          <p className="text-sm text-muted-foreground">
            {new Date().toLocaleDateString("fr-ML", {
              weekday: "long", day: "numeric",
              month: "long", year: "numeric",
            })}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      {/* KPIs */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
        <KpiCard
          titre="Ventes aujourd'hui"
          valeur={fmt(resume?.ventes_jour ?? 0)}
          sous_titre={`${resume?.nb_ventes_jour ?? 0} vente${(resume?.nb_ventes_jour ?? 0) > 1 ? "s" : ""}`}
          icone={ShoppingCart}
          couleur="text-primary"
        />
        <KpiCard
          titre="Caisse"
          valeur={fmt(resume?.caisse_solde ?? 0)}
          sous_titre={resume?.caisse_ouverte ? "Session ouverte" : "Session fermée"}
          icone={Wallet}
          couleur={resume?.caisse_ouverte ? "text-green-600" : "text-muted-foreground"}
        />
        <KpiCard
          titre="Créances clients"
          valeur={fmt(resume?.total_creances ?? 0)}
          sous_titre={`${resume?.nb_clients_creance ?? 0} client${(resume?.nb_clients_creance ?? 0) > 1 ? "s" : ""}`}
          icone={Users}
          couleur={(resume?.total_creances ?? 0) > 0 ? "text-orange-500" : "text-foreground"}
          alerte={(resume?.total_creances ?? 0) > 0}
        />
        <KpiCard
          titre="Stock à régulariser"
          valeur={String(resume?.articles_a_regulariser ?? 0)}
          sous_titre="articles en négatif"
          icone={Package}
          alerte={(resume?.articles_a_regulariser ?? 0) > 0}
        />
      </div>

      {/* Ventes du jour */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <TrendingUp className="h-4 w-4" />
            Ventes du jour
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {ventesRecentes.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              Aucune vente aujourd'hui
            </p>
          ) : (
            <div className="divide-y divide-border">
              {ventesRecentes.map((v, i) => (
                <div key={i}
                  className="flex items-center justify-between px-4 py-2.5 hover:bg-muted/40">
                  <div>
                    <p className="text-sm font-medium">{v.client_nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {fmtHeure(v.date_vente)}
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold">{fmt(v.total)}</span>
                    <Badge variant={
                      v.statut === "payee" ? "secondary"
                      : v.statut === "creance_ouverte" ? "destructive"
                      : "outline"
                    } className="text-xs">
                      {v.statut === "payee" ? "Payée"
                        : v.statut === "creance_ouverte" ? "Créance"
                        : "Partiel"}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
