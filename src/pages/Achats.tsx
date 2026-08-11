import { useState, useRef, useEffect } from "react";
import {
  Plus, Trash2, ShoppingBag, User, Search,
  Loader2, Warehouse, AlertTriangle, Truck
} from "lucide-react";
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
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";

// =====================================================================
//  Types
// =====================================================================

interface Fournisseur {
  id: string;
  nom: string;
  telephone?: string;
}

interface UniteVente {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

interface ArticleAchat {
  id: string;
  nom: string;
  unite_base: string;
  stock: number;
  unites: UniteVente[];
}

interface LignePanierAchat {
  id: string;
  article: ArticleAchat;
  unite: UniteVente;
  quantite: number;
  prix_achat: number; // prix d'achat unitaire (unité de base)
  montant: number;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function genId(): string {
  return Math.random().toString(36).slice(2);
}

// =====================================================================
//  Modal : Nouveau fournisseur rapide
// =====================================================================

function ModalNouveauFournisseur({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: (f: Fournisseur) => void;
}) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      const id = await invoke<string>("creer_fournisseur", {
        nom: nom.trim(),
        telephone: telephone.trim() || null,
        nif: null,
        adresse: null,
      });
      onCreer({ id, nom: nom.trim(), telephone: telephone.trim() || undefined });
      setNom(""); setTelephone("");
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouveau fournisseur</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du fournisseur" autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" />
          </div>
          <div className="flex gap-2 pt-1">
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
//  Modal : Confirmation achat
// =====================================================================

function ModalConfirmationAchat({
  ouvert, total, modeReglement, fournisseur,
  chargement, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  total: number;
  modeReglement: "comptant" | "credit";
  fournisseur: Fournisseur | null;
  chargement: boolean;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Confirmation d'achat</DialogTitle></DialogHeader>
        <div className="space-y-4 pt-2">

          {/* Fournisseur */}
          <div className="flex items-center gap-2 pb-2 border-b border-border">
            <Truck className="h-4 w-4 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">{fournisseur?.nom ?? "Fournisseur secondaire"}</p>
              {fournisseur?.telephone && (
                <p className="text-xs text-muted-foreground">{fournisseur.telephone}</p>
              )}
            </div>
          </div>

          {/* Résumé financier */}
          <div className="space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Total achat</span>
              <span className="font-semibold">{formaterMontant(total)}</span>
            </div>

            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Mode</span>
              <Badge variant={modeReglement === "comptant" ? "default" : "outline"}>
                {modeReglement === "comptant" ? "Comptant" : "À crédit"}
              </Badge>
            </div>

            {/* Note importante : pas d'impact caisse */}
            <div className="bg-muted rounded-md px-3 py-2 text-xs text-muted-foreground mt-2">
              ℹ️ Cet achat met à jour le stock uniquement.
              Le paiement fournisseur se gère séparément.
            </div>
          </div>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} disabled={chargement} className="flex-1">
              Annuler
            </Button>
            <Button onClick={onConfirmer} disabled={chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirmer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page principale : Achats
// =====================================================================

export function Achats() {
  // Données depuis Tauri
  const [tousFournisseurs, setTousFournisseurs] = useState<Fournisseur[]>([]);
  const [tousArticles, setTousArticles] = useState<ArticleAchat[]>([]);
  const [chargementInitial, setChargementInitial] = useState(true);

  // Fournisseur
  const [fournisseur, setFournisseur] = useState<Fournisseur | null>(null);
  const [rechercheFournisseur, setRechercheFournisseur] = useState("");
  const [fournisseursFiltres, setFournisseursFiltres] = useState<Fournisseur[]>([]);
  const [modalNouveauFournisseur, setModalNouveauFournisseur] = useState(false);

  // Article
  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<ArticleAchat[]>([]);
  const [articleSelectionne, setArticleSelectionne] = useState<ArticleAchat | null>(null);
  const [uniteSelectionnee, setUniteSelectionnee] = useState<UniteVente | null>(null);
  const [quantite, setQuantite] = useState("1");
  const [prixAchat, setPrixAchat] = useState("");

  // Panier
  const [panier, setPanier] = useState<LignePanierAchat[]>([]);
  const [modeReglement, setModeReglement] = useState<"comptant" | "credit">("comptant");

  // Modal
  const [modalConfirmation, setModalConfirmation] = useState(false);
  const [chargementAchat, setChargementAchat] = useState(false);

  const inputArticleRef = useRef<HTMLInputElement>(null);
  const total = panier.reduce((sum, l) => sum + l.montant, 0);

  // ---- Chargement initial ----
  useEffect(() => {
    async function charger() {
      try {
        const [fournisseurs, articles] = await Promise.all([
          invoke<Fournisseur[]>("lire_fournisseurs"),
          invoke<ArticleAchat[]>("lire_articles_avec_unites"),
        ]);
        setTousFournisseurs(fournisseurs);
        setTousArticles(articles);
      } catch (e) {
        console.error("Erreur chargement achats :", e);
      } finally {
        setChargementInitial(false);
      }
    }
    charger();
  }, []);

  if (chargementInitial) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // ---- Fournisseur ----
  function handleRechercheFournisseur(val: string) {
    setRechercheFournisseur(val);
    if (val.length < 1) { setFournisseursFiltres([]); return; }
    setFournisseursFiltres(
      tousFournisseurs.filter(f =>
        f.nom.toLowerCase().includes(val.toLowerCase()) ||
        (f.telephone && f.telephone.includes(val))
      )
    );
  }

  function selectionnerFournisseur(f: Fournisseur) {
    setFournisseur(f);
    setRechercheFournisseur("");
    setFournisseursFiltres([]);
    inputArticleRef.current?.focus();
  }

  // ---- Article ----
  function handleRechercheArticle(val: string) {
    setRechercheArticle(val);
    if (val.length < 1) { setArticlesFiltres([]); return; }
    setArticlesFiltres(
      tousArticles.filter(a => a.nom.toLowerCase().includes(val.toLowerCase()))
    );
  }

  function selectionnerArticle(article: ArticleAchat) {
    setArticleSelectionne(article);
    setUniteSelectionnee(article.unites[0]);
    setPrixAchat(""); // prix d'achat vide — à saisir
    setQuantite("1");
    setRechercheArticle("");
    setArticlesFiltres([]);
  }

  function handleUniteChange(uniteId: string) {
    const unite = articleSelectionne?.unites.find(u => u.id === uniteId);
    if (unite) {
      setUniteSelectionnee(unite);
      setPrixAchat("");
    }
  }

  // ---- Panier ----
  function ajouterAuPanier() {
    if (!articleSelectionne || !uniteSelectionnee || !quantite || !prixAchat) return;
    const qte = parseFloat(quantite) || 1;
    const prix = parseInt(prixAchat) || 0;

    setPanier(prev => [...prev, {
      id: genId(),
      article: articleSelectionne,
      unite: uniteSelectionnee,
      quantite: qte,
      prix_achat: prix,
      montant: Math.round(prix * qte),
    }]);

    setArticleSelectionne(null);
    setUniteSelectionnee(null);
    setQuantite("1");
    setPrixAchat("");
    inputArticleRef.current?.focus();
  }

  function supprimerDuPanier(id: string) {
    setPanier(prev => prev.filter(l => l.id !== id));
  }

  function viderPanier() {
    setPanier([]);
    setFournisseur(null);
    setModeReglement("comptant");
  }

  // ---- Confirmation achat ----
  async function handleConfirmerAchat() {
    setChargementAchat(true);
    try {
      // Enregistrer chaque ligne comme entrée de stock.
      for (const ligne of panier) {
        await invoke("enregistrer_entree_stock", {
          articleId: ligne.article.id,
          depotId: null, // utilise le dépôt par défaut côté Rust
          quantite: ligne.quantite * ligne.unite.facteur, // en unité de base
          prixAchat: ligne.prix_achat,
          fournisseurId: fournisseur?.id ?? null,
        });
      }

      // Recharger les articles pour stock à jour.
      const articles = await invoke<ArticleAchat[]>("lire_articles_avec_unites");
      setTousArticles(articles);

      setModalConfirmation(false);
      viderPanier();
      await message("Achat enregistré ✓ — stock mis à jour", {
        title: "Succès", kind: "info",
      });

    } catch (e) {
      setModalConfirmation(false);
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargementAchat(false);
    }
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* En-tête */}
      <div className="flex items-center justify-between px-4 h-12 border-b border-border bg-card shrink-0">
        <h1 className="font-semibold text-sm">Nouvel achat</h1>
        <div className="flex items-center gap-2">
          <Button size="sm"
            variant={modeReglement === "comptant" ? "default" : "outline"}
            onClick={() => setModeReglement("comptant")}>
            Comptant
          </Button>
          <Button size="sm"
            variant={modeReglement === "credit" ? "default" : "outline"}
            onClick={() => setModeReglement("credit")}>
            À crédit
          </Button>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">

        {/* ── Colonne gauche ── */}
        <div className="w-1/2 flex flex-col p-4 gap-4 border-r border-border overflow-auto">

          {/* Fournisseur */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <Label className="flex items-center gap-1">
                <Truck className="h-3 w-3" /> Fournisseur
              </Label>
              <button onClick={() => setModalNouveauFournisseur(true)}
                className="text-xs text-primary hover:underline flex items-center gap-0.5">
                <Plus className="h-3 w-3" /> Nouveau
              </button>
            </div>

            {fournisseur ? (
              <div className="flex items-center gap-2 mb-1">
                <Badge variant="secondary" className="text-xs">{fournisseur.nom}</Badge>
                <button onClick={() => setFournisseur(null)}
                  className="text-xs text-muted-foreground hover:text-foreground">✕</button>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground mb-1">
                Aucun fournisseur sélectionné
              </p>
            )}

            <div className="relative">
              <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
              <Input value={rechercheFournisseur}
                onChange={e => handleRechercheFournisseur(e.target.value)}
                placeholder="Rechercher un fournisseur..."
                className="pl-8 h-8 text-sm" />
              {fournisseursFiltres.length > 0 && (
                <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md">
                  {fournisseursFiltres.map(f => (
                    <button key={f.id} onClick={() => selectionnerFournisseur(f)}
                      className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent transition-colors">
                      <span className="font-medium">{f.nom}</span>
                      {f.telephone && (
                        <span className="text-muted-foreground ml-2 text-xs">{f.telephone}</span>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Article */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <Label className="flex items-center gap-1">
                <ShoppingBag className="h-3 w-3" /> Article reçu
              </Label>
            </div>

            <div className="relative">
              <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
              <Input ref={inputArticleRef} value={rechercheArticle}
                onChange={e => handleRechercheArticle(e.target.value)}
                placeholder="Rechercher un article..."
                className="pl-8 h-8 text-sm" />
              {articlesFiltres.length > 0 && (
                <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md">
                  {articlesFiltres.map(a => (
                    <button key={a.id} onClick={() => selectionnerArticle(a)}
                      className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent transition-colors">
                      <div className="flex items-center justify-between">
                        <span className="font-medium">{a.nom}</span>
                        <span className="text-xs text-muted-foreground">
                          Stock : {a.stock % 1 === 0 ? a.stock : a.stock.toFixed(2)} {a.unite_base}
                        </span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Formulaire article sélectionné */}
          {articleSelectionne && (
            <div className="border border-border rounded-lg p-3 bg-muted/30 space-y-3">
              <div className="flex items-center justify-between">
                <p className="font-medium text-sm">{articleSelectionne.nom}</p>
                <Badge variant="secondary" className="text-xs">
                  Stock actuel : {articleSelectionne.stock % 1 === 0
                    ? articleSelectionne.stock
                    : articleSelectionne.stock.toFixed(2)
                  } {articleSelectionne.unite_base}
                </Badge>
              </div>

              {articleSelectionne.unites.length > 1 && (
                <div>
                  <Label className="text-xs">Unité reçue</Label>
                  <Select
                    value={uniteSelectionnee?.id ?? ""}
                    onValueChange={v => { if (v) handleUniteChange(v); }}
                  >
                    <SelectTrigger className="h-8 text-sm mt-0.5">
                      <SelectValue>
                        {uniteSelectionnee?.libelle ?? "Choisir"}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {articleSelectionne.unites.map(u => (
                        <SelectItem key={u.id} value={u.id}>
                          {u.libelle} (× {u.facteur} {articleSelectionne.unite_base})
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <Label className="text-xs">Quantité reçue</Label>
                  <Input type="number" value={quantite}
                    onChange={e => setQuantite(e.target.value)}
                    className="h-8 text-sm mt-0.5" autoFocus
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
                <div>
                  <Label className="text-xs">
                    Prix d'achat (F/{uniteSelectionnee?.libelle ?? articleSelectionne.unite_base}) *
                  </Label>
                  <Input type="number" value={prixAchat}
                    onChange={e => setPrixAchat(e.target.value)}
                    placeholder="Prix unitaire"
                    className="h-8 text-sm mt-0.5"
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
              </div>

              {/* Stock après entrée */}
              {quantite && uniteSelectionnee && (
                <p className="text-xs text-green-600">
                  Stock après : {(
                    articleSelectionne.stock +
                    parseFloat(quantite) * uniteSelectionnee.facteur
                  ).toFixed(articleSelectionne.unite_base === "kg" || articleSelectionne.unite_base === "litre" ? 2 : 0)} {articleSelectionne.unite_base}
                </p>
              )}

              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  Montant : {prixAchat && quantite
                    ? formaterMontant(Math.round(parseInt(prixAchat) * parseFloat(quantite)))
                    : "—"
                  }
                </span>
                <Button
                  size="sm"
                  onClick={ajouterAuPanier}
                  disabled={!quantite || !prixAchat}
                >
                  Ajouter →
                </Button>
              </div>
            </div>
          )}
        </div>

        {/* ── Colonne droite : panier achat ── */}
        <div className="w-1/2 flex flex-col p-4">

          {/* Fournisseur en haut */}
          <div className="flex items-center gap-2 pb-3 mb-3 border-b border-border">
            <div className="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <Truck className="h-3.5 w-3.5 text-primary" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-medium truncate">
                {fournisseur?.nom ?? "Fournisseur non sélectionné"}
              </p>
              {fournisseur?.telephone && (
                <p className="text-xs text-muted-foreground">{fournisseur.telephone}</p>
              )}
            </div>
            <Badge variant="outline" className="ml-auto text-xs shrink-0">
              {modeReglement === "comptant" ? "Comptant" : "À crédit"}
            </Badge>
          </div>

          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium">
              Réception ({panier.length} article{panier.length > 1 ? "s" : ""})
            </span>
            {panier.length > 0 && (
              <button onClick={viderPanier}
                className="text-xs text-muted-foreground hover:text-destructive transition-colors">
                Vider
              </button>
            )}
          </div>

          {/* Lignes */}
          <div className="flex-1 overflow-auto space-y-1 min-h-0">
            {panier.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
                <ShoppingBag className="h-8 w-8 mb-2 opacity-30" />
                <p className="text-sm">Aucun article ajouté</p>
              </div>
            ) : (
              panier.map(ligne => (
                <div key={ligne.id}
                  className="flex items-center justify-between py-2 px-3 rounded-md bg-muted/40 group">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium truncate">{ligne.article.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {ligne.quantite} {ligne.unite.libelle} ×{" "}
                      {formaterMontant(ligne.prix_achat)}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 ml-2">
                    <span className="text-sm font-semibold">{formaterMontant(ligne.montant)}</span>
                    <button onClick={() => supprimerDuPanier(ligne.id)}
                      className={cn(
                        "opacity-0 group-hover:opacity-100 transition-opacity",
                        "text-muted-foreground hover:text-destructive"
                      )}>
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>

          {/* Total + bouton */}
          <div className="border-t border-border pt-3 mt-3 space-y-2">
            <div className="flex items-center justify-between">
              <span className="font-medium">Total achat</span>
              <span className="text-xl font-bold">{formaterMontant(total)}</span>
            </div>

            {/* Rappel : pas d'impact caisse */}
            <p className="text-xs text-muted-foreground text-center">
              Le stock sera mis à jour — aucun impact sur la caisse
            </p>

            <Button className="w-full" size="lg"
              disabled={panier.length === 0 || chargementAchat}
              onClick={() => setModalConfirmation(true)}>
              {chargementAchat
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Enregistrer la réception →"
              }
            </Button>
          </div>
        </div>
      </div>

      {/* Modals */}
      <ModalNouveauFournisseur
        ouvert={modalNouveauFournisseur}
        onFermer={() => setModalNouveauFournisseur(false)}
        onCreer={f => {
          setTousFournisseurs(prev => [...prev, f]);
          setFournisseur(f);
          setModalNouveauFournisseur(false);
        }}
      />

      <ModalConfirmationAchat
        ouvert={modalConfirmation}
        total={total}
        modeReglement={modeReglement}
        fournisseur={fournisseur}
        chargement={chargementAchat}
        onFermer={() => setModalConfirmation(false)}
        onConfirmer={handleConfirmerAchat}
      />
    </div>
  );
}
