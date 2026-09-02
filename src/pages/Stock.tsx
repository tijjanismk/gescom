import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle, Package, RefreshCw, Loader2,
  ArrowUpCircle, ClipboardList, X, Printer,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DEPOT_ACTIF } from "@/App";
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

interface UniteVente {
  id: string; libelle: string; facteur: number;
}
interface StockRow {
  article_id: string;
  article_nom: string;
  unite_base: string;
  /** Conditionnements disponibles — sac de 50 kg, carton de 12… */
  unites?: UniteVente[];
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
  const [uniteChoisie, setUniteChoisie] = useState("");

  // Unite retenue pour la saisie. Par defaut la plus petite — c'est
  // celle du stock, donc la conversion est neutre.
  const uniteSel = article?.unites?.find(u => u.id === uniteChoisie)
    ?? article?.unites?.[0];
  const facteurChoisi = uniteSel?.facteur ?? 1;
  const libelleChoisi = uniteSel?.libelle ?? article?.unite_base ?? "";
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
        // Le stock est tenu en unite de base : on convertit avant
        // d'envoyer, sinon 10 sacs deviendraient 10 kilos.
        quantite: parseFloat(quantite) * facteurChoisi,
        prixAchat: prixAchat
          ? Math.round(parseInt(prixAchat) / facteurChoisi)
          : null,
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
          {/* Le stock est TOUJOURS tenu en unite de base. Mais on
              recoit des sacs, pas des kilos : on saisit dans le
              conditionnement reel et on convertit. */}
          <div>
            <Label>Quantité reçue *</Label>
            <div className="flex gap-2 mt-1">
              <Input type="number" value={quantite}
                onChange={e => setQuantite(e.target.value)}
                placeholder="Ex: 50" autoFocus className="flex-1" />
              {(article?.unites?.length ?? 0) > 1 ? (
                <select value={uniteChoisie}
                  onChange={e => setUniteChoisie(e.target.value)}
                  className="h-9 px-2 text-sm border border-border
                             rounded-md bg-background w-32">
                  {article!.unites!.map(u => (
                    <option key={u.id} value={u.id}>
                      {u.libelle}{u.facteur !== 1 && ` (×${u.facteur})`}
                    </option>
                  ))}
                </select>
              ) : (
                <span className="flex items-center px-3 text-sm
                                 text-muted-foreground">
                  {article?.unite_base}
                </span>
              )}
            </div>
            {quantite && article && (
              <p className="text-xs text-green-600 mt-1">
                Soit {formaterQuantite(
                  parseFloat(quantite) * facteurChoisi, article.unite_base
                )} · stock après : {formaterQuantite(
                  article.quantite + parseFloat(quantite) * facteurChoisi,
                  article.unite_base
                )}
              </p>
            )}
          </div>
          <div>
            <Label>Prix d'achat (F/{libelleChoisi})
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

  // L'etat du stock est un document qu'on emporte pour l'inventaire :
  // on coche a la main dans la colonne « Compté », puis on ajuste.
  async function imprimerEtat() {
    try {
      const d = await invoke<any>("lire_etat_stock", {
        depotId: DEPOT_ACTIF, avecZero: false,
      });
      await invoke("imprimer_facture", {
        html: genererEtatStockHTML(d),
        nomFichier: `etat_stock_${new Date().toISOString().slice(0,10)}.html`,
      });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    }
  }

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Stock</h1>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => charger(page)}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
          <Button variant="outline" size="sm" onClick={imprimerEtat}>
            <Printer className="h-4 w-4 mr-2" /> État du stock
          </Button>
        </div>
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

// =====================================================================
//  État du stock imprimable
// =====================================================================
//
//  Document d'inventaire : une colonne « Compté » vide, à remplir à la
//  main dans les rayons. C'est le seul usage réel de cette impression —
//  on ne consulte pas un stock sur papier, on le vérifie.

