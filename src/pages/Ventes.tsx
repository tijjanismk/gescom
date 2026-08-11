import { useState, useRef, useEffect } from "react";
import {
  Plus, Trash2, ShoppingCart, User, Search,
  Loader2, Warehouse, AlertTriangle
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
import {
  Client, Article, UniteVente, Depot,
  lireClients, lireClientGenerique, lireDepotDefaut,
  lireArticlesAvecUnites, creerClientRapide, creerArticleRapide,
  creerVente, enregistrerPaiement,
} from "@/lib/tauri";
import { invoke } from "@tauri-apps/api/core";

// =====================================================================
//  Types locaux
// =====================================================================

interface ArticleAvecStock extends Article {
  stock: number; // en unité de base
}

interface DepotOption {
  id: string;
  nom: string;
  est_defaut: boolean;
}

interface LignePanier {
  id: string;
  article: ArticleAvecStock;
  unite: UniteVente;
  quantite: number;
  prix_pratique: number;
  montant: number;
  a_decouvert: boolean;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function formaterStock(stock: number, unite: string): string {
  const q = stock % 1 === 0 ? stock.toString() : stock.toFixed(2);
  return `${q} ${unite}`;
}

function genId(): string {
  return Math.random().toString(36).slice(2);
}

// Stock disponible en unité de vente (stock_base / facteur)
function stockEnUniteVente(stockBase: number, facteur: number): number {
  return stockBase / facteur;
}

// =====================================================================
//  Modal : Nouveau client
// =====================================================================

function ModalNouveauClient({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: (c: Client) => void;
}) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      const client = await creerClientRapide(nom.trim(), telephone.trim() || undefined);
      onCreer(client);
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
        <DialogHeader><DialogTitle>Nouveau client</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du client" autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00"
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
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
//  Modal : Nouvel article
// =====================================================================

function ModalNouvelArticle({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: (a: ArticleAvecStock) => void;
}) {
  const [nom, setNom] = useState("");
  const [uniteBase, setUniteBase] = useState("unite");
  const [prix, setPrix] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim() || !prix) return;
    setChargement(true);
    try {
      const article = await creerArticleRapide(nom.trim(), uniteBase, parseInt(prix));
      onCreer({ ...article, stock: 0 });
      setNom(""); setUniteBase("unite"); setPrix("");
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouvel article</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom de l'article" autoFocus />
          </div>
          <div>
            <Label>Unité de base *</Label>
            <Select value={uniteBase} onValueChange={v => { if (v) setUniteBase(v); }}>
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
            <Input type="number" value={prix} onChange={e => setPrix(e.target.value)}
              placeholder="800" />
          </div>
          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleCreer} disabled={!nom.trim() || !prix || chargement}
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
//  Modal : Encaissement comptant
// =====================================================================

function ModalEncaissement({
  ouvert, total, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  total: number;
  onFermer: () => void;
  onConfirmer: (mode: string, montant: number) => void;
}) {
  const [mode, setMode] = useState("especes");
  const [montant, setMontant] = useState(total.toString());
  const rendu = parseInt(montant || "0") - total;

  useEffect(() => { setMontant(total.toString()); }, [total]);

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Encaissement</DialogTitle></DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-lg p-4 text-center">
            <p className="text-sm text-muted-foreground">Total à payer</p>
            <p className="text-3xl font-bold mt-1">{formaterMontant(total)}</p>
          </div>
          <div>
            <Label>Mode de paiement</Label>
            <Select value={mode} onValueChange={v => { if (v) setMode(v); }}>
              <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="especes">Espèces</SelectItem>
                <SelectItem value="orange_money">Orange Money</SelectItem>
                <SelectItem value="moov_money">Moov Money</SelectItem>
                <SelectItem value="cheque">Chèque</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Montant reçu uniquement pour espèces */}
          {mode === "especes" && (
            <div>
              <Label>Montant reçu (F)</Label>
              <Input type="number" value={montant}
                onChange={e => setMontant(e.target.value)}
                className="mt-1 text-lg font-medium" autoFocus />
              {rendu > 0 && (
                <p className="text-sm text-green-600 mt-1 font-medium">
                  Rendu : {formaterMontant(rendu)}
                </p>
              )}
              {parseInt(montant) < total && (
                <p className="text-sm text-red-500 mt-1">
                  Montant insuffisant
                </p>
              )}
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={() => onConfirmer(mode, mode === "especes" ? parseInt(montant) || total : total)}
              disabled={mode === "especes" && parseInt(montant || "0") < total}
              className="flex-1">
              Confirmer
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Confirmation finale
// =====================================================================

function ModalConfirmation({
  ouvert, total, acompte, modeAcompte, modeReglement, client,
  chargement, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  total: number;
  acompte: number;
  modeAcompte: string;
  modeReglement: "comptant" | "credit";
  client: Client;
  chargement: boolean;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const resteCreance = total - acompte;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Confirmation de vente</DialogTitle></DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="flex items-center gap-2 pb-2 border-b border-border">
            <User className="h-4 w-4 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">{client.nom}</p>
              <p className="text-xs text-muted-foreground">{client.code}</p>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Total</span>
              <span className="font-semibold">{formaterMontant(total)}</span>
            </div>

            {modeReglement === "comptant" && (
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Mode</span>
                <span className="capitalize">{modeAcompte.replace(/_/g, " ")}</span>
              </div>
            )}

            {modeReglement === "credit" && acompte > 0 && (
              <>
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">
                    Acompte ({modeAcompte.replace(/_/g, " ")})
                  </span>
                  <span className="text-green-600 font-medium">{formaterMontant(acompte)}</span>
                </div>
                <div className="flex justify-between text-sm border-t border-border pt-2">
                  <span className="text-muted-foreground">Reste en créance</span>
                  <span className="text-orange-500 font-semibold">
                    {formaterMontant(resteCreance)}
                  </span>
                </div>
              </>
            )}

            {modeReglement === "credit" && acompte === 0 && (
              <div className="flex justify-between text-sm border-t border-border pt-2">
                <span className="text-muted-foreground">Créance ouverte</span>
                <span className="text-orange-500 font-semibold">{formaterMontant(total)}</span>
              </div>
            )}
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
//  Page principale : Ventes (POS)
// =====================================================================

export function Ventes() {
  // Données depuis Tauri
  const [clientGenerique, setClientGenerique] = useState<Client | null>(null);
  const [tousClients, setTousClients] = useState<Client[]>([]);
  const [tousArticles, setTousArticles] = useState<ArticleAvecStock[]>([]);
  const [depots, setDepots] = useState<DepotOption[]>([]);
  const [depotActif, setDepotActif] = useState<Depot | null>(null);
  const [chargementInitial, setChargementInitial] = useState(true);

  // Client
  const [client, setClient] = useState<Client | null>(null);
  const [rechercheClient, setRechercheClient] = useState("");
  const [clientsFiltres, setClientsFiltres] = useState<Client[]>([]);
  const [modalNouveauClient, setModalNouveauClient] = useState(false);

  // Article
  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<ArticleAvecStock[]>([]);
  const [articleSelectionne, setArticleSelectionne] = useState<ArticleAvecStock | null>(null);
  const [uniteSelectionnee, setUniteSelectionnee] = useState<UniteVente | null>(null);
  const [quantite, setQuantite] = useState("1");
  const [prixPratique, setPrixPratique] = useState("");
  const [modalNouvelArticle, setModalNouvelArticle] = useState(false);

  // Panier
  const [panier, setPanier] = useState<LignePanier[]>([]);
  const [modeReglement, setModeReglement] = useState<"comptant" | "credit">("comptant");
  const [acompte, setAcompte] = useState("");
  const [modeAcompte, setModeAcompte] = useState("especes");

  // Modals
  const [modalEncaissement, setModalEncaissement] = useState(false);
  const [modalConfirmation, setModalConfirmation] = useState(false);
  const [modePaiementComptant, setModePaiementComptant] = useState("especes");
  const [chargementVente, setChargementVente] = useState(false);

  const inputArticleRef = useRef<HTMLInputElement>(null);

  const total = panier.reduce((sum, l) => sum + l.montant, 0);
  const acompteNum = parseInt(acompte || "0");

  // ---- Chargement initial ----
  useEffect(() => {
    async function charger() {
      try {
        const [gen, clients, articles, dep] = await Promise.all([
          lireClientGenerique(),
          lireClients(),
          lireArticlesAvecUnites() as Promise<ArticleAvecStock[]>,
          lireDepotDefaut(),
        ]);

        // Charger aussi les dépôts disponibles
        const tousDepots = await invoke<DepotOption[]>("lire_depots");

        setClientGenerique(gen);
        setClient(gen);
        setTousClients(clients);
        setTousArticles(articles);
        setDepots(tousDepots);
        setDepotActif(dep);
      } catch (e) {
        console.error("Erreur chargement initial :", e);
      } finally {
        setChargementInitial(false);
      }
    }
    charger();
  }, []);

  // Recharger les articles si le dépôt change
  async function changerDepot(depotId: string) {
    const dep = depots.find(d => d.id === depotId);
    if (!dep) return;
    setDepotActif({ id: dep.id, nom: dep.nom });
    try {
      const articles = await lireArticlesAvecUnites() as ArticleAvecStock[];
      setTousArticles(articles);
    } catch (e) {
      console.error("Erreur rechargement articles :", e);
    }
  }

  if (chargementInitial) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // ---- Client ----
  function handleRechercheClient(val: string) {
    setRechercheClient(val);
    if (val.length < 1) { setClientsFiltres([]); return; }
    setClientsFiltres(
      tousClients.filter(c =>
        c.nom.toLowerCase().includes(val.toLowerCase()) ||
        (c.telephone && c.telephone.includes(val))
      )
    );
  }

  function selectionnerClient(c: Client) {
    setClient(c);
    setRechercheClient("");
    setClientsFiltres([]);
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

  function selectionnerArticle(article: ArticleAvecStock) {
    setArticleSelectionne(article);
    setUniteSelectionnee(article.unites[0]);
    setPrixPratique(article.unites[0].prix_reference.toString());
    setQuantite("1");
    setRechercheArticle("");
    setArticlesFiltres([]);
  }

  function handleUniteChange(uniteId: string) {
    const unite = articleSelectionne?.unites.find(u => u.id === uniteId);
    if (unite) {
      setUniteSelectionnee(unite);
      setPrixPratique(unite.prix_reference.toString());
    }
  }

  // Stock disponible pour l'unité sélectionnée
  const stockDisponible = articleSelectionne && uniteSelectionnee
    ? stockEnUniteVente(articleSelectionne.stock, uniteSelectionnee.facteur)
    : 0;

  const quantiteNum = parseFloat(quantite) || 0;
  const aDecouvert = articleSelectionne && quantiteNum > stockDisponible && stockDisponible >= 0;
  const enRupture = articleSelectionne && articleSelectionne.stock <= 0;

  // ---- Panier ----
  function ajouterAuPanier() {
    if (!articleSelectionne || !uniteSelectionnee) return;
    const qte = parseFloat(quantite) || 1;
    const prix = parseInt(prixPratique) || uniteSelectionnee.prix_reference;

    setPanier(prev => [...prev, {
      id: genId(),
      article: articleSelectionne,
      unite: uniteSelectionnee,
      quantite: qte,
      prix_pratique: prix,
      montant: Math.round(prix * qte),
      a_decouvert: qte > stockDisponible && articleSelectionne.stock >= 0,
    }]);

    setArticleSelectionne(null);
    setUniteSelectionnee(null);
    setQuantite("1");
    setPrixPratique("");
    inputArticleRef.current?.focus();
  }

  function supprimerDuPanier(id: string) {
    setPanier(prev => prev.filter(l => l.id !== id));
  }

  function viderPanier() {
    setPanier([]);
    setClient(clientGenerique);
    setModeReglement("comptant");
    setAcompte("");
    inputArticleRef.current?.focus();
  }

  // ---- Encaissement ----
  function handleEncaisser() {
    if (modeReglement === "comptant") {
      setModalEncaissement(true);
    } else {
      setModalConfirmation(true);
    }
  }

  function handleConfirmerEncaissement(mode: string, _montant: number) {
    setModePaiementComptant(mode);
    setModalEncaissement(false);
    setModalConfirmation(true);
  }

  async function handleConfirmerVente() {
    if (!client || !depotActif) return;
    setChargementVente(true);

    try {
      const lignes = panier.map(l => ({
        article_id:               l.article.id,
        unite_vente_id:           l.unite.id,
        depot_source_id:          depotActif.id,
        source_approvisionnement: "stock" as const,
        quantite:                 l.quantite,
        facteur:                  l.unite.facteur,
        prix_reference:           l.unite.prix_reference,
        prix_pratique:            l.prix_pratique,
      }));

      const { vente_id } = await creerVente(
        client.id, depotActif.id, modeReglement, lignes,
      );

      if (modeReglement === "comptant") {
        await enregistrerPaiement(vente_id, total, modePaiementComptant);
      } else if (acompteNum > 0) {
        await enregistrerPaiement(vente_id, acompteNum, modeAcompte);
      }

      // Recharger les articles pour avoir le stock à jour.
      const articles = await lireArticlesAvecUnites() as ArticleAvecStock[];
      setTousArticles(articles);

      setModalConfirmation(false);
      viderPanier();
      await message("Vente enregistrée ✓", { title: "Succès", kind: "info" });

    } catch (e) {
      setModalConfirmation(false);
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargementVente(false);
    }
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* En-tête */}
      <div className="flex items-center justify-between px-4 h-12 border-b border-border bg-card shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="font-semibold text-sm">Nouvelle vente</h1>
          {/* Sélecteur de dépôt */}
          {depots.length > 1 ? (
            <Select
              value={depotActif?.id ?? ""}
              onValueChange={v => { if (v) changerDepot(v); }}
            >
              <SelectTrigger className="h-7 text-xs w-40">
                <Warehouse className="h-3 w-3 mr-1" />
                <SelectValue>
                  {depotActif?.nom ?? "Dépôt"}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {depots.map(d => (
                  <SelectItem key={d.id} value={d.id}>{d.nom}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <div className="flex items-center gap-1 text-xs text-muted-foreground">
              <Warehouse className="h-3 w-3" />
              {depotActif?.nom}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm"
            variant={modeReglement === "comptant" ? "default" : "outline"}
            onClick={() => { setModeReglement("comptant"); setAcompte(""); }}>
            Comptant
          </Button>
          <Button size="sm"
            variant={modeReglement === "credit" ? "default" : "outline"}
            onClick={() => setModeReglement("credit")}>
            Crédit
          </Button>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">

        {/* ── Colonne gauche ── */}
        <div className="w-1/2 flex flex-col p-4 gap-4 border-r border-border overflow-auto">

          {/* Client */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <Label className="flex items-center gap-1">
                <User className="h-3 w-3" /> Client
              </Label>
              <button onClick={() => setModalNouveauClient(true)}
                className="text-xs text-primary hover:underline flex items-center gap-0.5">
                <Plus className="h-3 w-3" /> Nouveau
              </button>
            </div>

            <div className="flex items-center gap-2 mb-1">
              <Badge variant="secondary" className="text-xs">{client?.nom}</Badge>
              {client?.id !== clientGenerique?.id && (
                <button onClick={() => setClient(clientGenerique)}
                  className="text-xs text-muted-foreground hover:text-foreground">✕</button>
              )}
            </div>

            <div className="relative">
              <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
              <Input value={rechercheClient}
                onChange={e => handleRechercheClient(e.target.value)}
                placeholder="Rechercher un client..."
                className="pl-8 h-8 text-sm" />
              {clientsFiltres.length > 0 && (
                <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md">
                  {clientsFiltres.map(c => (
                    <button key={c.id} onClick={() => selectionnerClient(c)}
                      className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent transition-colors">
                      <span className="font-medium">{c.nom}</span>
                      {c.telephone && (
                        <span className="text-muted-foreground ml-2 text-xs">{c.telephone}</span>
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
                <ShoppingCart className="h-3 w-3" /> Article
              </Label>
              <button onClick={() => setModalNouvelArticle(true)}
                className="text-xs text-primary hover:underline flex items-center gap-0.5">
                <Plus className="h-3 w-3" /> Nouveau
              </button>
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
                        <div className="flex items-center gap-2">
                          <span className="text-muted-foreground text-xs">
                            {a.unites[0].prix_reference} F/{a.unite_base}
                          </span>
                          {/* Stock visible dans la liste */}
                          <Badge
                            variant={a.stock < 0 ? "destructive" : a.stock === 0 ? "outline" : "secondary"}
                            className={cn("text-xs", a.stock === 0 && "text-orange-500 border-orange-300")}
                          >
                            {a.stock < 0 ? "Découvert" : a.stock === 0 ? "Rupture" : formaterStock(a.stock, a.unite_base)}
                          </Badge>
                        </div>
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
                {/* Stock du dépôt actif */}
                <div className="flex items-center gap-1">
                  {enRupture ? (
                    <Badge variant="destructive" className="text-xs">
                      <AlertTriangle className="h-3 w-3 mr-1" />
                      Rupture
                    </Badge>
                  ) : (
                    <Badge
                      variant={aDecouvert ? "outline" : "secondary"}
                      className={cn("text-xs", aDecouvert && "text-orange-500 border-orange-300")}
                    >
                      Stock : {formaterStock(articleSelectionne.stock, articleSelectionne.unite_base)}
                    </Badge>
                  )}
                </div>
              </div>

              {/* Avertissement découvert */}
              {aDecouvert && !enRupture && (
                <p className="text-xs text-orange-500 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3" />
                  Stock insuffisant — vente à découvert autorisée
                </p>
              )}
              {enRupture && (
                <p className="text-xs text-red-500 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3" />
                  Stock négatif — vente à découvert
                </p>
              )}

              {articleSelectionne.unites.length > 1 && (
                <div>
                  <Label className="text-xs">Unité</Label>
                  <Select
                    value={uniteSelectionnee?.id ?? ""}
                    onValueChange={v => { if (v) handleUniteChange(v); }}
                  >
                    <SelectTrigger className="h-8 text-sm mt-0.5">
                      <SelectValue>
                        {uniteSelectionnee
                          ? `${uniteSelectionnee.libelle} — ${formaterMontant(uniteSelectionnee.prix_reference)}`
                          : "Choisir"
                        }
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {articleSelectionne.unites.map(u => {
                        const stockUV = stockEnUniteVente(articleSelectionne.stock, u.facteur);
                        return (
                          <SelectItem key={u.id} value={u.id}>
                            <div className="flex items-center justify-between gap-4 w-full">
                              <span>{u.libelle} — {formaterMontant(u.prix_reference)}</span>
                              <span className={cn(
                                "text-xs",
                                stockUV <= 0 ? "text-red-500" : "text-muted-foreground"
                              )}>
                                {stockUV <= 0 ? "Rupture" : `${stockUV % 1 === 0 ? stockUV : stockUV.toFixed(1)} dispo`}
                              </span>
                            </div>
                          </SelectItem>
                        );
                      })}
                    </SelectContent>
                  </Select>
                </div>
              )}

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <Label className="text-xs">Quantité</Label>
                  <Input type="number" value={quantite}
                    onChange={e => setQuantite(e.target.value)}
                    className={cn("h-8 text-sm mt-0.5", aDecouvert && "border-orange-300")}
                    autoFocus
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
                <div>
                  <Label className="text-xs">Prix (F)</Label>
                  <Input type="number" value={prixPratique}
                    onChange={e => setPrixPratique(e.target.value)}
                    className="h-8 text-sm mt-0.5"
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
              </div>

              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  Sous-total : {formaterMontant(
                    Math.round((parseInt(prixPratique) || 0) * (parseFloat(quantite) || 1))
                  )}
                </span>
                <Button size="sm" onClick={ajouterAuPanier}>Ajouter →</Button>
              </div>
            </div>
          )}
        </div>

        {/* ── Colonne droite : panier ── */}
        <div className="w-1/2 flex flex-col p-4">

          {/* Client en haut du panier */}
          <div className="flex items-center gap-2 pb-3 mb-3 border-b border-border">
            <div className="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
              <User className="h-3.5 w-3.5 text-primary" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-medium truncate">{client?.nom}</p>
              <p className="text-xs text-muted-foreground">{client?.code}</p>
            </div>
            {modeReglement === "credit" && (
              <Badge variant="outline" className="ml-auto text-xs shrink-0">Crédit</Badge>
            )}
          </div>

          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium">
              Panier ({panier.length} article{panier.length > 1 ? "s" : ""})
            </span>
            {panier.length > 0 && (
              <button onClick={viderPanier}
                className="text-xs text-muted-foreground hover:text-destructive transition-colors">
                Vider
              </button>
            )}
          </div>

          {/* Lignes du panier */}
          <div className="flex-1 overflow-auto space-y-1 min-h-0">
            {panier.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
                <ShoppingCart className="h-8 w-8 mb-2 opacity-30" />
                <p className="text-sm">Panier vide</p>
              </div>
            ) : (
              panier.map(ligne => (
                <div key={ligne.id}
                  className="flex items-center justify-between py-2 px-3 rounded-md bg-muted/40 group">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium truncate">{ligne.article.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {ligne.quantite} {ligne.unite.libelle} × {formaterMontant(ligne.prix_pratique)}
                      {ligne.prix_pratique < ligne.unite.prix_reference && (
                        <span className="text-orange-500 ml-1">(remise)</span>
                      )}
                      {ligne.a_decouvert && (
                        <span className="text-red-500 ml-1">⚠ découvert</span>
                      )}
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

          {/* Bas du panier */}
          <div className="border-t border-border pt-3 mt-3 space-y-2">

            {/* Acompte crédit */}
            {modeReglement === "credit" && (
              <div className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <div className="flex-1">
                    <Label className="text-xs text-muted-foreground">Acompte (optionnel)</Label>
                    <Input type="number" value={acompte}
                      onChange={e => setAcompte(e.target.value)}
                      placeholder="0" className="h-8 text-sm mt-0.5" />
                  </div>
                  <div className="flex-1">
                    <Label className="text-xs text-muted-foreground">Mode</Label>
                    <Select value={modeAcompte} onValueChange={v => { if (v) setModeAcompte(v); }}>
                      <SelectTrigger className="h-8 text-sm mt-0.5"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="especes">Espèces</SelectItem>
                        <SelectItem value="orange_money">Orange Money</SelectItem>
                        <SelectItem value="moov_money">Moov Money</SelectItem>
                        <SelectItem value="cheque">Chèque</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                {acompteNum > 0 && (
                  <p className="text-xs text-orange-500">
                    Reste en créance : {formaterMontant(Math.max(0, total - acompteNum))}
                  </p>
                )}
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="font-medium">Total</span>
              <span className="text-xl font-bold">{formaterMontant(total)}</span>
            </div>

            <Button className="w-full" size="lg"
              disabled={panier.length === 0 || chargementVente}
              onClick={handleEncaisser}>
              {chargementVente
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : `${modeReglement === "comptant" ? "Encaisser" : "Enregistrer"} →`
              }
            </Button>
          </div>
        </div>
      </div>

      {/* Modals */}
      <ModalNouveauClient ouvert={modalNouveauClient}
        onFermer={() => setModalNouveauClient(false)}
        onCreer={c => {
          setTousClients(prev => [...prev, c]);
          setClient(c);
          setModalNouveauClient(false);
        }} />

      <ModalNouvelArticle ouvert={modalNouvelArticle}
        onFermer={() => setModalNouvelArticle(false)}
        onCreer={a => {
          setTousArticles(prev => [...prev, a]);
          selectionnerArticle(a);
          setModalNouvelArticle(false);
        }} />

      <ModalEncaissement ouvert={modalEncaissement}
        total={total}
        onFermer={() => setModalEncaissement(false)}
        onConfirmer={handleConfirmerEncaissement} />

      <ModalConfirmation
        ouvert={modalConfirmation}
        total={total}
        acompte={modeReglement === "credit" ? acompteNum : total}
        modeAcompte={modeReglement === "comptant" ? modePaiementComptant : modeAcompte}
        modeReglement={modeReglement}
        client={client ?? { id: "", code: "", nom: "Comptant" }}
        chargement={chargementVente}
        onFermer={() => setModalConfirmation(false)}
        onConfirmer={handleConfirmerVente} />
    </div>
  );
}