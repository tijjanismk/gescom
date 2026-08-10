import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Users, TrendingUp, Loader2, Search } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";

interface ClientAvecCreance {
  id: string;
  code: string;
  nom: string;
  telephone?: string;
  total_creances: number;
  nb_ventes: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

export function Clients() {
  const [clients, setClients] = useState<ClientAvecCreance[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");

  useEffect(() => {
    async function charger() {
      try {
        const data = await invoke<ClientAvecCreance[]>("lire_clients_avec_creances");
        setClients(data);
      } catch (e) {
        console.error("Erreur chargement clients :", e);
      } finally {
        setChargement(false);
      }
    }
    charger();
  }, []);

  const filtres = clients.filter(c =>
    c.nom.toLowerCase().includes(recherche.toLowerCase()) ||
    (c.telephone && c.telephone.includes(recherche))
  );

  const avecCreances = filtres.filter(c => c.total_creances > 0);
  const sansCreances = filtres.filter(c => c.total_creances === 0);

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
        <h1 className="text-2xl font-semibold">Clients</h1>
        <Badge variant="secondary">{clients.length} clients</Badge>
      </div>

      <div className="relative mb-6 max-w-sm">
        <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input value={recherche} onChange={e => setRecherche(e.target.value)}
          placeholder="Rechercher..." className="pl-8" />
      </div>

      {/* Clients avec créances ouvertes */}
      {avecCreances.length > 0 && (
        <Card className="mb-6">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2 text-orange-500">
              <TrendingUp className="h-4 w-4" />
              Créances ouvertes ({avecCreances.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-1">
              {avecCreances.map(c => (
                <div key={c.id}
                  className="flex items-center justify-between py-2.5 px-3 rounded-md hover:bg-muted/40 transition-colors">
                  <div>
                    <p className="text-sm font-medium">{c.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {c.code}{c.telephone ? ` · ${c.telephone}` : ""}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-semibold text-orange-500">
                      {formaterMontant(c.total_creances)}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Tous les clients */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Users className="h-4 w-4" />
            Tous les clients ({sansCreances.length} à jour)
          </CardTitle>
        </CardHeader>
        <CardContent>
          {sansCreances.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              Aucun client
            </p>
          ) : (
            <div className="space-y-1">
              {sansCreances.map(c => (
                <div key={c.id}
                  className="flex items-center justify-between py-2.5 px-3 rounded-md hover:bg-muted/40 transition-colors">
                  <div>
                    <p className="text-sm font-medium">{c.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {c.code}{c.telephone ? ` · ${c.telephone}` : ""}
                    </p>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
