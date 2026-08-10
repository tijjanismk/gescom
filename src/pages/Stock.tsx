import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Package, RefreshCw, Loader2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface StockArticle {
  article_id: string;
  article_nom: string;
  unite_base: string;
  depot_nom: string;
  quantite: number;
}

function formaterQuantite(q: number, unite: string): string {
  return `${q % 1 === 0 ? q : q.toFixed(2)} ${unite}`;
}

export function Stock() {
  const [stocks, setStocks] = useState<StockArticle[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<StockArticle[]>("lire_stocks");
      setStocks(data);
    } catch (e) {
      console.error("Erreur chargement stock :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  const filtres = stocks.filter(s =>
    s.article_nom.toLowerCase().includes(recherche.toLowerCase())
  );

  const aRegulariser = filtres.filter(s => s.quantite < 0);
  const normaux = filtres.filter(s => s.quantite >= 0);

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
        <h1 className="text-2xl font-semibold">Stock</h1>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      <Input
        value={recherche}
        onChange={e => setRecherche(e.target.value)}
        placeholder="Rechercher un article..."
        className="mb-6 max-w-sm"
      />

      {/* Articles à régulariser */}
      {aRegulariser.length > 0 && (
        <Card className="mb-6 border-red-200 bg-red-50 dark:bg-red-950/20">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2 text-red-600">
              <AlertTriangle className="h-4 w-4" />
              À régulariser ({aRegulariser.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {aRegulariser.map(s => (
                <div key={s.article_id}
                  className="flex items-center justify-between py-2 px-3 bg-white dark:bg-red-950/40 rounded-md">
                  <div>
                    <p className="text-sm font-medium">{s.article_nom}</p>
                    <p className="text-xs text-muted-foreground">{s.depot_nom}</p>
                  </div>
                  <Badge variant="destructive">
                    {formaterQuantite(s.quantite, s.unite_base)}
                  </Badge>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Stock normal */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Package className="h-4 w-4" />
            Stock disponible ({normaux.length} articles)
          </CardTitle>
        </CardHeader>
        <CardContent>
          {normaux.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              Aucun article en stock
            </p>
          ) : (
            <div className="space-y-1">
              {normaux.map(s => (
                <div key={s.article_id}
                  className="flex items-center justify-between py-2 px-3 rounded-md hover:bg-muted/40 transition-colors">
                  <div>
                    <p className="text-sm font-medium">{s.article_nom}</p>
                    <p className="text-xs text-muted-foreground">{s.depot_nom}</p>
                  </div>
                  <Badge variant={s.quantite < 10 ? "outline" : "secondary"}
                    className={s.quantite < 10 ? "text-orange-500 border-orange-300" : ""}>
                    {formaterQuantite(s.quantite, s.unite_base)}
                  </Badge>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
