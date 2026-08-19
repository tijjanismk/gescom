import { useState, useRef, useEffect } from "react";
import {
  Plus, Trash2, ShoppingCart, User, Search,
  Loader2, Warehouse, AlertTriangle, Printer
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
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { ModalImpression } from "@/components/ModalImpression";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface UniteVente {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

interface Article {
  id: string;
  nom: string;
  unite_base: string;
  stock: number;
  unites: UniteVente[];
}

interface Client {
  id: string;
  code: string;
  nom: string;
  telephone?: string;
}

interface Depot {
  id: string;
  nom: string;
  est_defaut: boolean;
}

interface LignePanier {
  id: string;
  article: Article;
  unite: UniteVente;
  quantite: number;
  prix_pratique: number;
  montant: number;
  a_decouvert: boolean;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function fmtQte(q: number, unite: string): string {
  return `${q % 1 === 0 ? q : q.toFixed(2)} ${unite}`;
}

function genId(): string {
  return Math.random().toString(36).slice(2);
}

function stockEnUniteVente(stockBase: number, facteur: number): number {
  return stockBase / facteur;
}

// =====================================================================
//  Modal : Nouveau client rapide
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
      const client = await invoke<Client>("creer_client_rapide", {
        nom: nom.trim(),
        telephone: telephone.trim() || null,
      });
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
              autoFocus placeholder="Nom du client"
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
  const montantNum = parseMontant(montant);
  const rendu = montantNum - total;

  useEffect(() => { setMontant(total.toString()); }, [total]);

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Encaissement</DialogTitle></DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-lg p-4 text-center">
            <p className="text-sm text-muted-foreground">Total à payer</p>
            <p className="text-3xl font-bold mt-1">{fmt(total)}</p>
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

          {mode === "especes" && (
            <div>
              <Label>Montant reçu (F)</Label>
              <MoneyInput
                value={montant}
                onChange={setMontant}
                className="mt-1 text-lg"
                autoFocus
              />
              {rendu > 0 && (
                <p className="text-sm text-green-600 mt-1 font-medium">
                  Rendu : {fmt(rendu)}
                </p>
              )}
              {montantNum < total && montantNum > 0 && (
                <p className="text-sm text-red-500 mt-1">Montant insuffisant</p>
              )}
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={() => onConfirmer(mode, mode === "especes" ? montantNum : total)}
              disabled={mode === "especes" && montantNum < total}
              className="flex-1"
            >
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
              <span className="font-semibold">{fmt(total)}</span>
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
                  <span className="text-muted-foreground">Acompte</span>
                  <span className="text-green-600 font-medium">{fmt(acompte)}</span>
                </div>
                <div className="flex justify-between text-sm border-t border-border pt-2">
                  <span className="text-muted-foreground">Reste en créance</span>
                  <span className="text-orange-500 font-semibold">{fmt(total - acompte)}</span>
                </div>
              </>
            )}
            {modeReglement === "credit" && acompte === 0 && (
              <div className="flex justify-between text-sm border-t border-border pt-2">
                <span className="text-muted-foreground">Créance ouverte</span>
                <span className="text-orange-500 font-semibold">{fmt(total)}</span>
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
//  Page Ventes (POS)
// =====================================================================

export function Ventes() {
  const [clientGenerique, setClientGenerique] = useState<Client | null>(null);
  const [tousClients, setTousClients] = useState<Client[]>([]);
  const [tousArticles, setTousArticles] = useState<Article[]>([]);
  const [depots, setDepots] = useState<Depot[]>([]);
  const [depotActif, setDepotActif] = useState<Depot | null>(null);
  const [chargementInitial, setChargementInitial] = useState(true);

  const [client, setClient] = useState<Client | null>(null);
  const [rechercheClient, setRechercheClient] = useState("");
  const [clientsFiltres, setClientsFiltres] = useState<Client[]>([]);
  const [modalNouveauClient, setModalNouveauClient] = useState(false);

  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<Article[]>([]);
  const [articleSelectionne, setArticleSelectionne] = useState<Article | null>(null);
  const [uniteSelectionnee, setUniteSelectionnee] = useState<UniteVente | null>(null);
  const [quantite, setQuantite] = useState("1");
  const [prixPratique, setPrixPratique] = useState("");

  const [panier, setPanier] = useState<LignePanier[]>([]);
  const [modeReglement, setModeReglement] = useState<"comptant" | "credit">("comptant");
  const [acompte, setAcompte] = useState("");
  const [modeAcompte, setModeAcompte] = useState("especes");

  const [modalEncaissement, setModalEncaissement] = useState(false);
  const [modalConfirmation, setModalConfirmation] = useState(false);
  const [modalImpression, setModalImpression] = useState(false);
  const [modePaiementComptant, setModePaiementComptant] = useState("especes");
  const [chargementVente, setChargementVente] = useState(false);
  const [venteIdPourImpression, setVenteIdPourImpression] = useState<string | null>(null);

  const inputArticleRef = useRef<HTMLInputElement>(null);
  const total = panier.reduce((s, l) => s + l.montant, 0);
  const acompteNum = parseMontant(acompte);

  // ---- Chargement initial ----
  useEffect(() => {
    async function charger() {
      try {
        const role = UTILISATEUR_ACTIF?.role ?? "employe";
        const [gen, clients, articles, tousDepots] = await Promise.all([
          invoke<Client>("lire_client_generique"),
          invoke<Client[]>("lire_clients"),
          invoke<Article[]>("lire_articles_avec_unites", { role }),
          invoke<Depot[]>("lire_depots"),
        ]);
        setClientGenerique(gen);
        setClient(gen);
        setTousClients(clients);
        setTousArticles(articles);
        setDepots(tousDepots);
        setDepotActif(tousDepots.find(d => d.est_defaut) ?? tousDepots[0]);
      } catch (e) {
        console.error("Erreur chargement ventes :", e);
      } finally {
        setChargementInitial(false);
      }
    }
    charger();
  }, []);

  async function rechargerArticles() {
    const role = UTILISATEUR_ACTIF?.role ?? "employe";
    const articles = await invoke<Article[]>("lire_articles_avec_unites", { role });
    setTousArticles(articles);
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

  function selectionnerArticle(article: Article) {
    setArticleSelectionne(article);
    setUniteSelectionnee(article.unites[0]);
    setPrixPratique(article.unites[0].prix_reference.toString());
    setQuantite("1");
    setRechercheArticle("");
    setArticlesFiltres([]);
  }

  const stockDispo = articleSelectionne && uniteSelectionnee
    ? stockEnUniteVente(articleSelectionne.stock, uniteSelectionnee.facteur)
    : 0;
  const qteNum = parseFloat(quantite) || 0;
  const aDecouvert = !!articleSelectionne && qteNum > stockDispo && stockDispo >= 0;
  const enRupture = !!articleSelectionne && articleSelectionne.stock <= 0;

  // ---- Panier ----
  function ajouterAuPanier() {
    if (!articleSelectionne || !uniteSelectionnee) return;
    const qte = parseFloat(quantite) || 1;
    const prix = parseMontant(prixPratique) || uniteSelectionnee.prix_reference;
    setPanier(prev => [...prev, {
      id: genId(),
      article: articleSelectionne,
      unite: uniteSelectionnee,
      quantite: qte,
      prix_pratique: prix,
      montant: Math.round(prix * qte),
      a_decouvert: qte > stockDispo && articleSelectionne.stock >= 0,
    }]);
    setArticleSelectionne(null);
    setUniteSelectionnee(null);
    setQuantite("1");
    setPrixPratique("");
    inputArticleRef.current?.focus();
  }

  function viderPanier() {
    setPanier([]);
    setClient(clientGenerique);
    setModeReglement("comptant");
    setAcompte("");
  }

  // ---- Encaissement ----
  async function handleConfirmerVente() {
    if (!client || !depotActif) return;
    setChargementVente(true);
    try {
      const lignes = panier.map(l => ({
        article_id: l.article.id,
        unite_vente_id: l.unite.id,
        depot_source_id: depotActif.id,
        source_approvisionnement: "stock",
        quantite: l.quantite,
        facteur: l.unite.facteur,
        prix_reference: l.unite.prix_reference,
        prix_pratique: l.prix_pratique,
        taux_tva: 0.0,
      }));

      const role = UTILISATEUR_ACTIF?.role ?? "employe";
      const { vente_id } = await invoke<{ vente_id: string }>("creer_vente", {
        clientId: client.id,
        depotId: depotActif.id,
        modeReglement,
        lignes,
        utilisateurRole: role,
      });

      if (modeReglement === "comptant") {
        await invoke("enregistrer_paiement", {
          venteId: vente_id,
          montant: total,
          mode: modePaiementComptant,
          utilisateurRole: role,
        });
      } else if (acompteNum > 0) {
        await invoke("enregistrer_paiement", {
          venteId: vente_id,
          montant: acompteNum,
          mode: modeAcompte,
          utilisateurRole: role,
        });
      }

      await rechargerArticles();
      setModalConfirmation(false);
      viderPanier();

      // Proposer l'impression.
      setVenteIdPourImpression(vente_id);
      setModalImpression(true);

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
          {depots.length > 1 ? (
            <Select value={depotActif?.id ?? ""}
              onValueChange={v => {
                const d = depots.find(dep => dep.id === v);
                if (d) setDepotActif(d);
              }}>
              <SelectTrigger className="h-7 text-xs w-40">
                <Warehouse className="h-3 w-3 mr-1" />
                <SelectValue>{depotActif?.nom}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {depots.map(d => <SelectItem key={d.id} value={d.id}>{d.nom}</SelectItem>)}
              </SelectContent>
            </Select>
          ) : (
            <div className="flex items-center gap-1 text-xs text-muted-foreground">
              <Warehouse className="h-3 w-3" /> {depotActif?.nom}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant={modeReglement === "comptant" ? "default" : "outline"}
            onClick={() => { setModeReglement("comptant"); setAcompte(""); }}>
            Comptant
          </Button>
          <Button size="sm" variant={modeReglement === "credit" ? "default" : "outline"}
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
                      {c.telephone && <span className="text-muted-foreground ml-2 text-xs">{c.telephone}</span>}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Article */}
          <div>
            <Label className="flex items-center gap-1 mb-1">
              <ShoppingCart className="h-3 w-3" /> Article
            </Label>
            <div className="relative">
              <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
              <Input ref={inputArticleRef} value={rechercheArticle}
                onChange={e => handleRechercheArticle(e.target.value)}
                placeholder="Rechercher un article..."
                className="pl-8 h-8 text-sm" />
              {articlesFiltres.length > 0 && (
                <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md max-h-48 overflow-auto">
                  {articlesFiltres.map(a => (
                    <button key={a.id} onClick={() => selectionnerArticle(a)}
                      className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent transition-colors">
                      <div className="flex items-center justify-between">
                        <span className="font-medium">{a.nom}</span>
                        <div className="flex items-center gap-2">
                          <span className="text-muted-foreground text-xs">
                            {fmt(a.unites[0].prix_reference)}/{a.unite_base}
                          </span>
                          <Badge variant={
                            a.stock < 0 ? "destructive"
                            : a.stock === 0 ? "outline" : "secondary"}
                            className={cn("text-xs",
                              a.stock === 0 && "text-orange-500 border-orange-300")}>
                            {a.stock < 0 ? "Découvert" : a.stock === 0 ? "Rupture"
                              : fmtQte(a.stock, a.unite_base)}
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
                {enRupture ? (
                  <Badge variant="destructive" className="text-xs">
                    <AlertTriangle className="h-3 w-3 mr-1" /> Rupture
                  </Badge>
                ) : (
                  <Badge variant={aDecouvert ? "outline" : "secondary"}
                    className={cn("text-xs", aDecouvert && "text-orange-500 border-orange-300")}>
                    Stock : {fmtQte(articleSelectionne.stock, articleSelectionne.unite_base)}
                  </Badge>
                )}
              </div>

              {aDecouvert && (
                <p className="text-xs text-orange-500 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3" /> Vente à découvert autorisée
                </p>
              )}

              {articleSelectionne.unites.length > 1 && (
                <div>
                  <Label className="text-xs">Unité</Label>
                  <Select value={uniteSelectionnee?.id ?? ""}
                    onValueChange={v => {
                      const u = articleSelectionne.unites.find(u => u.id === v);
                      if (u) { setUniteSelectionnee(u); setPrixPratique(u.prix_reference.toString()); }
                    }}>
                    <SelectTrigger className="h-8 text-sm mt-0.5">
                      <SelectValue>
                        {uniteSelectionnee
                          ? `${uniteSelectionnee.libelle} — ${fmt(uniteSelectionnee.prix_reference)}`
                          : "Choisir"}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {articleSelectionne.unites.map(u => (
                        <SelectItem key={u.id} value={u.id}>
                          {u.libelle} — {fmt(u.prix_reference)}
                        </SelectItem>
                      ))}
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
                  <MoneyInput value={prixPratique} onChange={setPrixPratique}
                    className="h-8 mt-0.5"
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
              </div>

              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  Sous-total : {fmt(Math.round((parseMontant(prixPratique) || 0) * (parseFloat(quantite) || 1)))}
                </span>
                <Button size="sm" onClick={ajouterAuPanier}>Ajouter →</Button>
              </div>
            </div>
          )}
        </div>

        {/* ── Colonne droite : panier ── */}
        <div className="w-1/2 flex flex-col p-4">
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
                      {ligne.quantite} {ligne.unite.libelle} × {fmt(ligne.prix_pratique)}
                      {ligne.prix_pratique < ligne.unite.prix_reference && (
                        <span className="text-orange-500 ml-1">(remise)</span>
                      )}
                      {ligne.a_decouvert && (
                        <span className="text-red-500 ml-1">⚠</span>
                      )}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 ml-2">
                    <span className="text-sm font-semibold">{fmt(ligne.montant)}</span>
                    <button onClick={() => setPanier(prev => prev.filter(l => l.id !== ligne.id))}
                      className="opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-destructive">
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>

          <div className="border-t border-border pt-3 mt-3 space-y-2">
            {modeReglement === "credit" && (
              <div className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <div className="flex-1">
                    <Label className="text-xs text-muted-foreground">Acompte (optionnel)</Label>
                    <MoneyInput value={acompte} onChange={setAcompte}
                      placeholder="0" className="h-8 mt-0.5" />
                  </div>
                  <div className="flex-1">
                    <Label className="text-xs text-muted-foreground">Mode</Label>
                    <Select value={modeAcompte} onValueChange={v => { if (v) setModeAcompte(v); }}>
                      <SelectTrigger className="h-8 text-sm mt-0.5"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="especes">Espèces</SelectItem>
                        <SelectItem value="orange_money">Orange Money</SelectItem>
                        <SelectItem value="moov_money">Moov Money</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                {acompteNum > 0 && (
                  <p className="text-xs text-orange-500">
                    Reste en créance : {fmt(Math.max(0, total - acompteNum))}
                  </p>
                )}
              </div>
            )}

            <div className="flex items-center justify-between">
              <span className="font-medium">Total</span>
              <span className="text-xl font-bold">{fmt(total)}</span>
            </div>

            <Button className="w-full" size="lg"
              disabled={panier.length === 0 || chargementVente}
              onClick={() => modeReglement === "comptant"
                ? setModalEncaissement(true)
                : setModalConfirmation(true)
              }>
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
        onCreer={c => { setTousClients(prev => [...prev, c]); setClient(c); setModalNouveauClient(false); }} />

      <ModalEncaissement ouvert={modalEncaissement} total={total}
        onFermer={() => setModalEncaissement(false)}
        onConfirmer={(mode, _montant) => {
          setModePaiementComptant(mode);
          setModalEncaissement(false);
          setModalConfirmation(true);
        }} />

      <ModalConfirmation
        ouvert={modalConfirmation} total={total}
        acompte={modeReglement === "comptant" ? total : acompteNum}
        modeAcompte={modeReglement === "comptant" ? modePaiementComptant : modeAcompte}
        modeReglement={modeReglement}
        client={client ?? { id: "", code: "", nom: "Comptant" }}
        chargement={chargementVente}
        onFermer={() => setModalConfirmation(false)}
        onConfirmer={handleConfirmerVente} />

      <ModalImpression
        ouvert={modalImpression}
        venteId={venteIdPourImpression}
        onFermer={() => setModalImpression(false)} />
    </div>
  );
}