function genererEtatStockHTML(d: any): string {
  const fmt = (n: number) =>
    new Intl.NumberFormat("fr-ML").format(n) + " F";
  const fmtQ = (n: number) => (n % 1 === 0 ? String(n) : n.toFixed(2));

  let categorieCourante = "";
  const lignes = d.lignes.map((l: any) => {
    let entete = "";
    if (l.categorie !== categorieCourante) {
      categorieCourante = l.categorie;
      entete = `<tr><td colspan="6" style="background:#eee;padding:5px 8px;
        font-weight:bold;font-size:11px">${l.categorie}</td></tr>`;
    }
    return entete + `
      <tr style="border-bottom:1px solid #eee">
        <td style="padding:5px 8px">${l.article}</td>
        <td style="padding:5px 8px;font-size:10px;color:#666">${l.depot}</td>
        <td style="padding:5px 8px;text-align:right">${fmtQ(l.quantite)}</td>
        <td style="padding:5px 8px;font-size:10px;color:#666">${l.unite}</td>
        <td style="padding:5px 8px;text-align:right">${fmt(l.valeur)}</td>
        <td style="padding:5px 8px;width:70px;border-left:1px solid #ccc"></td>
      </tr>`;
  }).join("");

  const maintenant = new Date();
  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8"><title>État du stock</title>
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  body{font-family:Arial,sans-serif;font-size:12px;padding:12mm}
  table{width:100%;border-collapse:collapse}
  @media print{@page{size:A4;margin:8mm}}
</style></head><body>

<div style="display:flex;justify-content:space-between;margin-bottom:14px">
  <div>
    <div style="font-size:16px;font-weight:bold">${d.societe.nom}</div>
    ${d.societe.adresse ? `<div style="font-size:10px;color:#555">${d.societe.adresse}</div>` : ""}
  </div>
  <div style="text-align:right">
    <div style="font-size:15px;font-weight:bold">ÉTAT DU STOCK</div>
    <div style="font-size:10px;color:#555">
      ${maintenant.toLocaleDateString("fr-ML")} à
      ${maintenant.toLocaleTimeString("fr-ML", { hour: "2-digit", minute: "2-digit" })}
    </div>
    <div style="font-size:10px;color:#555">${d.nb_articles} référence(s)</div>
  </div>
</div>

<table>
  <thead>
    <tr style="background:#f0f0f0;border-bottom:2px solid #000">
      <th style="text-align:left;padding:6px 8px">Article</th>
      <th style="text-align:left;padding:6px 8px">Dépôt</th>
      <th style="text-align:right;padding:6px 8px">Théorique</th>
      <th style="text-align:left;padding:6px 8px">Unité</th>
      <th style="text-align:right;padding:6px 8px">Valeur</th>
      <th style="text-align:center;padding:6px 8px;border-left:1px solid #ccc">Compté</th>
    </tr>
  </thead>
  <tbody>${lignes}</tbody>
  <tfoot>
    <tr style="border-top:2px solid #000;background:#f5f5f5">
      <td colspan="4" style="padding:7px 8px;font-weight:bold">
        VALEUR TOTALE DU STOCK
      </td>
      <td style="padding:7px 8px;text-align:right;font-weight:bold">
        ${fmt(d.valeur_totale)}
      </td>
      <td style="border-left:1px solid #ccc"></td>
    </tr>
  </tfoot>
</table>

<div style="font-size:10px;color:#777;margin-top:12px">
  Valeur calculée au dernier prix d'achat connu. La colonne « Compté »
  est à remplir lors de l'inventaire, puis à saisir dans l'application.
</div>

<div style="display:flex;justify-content:space-between;margin-top:36px">
  <div style="width:45%;border-top:1px solid #000;padding-top:6px;text-align:center">
    Compté par
  </div>
  <div style="width:45%;border-top:1px solid #000;padding-top:6px;text-align:center">
    Vérifié par
  </div>
</div>

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}
