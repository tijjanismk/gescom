import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle, Package, RefreshCw, Loader2,
  Plus, ArrowUpCircle, ClipboardList
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";

// =====================================================================
//  Types
// =====================================================================

interface StockArticle {
  article_id: string;
  article_nom: string;
  unite_base: string;
  depot_id: string;
  depot_nom: string;
  quantite: number;
}

interface Fournisseur {
  id: string;
  nom: string;
}

function formaterQuantite(q: number, unite: string): string {
  return `${q % 1 === 0 ? q : q.toFixed(2)} ${unite}`;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Modal : Entrée de stock (achat reçu)
// =====================================================================

function ModalEntreeStock({
  ouvert, article, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  article: StockArticle | null;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const [quantite, setQuantite] = useState("");
  const [prixAchat, setPrixAchat] = useState("");
  const [fournisseurs, setFournisseurs] = useState<Fournisseur[]>([]);
  const [fournisseurId, setFournisseurId] = useState("");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert) {
      setQuantite("");
      setPrixAchat("");
      setFournisseurId("");
      invoke<Fournisseur[]>("lire_fournisseurs").then(setFournisseurs).catch(console.error);
    }
  }, [ouvert]);

  async function handleConfirmer() {
    if (!article || !quantite || parseFloat(quantite) <= 0) return;
    setChargement(true);
    try {
      await invoke("enregistrer_entree_stock", {
        articleId: article.article_id,
        depotId: article.depot_id,
        quantite: parseFloat(quantite),
        prixAchat: prixAchat ? parseInt(prixAchat) : null,
        fournisseurId: fournisseurId || null,
      });
      onConfirmer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>
            Entrée de stock — {article?.article_nom}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">

          {/* Stock actuel */}
          <div className="bg-muted rounded-md px-3 py-2 text-sm">
            <span className="text-muted-foreground">Stock actuel : </span>
            <span className={`font-semibold ${(article?.quantite ?? 0) < 0 ? "text-red-500" : ""}`}>
              {article ? formaterQuantite(article.quantite, article.unite_base) : "—"}
            </span>
          </div>

          <div>
            <Label>Quantité reçue ({article?.unite_base}) *</Label>
            <Input
              type="number"
              value={quantite}
              onChange={e => setQuantite(e.target.value)}
              placeholder="Ex: 50"
              autoFocus
              className="mt-1"
            />
            {quantite && article && (
              <p className="text-xs text-green-600 mt-1">
                Stock après : {formaterQuantite(
                  article.quantite + parseFloat(quantite),
                  article.unite_base
                )}
              </p>
            )}
          </div>

          <div>
            <Label>Prix d'achat unitaire (F) <span className="text-muted-foreground text-xs">optionnel</span></Label>
            <Input
              type="number"
              value={prixAchat}
              onChange={e => setPrixAchat(e.target.value)}
              placeholder="Prix par unité de base"
              className="mt-1"
            />
          </div>

          <div>
            <Label>Fournisseur <span className="text-muted-foreground text-xs">optionnel</span></Label>
            <Select
              value={fournisseurId}
              onValueChange={v => { if (v) setFournisseurId(v); }}
            >
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="Choisir un fournisseur">
                  {fournisseurs.find(f => f.id === fournisseurId)?.nom ?? "Aucun"}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="aucun">Aucun</SelectItem>
                {fournisseurs.map(f => (
                  <SelectItem key={f.id} value={f.id}>{f.nom}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={handleConfirmer}
              disabled={!quantite || parseFloat(quantite) <= 0 || chargement}
              className="flex-1"
            >
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Ajustement d'inventaire (style Ciel)
// =====================================================================

function ModalAjustement({
  ouvert, article, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  article: StockArticle | null;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const [quantiteReelle, setQuantiteReelle] = useState("");
  const [motif, setMotif] = useState("inventaire");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert && article) {
      setQuantiteReelle(article.quantite.toString());
      setMotif("inventaire");
    }
  }, [ouvert, article]);

  const ecart = article && quantiteReelle
    ? parseFloat(quantiteReelle) - article.quantite
    : 0;

  async function handleConfirmer() {
    if (!article || !quantiteReelle || !motif.trim()) return;
    setChargement(true);
    try {
      await invoke("enregistrer_ajustement_inventaire", {
        articleId: article.article_id,
        depotId: article.depot_id,
        quantiteReelle: parseFloat(quantiteReelle),
        motif: motif.trim(),
      });
      onConfirmer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>
            Ajustement d'inventaire — {article?.article_nom}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">

          {/* Résumé */}
          <div className="grid grid-cols-3 gap-2 text-center text-sm">
            <div className="bg-muted rounded-md p-2">
              <p className="text-xs text-muted-foreground">Théorique</p>
              <p className="font-semibold">
                {article ? formaterQuantite(article.quantite, article.unite_base) : "—"}
              </p>
            </div>
            <div className="bg-muted rounded-md p-2">
              <p className="text-xs text-muted-foreground">Réel compté</p>
              <p className="font-semibold text-primary">
                {quantiteReelle
                  ? formaterQuantite(parseFloat(quantiteReelle), article?.unite_base ?? "")
                  : "—"
                }
              </p>
            </div>
            <div className={`rounded-md p-2 ${
              ecart > 0 ? "bg-green-50 dark:bg-green-950/30"
              : ecart < 0 ? "bg-red-50 dark:bg-red-950/30"
              : "bg-muted"
            }`}>
              <p className="text-xs text-muted-foreground">Écart</p>
              <p className={`font-semibold ${
                ecart > 0 ? "text-green-600"
                : ecart < 0 ? "text-red-600"
                : ""
              }`}>
                {ecart > 0 ? "+" : ""}{ecart !== 0 ? formaterQuantite(ecart, article?.unite_base ?? "") : "—"}
              </p>
            </div>
          </div>

          <div>
            <Label>Quantité réellement comptée ({article?.unite_base}) *</Label>
            <Input
              type="number"
              value={quantiteReelle}
              onChange={e => setQuantiteReelle(e.target.value)}
              className="mt-1"
              autoFocus
            />
          </div>

          <div>
            <Label>Motif * <span className="text-xs text-muted-foreground">(obligatoire)</span></Label>
            <Select
              value={motif}
              onValueChange={v => { if (v) setMotif(v); }}
            >
              <SelectTrigger className="mt-1">
                <SelectValue>
                  {{
                    inventaire: "Inventaire physique",
                    casse: "Casse / détérioration",
                    vol: "Vol / perte",
                    don: "Don / offert",
                    erreur: "Erreur de saisie",
                    autre: "Autre",
                  }[motif] ?? motif}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="inventaire">Inventaire physique</SelectItem>
                <SelectItem value="casse">Casse / détérioration</SelectItem>
                <SelectItem value="vol">Vol / perte</SelectItem>
                <SelectItem value="don">Don / offert</SelectItem>
                <SelectItem value="erreur">Erreur de saisie</SelectItem>
                <SelectItem value="autre">Autre</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={handleConfirmer}
              disabled={!quantiteReelle || !motif || ecart === 0 || chargement}
              className="flex-1"
            >
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Appliquer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Stock
// =====================================================================

export function Stock() {
  const [stocks, setStocks] = useState<StockArticle[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");
  const [articleEntree, setArticleEntree] = useState<StockArticle | null>(null);
  const [articleAjustement, setArticleAjustement] = useState<StockArticle | null>(null);

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

  async function handleApresOperation() {
    setArticleEntree(null);
    setArticleAjustement(null);
    await charger();
    await message("Opération enregistrée ✓", { title: "Succès", kind: "info" });
  }

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
                  <div className="flex items-center gap-2">
                    <Badge variant="destructive">
                      {formaterQuantite(s.quantite, s.unite_base)}
                    </Badge>
                    <Button size="sm" variant="outline"
                      onClick={() => setArticleEntree(s)}
                      className="h-7 text-xs">
                      <Plus className="h-3 w-3 mr-1" /> Entrée
                    </Button>
                  </div>
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
                  className="flex items-center justify-between py-2 px-3 rounded-md hover:bg-muted/40 transition-colors group">
                  <div>
                    <p className="text-sm font-medium">{s.article_nom}</p>
                    <p className="text-xs text-muted-foreground">{s.depot_nom}</p>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge
                      variant={s.quantite < 10 ? "outline" : "secondary"}
                      className={s.quantite < 10 ? "text-orange-500 border-orange-300" : ""}
                    >
                      {formaterQuantite(s.quantite, s.unite_base)}
                    </Badge>
                    {/* Boutons d'action — visibles au survol */}
                    <div className="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                      <Button size="sm" variant="ghost"
                        onClick={() => setArticleEntree(s)}
                        className="h-7 text-xs px-2">
                        <ArrowUpCircle className="h-3 w-3 mr-1" /> Entrée
                      </Button>
                      <Button size="sm" variant="ghost"
                        onClick={() => setArticleAjustement(s)}
                        className="h-7 text-xs px-2">
                        <ClipboardList className="h-3 w-3 mr-1" /> Ajuster
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Modals */}
      <ModalEntreeStock
        ouvert={articleEntree !== null}
        article={articleEntree}
        onFermer={() => setArticleEntree(null)}
        onConfirmer={handleApresOperation}
      />

      <ModalAjustement
        ouvert={articleAjustement !== null}
        article={articleAjustement}
        onFermer={() => setArticleAjustement(null)}
        onConfirmer={handleApresOperation}
      />
    </div>
  );
}