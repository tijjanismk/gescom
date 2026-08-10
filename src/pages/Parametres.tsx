import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Package, Plus, Pencil, ChevronDown, ChevronRight,
  Loader2, Tag, Layers
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
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

interface Categorie {
  id: string;
  nom: string;
  nb_articles: number;
}

interface UniteVente {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

interface ArticleComplet {
  id: string;
  nom: string;
  reference?: string;
  categorie_id: string;
  categorie_nom: string;
  unite_base: string;
  gere_en_stock: boolean;
  actif: boolean;
  unites: UniteVente[];
  stock_total: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Modal : Nouvelle catégorie
// =====================================================================

function ModalNouvelleCategorie({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: () => void;
}) {
  const [nom, setNom] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      await invoke("creer_categorie", { nom: nom.trim() });
      setNom("");
      onCreer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouvelle catégorie</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Ex: Alimentation" autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleCreer} disabled={!nom.trim() || chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Nouvel article complet
// =====================================================================

function ModalNouvelArticle({
  ouvert, categories, onFermer, onCreer,
}: {
  ouvert: boolean;
  categories: Categorie[];
  onFermer: () => void;
  onCreer: () => void;
}) {
  const [nom, setNom] = useState("");
  const [reference, setReference] = useState("");
  const [categorieId, setCategorieId] = useState("");
  const [uniteBase, setUniteBase] = useState("unite");
  const [prixRef, setPrixRef] = useState("");
  const [stockInitial, setStockInitial] = useState("0");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim() || !categorieId || !prixRef) return;
    setChargement(true);
    try {
      await invoke("creer_article_complet", {
        nom: nom.trim(),
        reference: reference.trim() || null,
        categorieId,
        uniteBase,
        prixReference: parseInt(prixRef),
        stockInitial: parseFloat(stockInitial) || 0,
      });
      setNom(""); setReference(""); setCategorieId("");
      setUniteBase("unite"); setPrixRef(""); setStockInitial("0");
      onCreer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-md">
        <DialogHeader><DialogTitle>Nouvel article</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div className="grid grid-cols-2 gap-3">
            <div className="col-span-2">
              <Label>Nom *</Label>
              <Input value={nom} onChange={e => setNom(e.target.value)}
                placeholder="Nom de l'article" autoFocus />
            </div>
            <div>
              <Label>Référence / Code-barres</Label>
              <Input value={reference} onChange={e => setReference(e.target.value)}
                placeholder="Optionnel" />
            </div>
            <div>
              <Label>Catégorie *</Label>
              <Select value={categorieId}
                onValueChange={v => { if (v) setCategorieId(v); }}>
                <SelectTrigger>
                  <SelectValue placeholder="Choisir">
                    {categories.find(c => c.id === categorieId)?.nom ?? "Choisir"}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {categories.map(c => (
                    <SelectItem key={c.id} value={c.id}>{c.nom}</SelectItem>
                  ))}
                </SelectContent>
            </Select>
            </div>
            <div>
              <Label>Unité de base *</Label>
              <Select value={uniteBase}
                onValueChange={v => { if (v) setUniteBase(v); }}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="unite">Unité</SelectItem>
                  <SelectItem value="kg">Kg</SelectItem>
                  <SelectItem value="litre">Litre</SelectItem>
                  <SelectItem value="metre">Mètre</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label>Prix de vente (F) *</Label>
              <Input type="number" value={prixRef}
                onChange={e => setPrixRef(e.target.value)}
                placeholder="800" />
            </div>
            <div className="col-span-2">
              <Label>Stock initial ({uniteBase})</Label>
              <Input type="number" value={stockInitial}
                onChange={e => setStockInitial(e.target.value)}
                placeholder="0" />
            </div>
          </div>
          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={handleCreer}
              disabled={!nom.trim() || !categorieId || !prixRef || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Ajouter unité de vente
// =====================================================================

function ModalNouvelleUnite({
  ouvert, article, onFermer, onCreer,
}: {
  ouvert: boolean;
  article: ArticleComplet | null;
  onFermer: () => void;
  onCreer: () => void;
}) {
  const [libelle, setLibelle] = useState("");
  const [facteur, setFacteur] = useState("");
  const [prixRef, setPrixRef] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!article || !libelle.trim() || !facteur || !prixRef) return;
    setChargement(true);
    try {
      await invoke("creer_unite_vente", {
        articleId: article.id,
        libelle: libelle.trim(),
        facteur: parseFloat(facteur),
        prixReference: parseInt(prixRef),
      });
      setLibelle(""); setFacteur(""); setPrixRef("");
      onCreer();
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
            Nouvelle unité — {article?.nom}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Libellé *</Label>
            <Input value={libelle} onChange={e => setLibelle(e.target.value)}
              placeholder="Ex: carton 24" autoFocus />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label>Facteur * <span className="text-xs text-muted-foreground">
                (en {article?.unite_base})
              </span></Label>
              <Input type="number" value={facteur}
                onChange={e => setFacteur(e.target.value)}
                placeholder="Ex: 24" />
            </div>
            <div>
              <Label>Prix de vente (F) *</Label>
              <Input type="number" value={prixRef}
                onChange={e => setPrixRef(e.target.value)}
                placeholder="Ex: 10800" />
            </div>
          </div>
          <p className="text-xs text-muted-foreground">
            {facteur && prixRef
              ? `Équivaut à ${formaterMontant(Math.round(parseInt(prixRef) / parseFloat(facteur)))}
                 par ${article?.unite_base}`
              : "Saisissez le facteur et le prix pour voir le prix unitaire"
            }
          </p>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={handleCreer}
              disabled={!libelle.trim() || !facteur || !prixRef || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Ajouter"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Ligne article expandable
// =====================================================================

function LigneArticle({
  article,
  onAjouterUnite,
}: {
  article: ArticleComplet;
  onAjouterUnite: (a: ArticleComplet) => void;
}) {
  const [ouvert, setOuvert] = useState(false);

  return (
    <div className="border border-border rounded-md overflow-hidden">
      {/* En-tête de l'article */}
      <button
        onClick={() => setOuvert(!ouvert)}
        className="w-full flex items-center gap-3 px-4 py-3 hover:bg-muted/40 transition-colors text-left">
        {ouvert
          ? <ChevronDown className="h-4 w-4 text-muted-foreground shrink-0" />
          : <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" />
        }
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-sm font-medium">{article.nom}</p>
            {article.reference && (
              <span className="text-xs text-muted-foreground">#{article.reference}</span>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            {article.categorie_nom} · {article.unite_base}
          </p>
        </div>
        <div className="flex items-center gap-3 shrink-0">
          <Badge variant={article.stock_total < 0 ? "destructive" : "secondary"}
            className="text-xs">
            {article.stock_total % 1 === 0
              ? article.stock_total
              : article.stock_total.toFixed(2)
            } {article.unite_base}
          </Badge>
          <span className="text-sm font-medium">
            {formaterMontant(article.unites[0]?.prix_reference ?? 0)}
          </span>
        </div>
      </button>

      {/* Détail des unités */}
      {ouvert && (
        <div className="border-t border-border bg-muted/20 px-4 py-3">
          <div className="flex items-center justify-between mb-2">
            <p className="text-xs font-medium text-muted-foreground">
              Unités de vente
            </p>
            <button
              onClick={() => onAjouterUnite(article)}
              className="text-xs text-primary hover:underline flex items-center gap-0.5">
              <Plus className="h-3 w-3" /> Ajouter
            </button>
          </div>
          <div className="space-y-1">
            {article.unites.map(u => (
              <div key={u.id}
                className="flex items-center justify-between text-sm py-1">
                <span className="text-muted-foreground">{u.libelle}</span>
                <div className="flex items-center gap-4">
                  <span className="text-xs text-muted-foreground">
                    × {u.facteur} {article.unite_base}
                  </span>
                  <span className="font-medium">{formaterMontant(u.prix_reference)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// =====================================================================
//  Page Paramètres
// =====================================================================

export function Parametres() {
  const [onglet, setOnglet] = useState<"articles" | "categories">("articles");
  const [articles, setArticles] = useState<ArticleComplet[]>([]);
  const [categories, setCategories] = useState<Categorie[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");

  const [modalCategorie, setModalCategorie] = useState(false);
  const [modalArticle, setModalArticle] = useState(false);
  const [modalUnite, setModalUnite] = useState(false);
  const [articlePourUnite, setArticlePourUnite] = useState<ArticleComplet | null>(null);

  async function charger() {
    setChargement(true);
    try {
      const [arts, cats] = await Promise.all([
        invoke<ArticleComplet[]>("lire_articles_complets"),
        invoke<Categorie[]>("lire_categories"),
      ]);
      setArticles(arts);
      setCategories(cats);
    } catch (e) {
      console.error("Erreur chargement paramètres :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  const articlesFiltres = articles.filter(a =>
    a.nom.toLowerCase().includes(recherche.toLowerCase()) ||
    (a.reference && a.reference.toLowerCase().includes(recherche.toLowerCase()))
  );

  function handleAjouterUnite(article: ArticleComplet) {
    setArticlePourUnite(article);
    setModalUnite(true);
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
        <h1 className="text-2xl font-semibold">Paramètres</h1>
        <div className="flex gap-2">
          {onglet === "articles" && (
            <Button size="sm" onClick={() => setModalArticle(true)}>
              <Plus className="h-4 w-4 mr-1" /> Article
            </Button>
          )}
          {onglet === "categories" && (
            <Button size="sm" onClick={() => setModalCategorie(true)}>
              <Plus className="h-4 w-4 mr-1" /> Catégorie
            </Button>
          )}
        </div>
      </div>

      {/* Onglets */}
      <div className="flex gap-1 mb-6 border-b border-border">
        <button
          onClick={() => setOnglet("articles")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            onglet === "articles"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}>
          <Package className="h-4 w-4" />
          Articles ({articles.length})
        </button>
        <button
          onClick={() => setOnglet("categories")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            onglet === "categories"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}>
          <Tag className="h-4 w-4" />
          Catégories ({categories.length})
        </button>
      </div>

      {/* Articles */}
      {onglet === "articles" && (
        <>
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
            placeholder="Rechercher un article..."
            className="mb-4 max-w-sm" />

          <div className="space-y-2">
            {articlesFiltres.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-12">
                Aucun article
              </p>
            ) : (
              articlesFiltres.map(a => (
                <LigneArticle
                  key={a.id}
                  article={a}
                  onAjouterUnite={handleAjouterUnite}
                />
              ))
            )}
          </div>
        </>
      )}

      {/* Catégories */}
      {onglet === "categories" && (
        <div className="space-y-2">
          {categories.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-12">
              Aucune catégorie
            </p>
          ) : (
            categories.map(c => (
              <div key={c.id}
                className="flex items-center justify-between px-4 py-3 border border-border rounded-md">
                <div className="flex items-center gap-3">
                  <Layers className="h-4 w-4 text-muted-foreground" />
                  <p className="text-sm font-medium">{c.nom}</p>
                </div>
                <Badge variant="secondary" className="text-xs">
                  {c.nb_articles} article{c.nb_articles > 1 ? "s" : ""}
                </Badge>
              </div>
            ))
          )}
        </div>
      )}

      {/* Modals */}
      <ModalNouvelleCategorie
        ouvert={modalCategorie}
        onFermer={() => setModalCategorie(false)}
        onCreer={() => { setModalCategorie(false); charger(); }}
      />

      <ModalNouvelArticle
        ouvert={modalArticle}
        categories={categories}
        onFermer={() => setModalArticle(false)}
        onCreer={() => { setModalArticle(false); charger(); }}
      />

      <ModalNouvelleUnite
        ouvert={modalUnite}
        article={articlePourUnite}
        onFermer={() => setModalUnite(false)}
        onCreer={() => { setModalUnite(false); charger(); }}
      />
    </div>
  );
}
