import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle, Package, RefreshCw, Loader2,
  ArrowUpCircle, ClipboardList, X
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
import { Pagination } from "@/components/Pagination";
import { cn } from "@/lib/utils";

const LIMITE = 40;

interface StockRow {
  article_id: string;
  article_nom: string;
  unite_base: string;
  depot_nom: string;
  depot_id: string;
  quantite: number;
}

interface Fournisseur {
  id: string;
  nom: string;
}

interface PageResult {
  donnees: StockRow[];
  total: number;
  pages: number;
  page: number;
}

function formaterQuantite(q: number, unite: string): string {
  return `${q % 1 === 0 ? q : q.toFixed(2)} ${unite}`;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function ModalEntreeStock({
  ouvert, article, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  article: StockRow | null;
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
      setQuantite(""); setPrixAchat(""); setFournisseurId("");
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
        fournisseurId: fournisseurId && fournisseurId !== "aucun" ? fournisseurId : null,
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
          <DialogTitle>Entrée de stock — {article?.article_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 text-sm">
            <span className="text-muted-foreground">Stock actuel : </span>
            <span className={`font-semibold ${(article?.quantite ?? 0) < 0 ? "text-red-500" : ""}`}>
              {article ? formaterQuantite(article.quantite, article.unite_base) : "—"}
            </span>
          </div>
          <div>
            <Label>Quantité reçue ({article?.unite_base}) *</Label>
            <Input type="number" value={quantite}
              onChange={e => setQuantite(e.target.value)}
              placeholder="Ex: 50" autoFocus className="mt-1" />
            {quantite && article && (
              <p className="text-xs text-green-600 mt-1">
                Stock après : {formaterQuantite(
                  article.quantite + parseFloat(quantite), article.unite_base
                )}
              </p>
            )}
          </div>
          <div>
            <Label>Prix d'achat (F/{article?.unite_base})
              <span className="text-muted-foreground text-xs ml-1">optionnel</span>
            </Label>
            <Input type="number" value={prixAchat}
              onChange={e => setPrixAchat(e.target.value)}
              placeholder="Prix unitaire" className="mt-1" />
          </div>
          <div>
            <Label>Fournisseur <span className="text-muted-foreground text-xs">optionnel</span></Label>
            <Select value={fournisseurId} onValueChange={v => { if (v) setFournisseurId(v); }}>
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="Aucun">
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
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleConfirmer}
              disabled={!quantite || parseFloat(quantite) <= 0 || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function ModalAjustement({
  ouvert, article, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  article: StockRow | null;
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
    ? parseFloat(quantiteReelle) - article.quantite : 0;

  async function handleConfirmer() {
    if (!article || !quantiteReelle || !motif.trim()) return;
    setChargement(true);
    try {
      await invoke("enregistrer_ajustement_inventaire", {
        articleId: article.article_id,
        depotId: article.depot_id,
        quantiteReelle: parseFloat(quantiteReelle),
        motif,
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
          <DialogTitle>Ajustement — {article?.article_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
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
                  : "—"}
              </p>
            </div>
            <div className={`rounded-md p-2 ${
              ecart > 0 ? "bg-green-50" : ecart < 0 ? "bg-red-50" : "bg-muted"
            }`}>
              <p className="text-xs text-muted-foreground">Écart</p>
              <p className={`font-semibold ${
                ecart > 0 ? "text-green-600" : ecart < 0 ? "text-red-600" : ""
              }`}>
                {ecart !== 0
                  ? `${ecart > 0 ? "+" : ""}${formaterQuantite(ecart, article?.unite_base ?? "")}`
                  : "—"}
              </p>
            </div>
          </div>

          <div>
            <Label>Quantité réellement comptée ({article?.unite_base}) *</Label>
            <Input type="number" value={quantiteReelle}
              onChange={e => setQuantiteReelle(e.target.value)}
              className="mt-1" autoFocus />
          </div>

          <div>
            <Label>Motif *</Label>
            <Select value={motif} onValueChange={v => { if (v) setMotif(v); }}>
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

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleConfirmer}
              disabled={!quantiteReelle || ecart === 0 || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Appliquer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export function Stock() {
  const [resultat, setResultat] = useState<PageResult>({
    donnees: [], total: 0, pages: 0, page: 0,
  });
  const [chargement, setChargement] = useState(true);
  const [page, setPage] = useState(0);
  const [recherche, setRecherche] = useState("");
  const [aRegulariserSeulement, setARegulariserSeulement] = useState(false);
  const [articleEntree, setArticleEntree] = useState<StockRow | null>(null);
  const [articleAjustement, setArticleAjustement] = useState<StockRow | null>(null);

  const charger = useCallback(async (p: number) => {
    setChargement(true);
    try {
      const data = await invoke<PageResult>("lire_stocks_pagines", {
        page: p,
        limite: LIMITE,
        recherche: recherche || null,
        aRegulariserSeulement,
        categorieId: null,
      });
      setResultat(data);
    } catch (e) {
      console.error("Erreur stock :", e);
    } finally {
      setChargement(false);
    }
  }, [recherche, aRegulariserSeulement]);

  useEffect(() => { setPage(0); charger(0); }, [recherche, aRegulariserSeulement]);
  useEffect(() => { charger(page); }, [page]);

  const nbRegulariser = resultat.donnees.filter(s => s.quantite < 0).length;

  async function handleApresOperation(type: "entree" | "ajustement") {
    if (type === "entree") setArticleEntree(null);
    else setArticleAjustement(null);
    await charger(page);
    await message("Opération enregistrée ✓", { title: "Succès", kind: "info" });
  }

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Stock</h1>
        <Button variant="outline" size="sm" onClick={() => charger(page)}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      {/* Filtres */}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <Input value={recherche} onChange={e => setRecherche(e.target.value)}
          placeholder="Rechercher un article..."
          className="h-8 text-sm w-48" />

        <button
          onClick={() => setARegulariserSeulement(!aRegulariserSeulement)}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-md border text-xs font-medium transition-colors",
            aRegulariserSeulement
              ? "border-red-400 bg-red-50 text-red-700"
              : "border-border text-muted-foreground hover:bg-muted"
          )}
        >
          <AlertTriangle className="h-3 w-3" />
          À régulariser {nbRegulariser > 0 && `(${nbRegulariser})`}
        </button>

        {(recherche || aRegulariserSeulement) && (
          <Button variant="ghost" size="sm" className="h-8 text-xs"
            onClick={() => { setRecherche(""); setARegulariserSeulement(false); }}>
            <X className="h-3 w-3 mr-1" /> Réinitialiser
          </Button>
        )}
      </div>

      {chargement ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Package className="h-4 w-4" />
              {resultat.total} articles
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {resultat.donnees.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun article
              </p>
            ) : (
              <>
                <div className="divide-y divide-border">
                  {resultat.donnees.map(s => (
                    <div key={s.article_id}
                      className="flex items-center justify-between py-2.5 px-4 hover:bg-muted/40 transition-colors group">
                      <div>
                        <p className="text-sm font-medium">{s.article_nom}</p>
                        <p className="text-xs text-muted-foreground">{s.depot_nom}</p>
                      </div>
                      <div className="flex items-center gap-2">
                        <Badge
                          variant={s.quantite < 0 ? "destructive"
                            : s.quantite < 10 ? "outline" : "secondary"}
                          className={s.quantite > 0 && s.quantite < 10
                            ? "text-orange-500 border-orange-300" : ""}
                        >
                          {formaterQuantite(s.quantite, s.unite_base)}
                        </Badge>
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
                <Pagination
                  page={page}
                  total={resultat.total}
                  limite={LIMITE}
                  onChanger={p => { setPage(p); window.scrollTo(0, 0); }}
                />
              </>
            )}
          </CardContent>
        </Card>
      )}

      <ModalEntreeStock
        ouvert={articleEntree !== null}
        article={articleEntree}
        onFermer={() => setArticleEntree(null)}
        onConfirmer={() => handleApresOperation("entree")}
      />
      <ModalAjustement
        ouvert={articleAjustement !== null}
        article={articleAjustement}
        onFermer={() => setArticleAjustement(null)}
        onConfirmer={() => handleApresOperation("ajustement")}
      />
    </div>
  );
}