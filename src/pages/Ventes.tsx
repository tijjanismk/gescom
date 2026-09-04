import { useState, useRef, useEffect, useCallback } from "react";
import {
  Plus, Trash2, ShoppingCart, User, Search,
  Loader2, Warehouse, AlertTriangle, Gift, Scan, PackagePlus
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
import { SelectUnite } from "@/components/SelectUnite";
import { ModalImpression } from "@/components/ModalImpression";
import { useScanner } from "@/lib/useScanner";
import { UTILISATEUR_ACTIF, DEPOT_ACTIF, definirDepotActif } from "@/App";
import type { CreerVenteResultat } from "@/lib/types-api";

// =====================================================================
//  Types
// =====================================================================

interface UniteVente {
  id: string; libelle: string; facteur: number; prix_reference: number;
}
interface Article {
  id: string; nom: string; unite_base: string; stock: number;
  taux_tva_defaut?: number;
  unites: UniteVente[];
  // Renseigne uniquement par le scan : l'unite dont le code-barres
  // a ete lu. null si le code designait l'article (D45).
  unite_scannee_id?: string | null;
}
interface Client {
  id: string; code: string; nom: string; telephone?: string;
}
interface Depot {
  id: string; nom: string; est_defaut: boolean;
}
interface LignePanier {
  id: string; article: Article; unite: UniteVente;
  quantite: number; prix_pratique: number; montant: number; a_decouvert: boolean;
  // Depot d'ou sort REELLEMENT cette ligne. Une meme vente peut puiser
  // dans deux depots : 5 sacs en boutique, 15 a la reserve. Deux lignes,
  // deux origines, chaque stock baisse au bon endroit.
  depot_id: string;
  depot_nom: string;
}

/** Stock d'un article, dépôt par dépôt. */
interface StockDepot {
  article_id: string; depot_id: string; depot_nom: string;
  est_defaut: boolean; quantite: number;
}
// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtQte(q: number, u: string): string {
  return `${q % 1 === 0 ? q : q.toFixed(2)} ${u}`;
}
function genId(): string { return Math.random().toString(36).slice(2); }
function stockUV(stockBase: number, facteur: number): number {
  return stockBase / facteur;
}

// =====================================================================
//  Modal : Nouveau client
// =====================================================================

function ModalNouveauClient({
  ouvert, onFermer, onCreer,
}: { ouvert: boolean; onFermer: () => void; onCreer: (c: Client) => void }) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      const client = await invoke<Client>("creer_client_rapide", {
        nom: nom.trim(), telephone: telephone.trim() || null,
      });
      onCreer(client); setNom(""); setTelephone("");
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
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
//  Modal : Encaissement
// =====================================================================

function ModalEncaissement({
  ouvert, total, onFermer, onConfirmer,
  chequeNumero, setChequeNumero, chequeBanque, setChequeBanque,
}: {
  ouvert: boolean; total: number;
  onFermer: () => void; onConfirmer: (mode: string, montant: number) => void;
  chequeNumero: string; setChequeNumero: (v: string) => void;
  chequeBanque: string; setChequeBanque: (v: string) => void;
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
          {/* Un cheque doit etre identifie : sans numero ni banque, on ne
              saura pas plus tard lequel a ete encaisse ou rejete. */}
          {mode === "cheque" && (
            <div className="flex gap-2">
              <div className="flex-1">
                <Label>N° du chèque</Label>
                <Input value={chequeNumero}
                  onChange={e => setChequeNumero(e.target.value)}
                  placeholder="0123456" className="mt-1" autoFocus />
              </div>
              <div className="flex-1">
                <Label>Banque</Label>
                <Input value={chequeBanque}
                  onChange={e => setChequeBanque(e.target.value)}
                  placeholder="BDM, BOA…" className="mt-1" />
              </div>
            </div>
          )}
          {mode === "especes" && (
            <div>
              <Label>Montant reçu (F)</Label>
              <MoneyInput value={montant} onChange={setMontant}
                className="mt-1 text-lg" autoFocus />
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
  ouvert, total, totalApresAvoir, avoirApplique,
  acompte, modeAcompte, modeReglement, client,
  chargement, onFermer, onConfirmer,
}: {
  ouvert: boolean; total: number; totalApresAvoir: number; avoirApplique: number;
  acompte: number; modeAcompte: string; modeReglement: "comptant" | "credit";
  client: Client; chargement: boolean; onFermer: () => void; onConfirmer: () => void;
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
              <span className="text-muted-foreground">Total articles</span>
              <span className="font-semibold">{fmt(total)}</span>
            </div>
            {avoirApplique > 0 && (
              <div className="flex justify-between text-sm text-green-600">
                <span className="flex items-center gap-1">
                  <Gift className="h-3 w-3" /> Avoir appliqué
                </span>
                <span>- {fmt(avoirApplique)}</span>
              </div>
            )}
            {avoirApplique > 0 && (
              <div className="flex justify-between text-sm font-bold border-t border-border pt-2">
                <span>Reste à payer</span>
                <span>{fmt(totalApresAvoir)}</span>
              </div>
            )}
            {modeReglement === "comptant" && totalApresAvoir > 0 && (
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Mode</span>
                <span className="capitalize">{modeAcompte.replace(/_/g, " ")}</span>
              </div>
            )}
            {modeReglement === "credit" && acompte > 0 && (
              <>
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">Acompte</span>
                  <span className="text-green-600">{fmt(acompte)}</span>
                </div>
                <div className="flex justify-between text-sm border-t border-border pt-2">
                  <span className="text-muted-foreground">Reste en créance</span>
                  <span className="text-orange-500 font-semibold">
                    {fmt(Math.max(0, totalApresAvoir - acompte))}
                  </span>
                </div>
              </>
            )}
            {modeReglement === "credit" && acompte === 0 && totalApresAvoir > 0 && (
              <div className="flex justify-between text-sm border-t border-border pt-2">
                <span className="text-muted-foreground">Créance ouverte</span>
                <span className="text-orange-500 font-semibold">{fmt(totalApresAvoir)}</span>
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
//  Page Ventes

// =====================================================================
//  Modal : répartition d'une ligne entre plusieurs dépôts
// =====================================================================
//
//  Le dépôt actif ne suffit pas, mais un autre a le complément. Plutôt
//  que de vendre à découvert — ce qui rendrait DEUX stocks faux — on
//  dit d'où sort chaque unité.
//
//  Le vendeur met la vente en pause, envoie quelqu'un chercher la
//  marchandise, et valide au retour. Une ligne de panier par dépôt :
//  chaque stock baisse là où la marchandise est réellement partie.

function ModalRepartition({
  demande, depotActif, stockDepots, onAnnuler, onValider,
}: {
  demande: { article: Article; unite: UniteVente; quantite: number; prix: number } | null;
  depotActif: Depot | null;
  stockDepots: StockDepot[];
  onAnnuler: () => void;
  onValider: (parts: {
    depot_id: string; depot_nom: string; qte: number; a_decouvert: boolean;
  }[]) => void;
}) {
  const [parts, setParts] = useState<Record<string, string>>({});

  // Sources disponibles, dépôt actif en tête.
  const sources = demande
    ? stockDepots
        .filter(sd => sd.article_id === demande.article.id && sd.quantite > 0)
        .sort((a, b) =>
          (b.depot_id === depotActif?.id ? 1 : 0) -
          (a.depot_id === depotActif?.id ? 1 : 0))
    : [];

  useEffect(() => {
    if (!demande) { setParts({}); return; }
    // Pré-remplir : on prend d'abord ce qu'on a sous la main.
    let reste = demande.quantite;
    const init: Record<string, string> = {};
    for (const sd of sources) {
      const pris = Math.min(reste, sd.quantite);
      init[sd.depot_id] = pris > 0 ? String(pris) : "";
      reste -= pris;
      if (reste <= 0) break;
    }
    setParts(init);
  }, [demande]);

  if (!demande) return null;

  const total = Object.values(parts)
    .reduce((s, v) => s + (parseFloat(v) || 0), 0);
  const manque = demande.quantite - total;
  const depassements = sources.filter(sd =>
    (parseFloat(parts[sd.depot_id] || "0") || 0) > sd.quantite);

  return (
    <Dialog open={!!demande} onOpenChange={onAnnuler}>
      <DialogContent style={{ width: "480px", maxWidth: "94vw" }}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Warehouse className="h-4 w-4" />
            D'où sort la marchandise ?
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-lg p-3">
            <p className="text-sm font-medium">{demande.article.nom}</p>
            <p className="text-xs text-muted-foreground">
              {demande.quantite} {demande.unite.libelle} demandé(s)
              · {fmt(demande.prix)} l'unité
            </p>
          </div>

          <p className="text-xs text-muted-foreground">
            Le dépôt courant ne suffit pas. Indiquer combien vient de
            chaque endroit, puis envoyer quelqu'un chercher le complément
            avant de valider.
          </p>

          <div className="space-y-2">
            {sources.map(sd => {
              const val = parts[sd.depot_id] || "";
              const trop = (parseFloat(val) || 0) > sd.quantite;
              return (
                <div key={sd.depot_id} className="flex items-center gap-3">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm">
                      {sd.depot_nom}
                      {sd.depot_id === depotActif?.id && (
                        <span className="text-xs text-muted-foreground"> · ici</span>
                      )}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {sd.quantite} disponible(s)
                    </p>
                  </div>
                  <Input type="number" min="0" step="any" max={sd.quantite}
                    value={val}
                    onChange={e => setParts(p => ({ ...p, [sd.depot_id]: e.target.value }))}
                    className={`h-8 w-24 text-right text-sm ${
                      trop ? "border-destructive" : ""}`} />
                </div>
              );
            })}
          </div>

          <div className={`flex justify-between text-sm px-3 py-2 rounded-md ${
            manque === 0 ? "bg-green-50 text-green-800" : "bg-orange-50 text-orange-800"
          }`}>
            <span>Réparti</span>
            <strong>
              {total} / {demande.quantite}
              {manque > 0 && ` — ${manque} à découvert`}
              {manque < 0 && ` — ${-manque} en trop`}
            </strong>
          </div>

          {/* Le manque n'est plus une erreur : c'est une vente à
              découvert, autorisée comme en mono-dépôt. Elle est imputée
              au dépôt où se tient le vendeur. */}
          {manque > 0 && (
            <p className="text-xs text-orange-700">
              {manque} unité(s) au-delà du stock — imputée(s) à{" "}
              {depotActif?.nom ?? "ce dépôt"}. À régulariser par une entrée
              ou un achat.
            </p>
          )}

          {manque < 0 && (
            <p className="text-xs text-destructive">
              Le total dépasse la quantité demandée.
            </p>
          )}

          {depassements.length > 0 && (
            <p className="text-xs text-destructive">
              Une quantité dépasse le stock d'un dépôt. Laisser le manque
              en découvert plutôt que de le cacher dans une ligne.
            </p>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onAnnuler} className="flex-1">
              Annuler
            </Button>
            <Button
              disabled={manque < 0 || depassements.length > 0 || total <= 0}
              onClick={() => onValider(
                sources.map(sd => ({
                  depot_id: sd.depot_id, depot_nom: sd.depot_nom,
                  qte: parseFloat(parts[sd.depot_id] || "0") || 0,
                  a_decouvert: false,
                })).concat(manque > 0 && depotActif ? [{
                  // Le reliquat sort du depot du vendeur, marque comme
                  // decouvert pour que le panier l'affiche en orange.
                  depot_id: depotActif.id, depot_nom: depotActif.nom,
                  qte: manque, a_decouvert: true,
                }] : [])
              )}
              className="flex-1">
              {manque > 0 ? "Valider — dont découvert" : "Marchandise réunie — valider"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================

export function Ventes() {
  // Données
  const [clientGenerique, setClientGenerique] = useState<Client | null>(null);
  const [tousClients, setTousClients] = useState<Client[]>([]);
  const [tousArticles, setTousArticles] = useState<Article[]>([]);
  const [depots, setDepots] = useState<Depot[]>([]);
  const [depotActif, setDepotActif] = useState<Depot | null>(null);
  // Stock de tous les articles dans tous les depots, charge une fois.
  // Interroger depot par depot a chaque frappe serait intenable.
  const [stockDepots, setStockDepots] = useState<StockDepot[]>([]);
  // Ligne en attente de repartition entre depots.
  const [repartition, setRepartition] = useState<{
    article: Article; unite: UniteVente; quantite: number; prix: number;
  } | null>(null);
  const [scannerActif, setScannerActif] = useState(false);
  const [chargementInitial, setChargementInitial] = useState(true);

  // Client & avoirs
  const [client, setClient] = useState<Client | null>(null);
  const [rechercheClient, setRechercheClient] = useState("");
  const [clientsFiltres, setClientsFiltres] = useState<Client[]>([]);
  const [modalNouveauClient, setModalNouveauClient] = useState(false);

  const [totalAvoirs, setTotalAvoirs] = useState(0);
  const [avoirAAppliquer, setAvoirAAppliquer] = useState(0);

  // Article
  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<Article[]>([]);
  // Création rapide : un client demande un article absent du catalogue.
  // L'envoyer dans Paramètres au milieu d'une vente, c'est la vente
  // qu'on perd.
  const [modalNouvelArticle, setModalNouvelArticle] = useState(false);
  const [naNom, setNaNom] = useState("");
  const [naUnite, setNaUnite] = useState("");
  const [naPrix, setNaPrix] = useState("");
  const [naEnCours, setNaEnCours] = useState(false);
  const [articleSelectionne, setArticleSelectionne] = useState<Article | null>(null);
  const [uniteSelectionnee, setUniteSelectionnee] = useState<UniteVente | null>(null);
  const [quantite, setQuantite] = useState("1");
  const [prixPratique, setPrixPratique] = useState("");
  const [remisePct, setRemisePct] = useState("");

  // Panier
  const [panier, setPanier] = useState<LignePanier[]>([]);
  const [modeReglement, setModeReglement] = useState<"comptant" | "credit">("comptant");
  const [acompte, setAcompte] = useState("");
  const [modeAcompte, setModeAcompte] = useState("especes");

  // Modals
  const [modalEncaissement, setModalEncaissement] = useState(false);
  const [modalConfirmation, setModalConfirmation] = useState(false);
  const [modalImpression, setModalImpression] = useState(false);
  const [modePaiementComptant, setModePaiementComptant] = useState("especes");
  // Un cheque doit etre identifie : numero + banque. Sans cela il est
  // impossible de savoir plus tard lequel a ete encaisse ou rejete.
  const [chequeNumero, setChequeNumero] = useState("");
  const [chequeBanque, setChequeBanque] = useState("");
  const [chargementVente, setChargementVente] = useState(false);
  const [venteIdPourImpression, setVenteIdPourImpression] = useState<string | null>(null);
  const [scannerNotification, setScannerNotification] = useState<string | null>(null);

  const inputArticleRef = useRef<HTMLInputElement>(null);
  // Les prix saisis / de reference sont des prix HORS TAXE.
  // La TVA est AJOUTEE par-dessus : le client paie HT + TVA.
  // On calcule ici le PU TTC une seule fois par ligne, et c'est ce PU
  // qui part vers creer_vente -> prix_pratique reste "ce que le client
  // paie", et toutes les requetes de totaux restent justes.
  const puTTC = (l: LignePanier) =>
    Math.round(l.prix_pratique * (1 + (l.article.taux_tva_defaut ?? 0)));

  const totalHT  = panier.reduce((s, l) => s + l.montant, 0);
  const total    = panier.reduce((s, l) => s + Math.round(puTTC(l) * l.quantite), 0);
  const totalTVA = total - totalHT;
  const aTVA = totalTVA > 0;
  const totalApresAvoir = Math.max(0, total - avoirAAppliquer);
  const acompteNum = parseMontant(acompte);

  // ---- Chargement initial ----
  useEffect(() => {
    async function charger() {
      try {
        const role = UTILISATEUR_ACTIF?.role ?? "employe";
        // taux_tva_defaut est fourni directement par lire_articles_avec_unites :
        // une seule source, celle de Parametres -> TVA.
        const [gen, clients, articles, tousDepots, scannerConfig] = await Promise.all([
          invoke<Client>("lire_client_generique"),
          invoke<Client[]>("lire_clients"),
          invoke<Article[]>("lire_articles_avec_unites", { role }),
          invoke<Depot[]>("lire_depots"),
          invoke<boolean>("lire_config_scanner"),
        ]);
        setClientGenerique(gen);
        setClient(gen);
        setTousClients(clients);
        setTousArticles(articles);
        setDepots(tousDepots);
        invoke<StockDepot[]>("lire_stock_multi_depots")
          .then(setStockDepots).catch(console.error);
        // Le depot de la sidebar fait foi. Sans cela, filtrer sur un
        // depot puis vendre sortait la marchandise du depot PAR DEFAUT
        // — le stock baissait au mauvais endroit.
        // DEPOT_ACTIF null = vue consolidee : on retombe sur le defaut.
        setDepotActif(
          tousDepots.find(d => d.id === DEPOT_ACTIF)
          ?? tousDepots.find(d => d.est_defaut)
          ?? tousDepots[0]
        );
        setScannerActif(scannerConfig);
      } catch (e) {
        console.error("Erreur chargement ventes :", e);
      } finally {
        setChargementInitial(false);
      }
    }
    charger();
  }, []);

  // ---- Recharger les articles quand la fenetre reprend le focus ----
  // Un taux de TVA modifie dans Parametres n'etait visible qu'au
  // prochain montage de l'ecran : les articles sont charges une fois au
  // mount. On rafraichit donc au retour sur la fenetre, ce qui couvre
  // aussi le cas ou un autre poste a modifie un prix.
  useEffect(() => {
    function surRetour() {
      if (document.visibilityState === "visible") {
        rechargerArticles().catch(console.error);
      }
    }
    window.addEventListener("focus", surRetour);
    document.addEventListener("visibilitychange", surRetour);
    return () => {
      window.removeEventListener("focus", surRetour);
      document.removeEventListener("visibilitychange", surRetour);
    };
  }, []);

  // ---- Client de passage : ni crédit, ni avoir (D40) ----
  const estComptant = !client || client.id === clientGenerique?.id;

  // Repasser en comptant si le client redevient générique : sinon
  // l'état reste "credit" jusqu'au refus serveur, en pleine vente.
  useEffect(() => {
    if (estComptant && modeReglement === "credit") {
      setModeReglement("comptant");
      setAcompte("");
    }
  }, [estComptant, modeReglement]);

  // ---- Charger les avoirs quand le client change ----
  useEffect(() => {
    async function chargerAvoirs() {
      // `!client` en plus de estComptant : TS ne deduit pas le non-null.
      if (estComptant || !client) {
        setTotalAvoirs(0);
        setAvoirAAppliquer(0);
        return;
      }
      try {
        // Seul le TOTAL est affiche ; la liste detaillee n'etait lue
        // nulle part, on ne la demande plus.
        const total = await invoke<number>(
          "total_avoirs_client", { clientId: client.id });
        setTotalAvoirs(total);
      } catch (e) {
        console.error("Erreur avoirs :", e);
      }
    }
    chargerAvoirs();
  }, [client, clientGenerique, estComptant]);

  // ---- Scanner code-barres ----
  const handleScan = useCallback(async (code: string) => {
    try {
      const article = await invoke<Article | null>(
        "chercher_article_par_code_barre", { codeBarre: code }
      );
      if (article) {
        selectionnerArticle(article);
        // Le code peut designer un CONDITIONNEMENT precis (D45) : on
        // ajoute alors ce carton-la, pas l'unite de base. Sinon le
        // carton partait au prix de la piece.
        const scannee = article.unite_scannee_id
          ? article.unites.find(u => u.id === article.unite_scannee_id)
          : null;
        if (scannee) {
          setPanier(prev => [...prev, {
            id: genId(), article, unite: scannee,
            quantite: 1,
            prix_pratique: scannee.prix_reference,
            montant: scannee.prix_reference,
            a_decouvert: 1 > stockUV(article.stock, scannee.facteur)
                         && article.stock >= 0,
            depot_id: depotActif?.id ?? "",
            depot_nom: depotActif?.nom ?? "",
          }]);
          setArticleSelectionne(null);
          setScannerNotification(`✓ ${article.nom} — ${scannee.libelle}`);
        } else if (article.unites.length === 1) {
          const unite = article.unites[0];
          setPanier(prev => [...prev, {
            id: genId(), article, unite,
            quantite: 1,
            prix_pratique: unite.prix_reference,
            montant: unite.prix_reference,
            a_decouvert: 1 > stockUV(article.stock, unite.facteur) && article.stock >= 0,
            // Sans ces deux champs la ligne partait sans depot_source_id :
            // creer_vente decrementait un depot vide.
            depot_id: depotActif?.id ?? "",
            depot_nom: depotActif?.nom ?? "",
          }]);
          setArticleSelectionne(null);
          setScannerNotification(`✓ ${article.nom} ajouté`);
        } else {
          setScannerNotification(`${article.nom} — choisir l'unité`);
        }
        setTimeout(() => setScannerNotification(null), 2000);
      } else {
        setScannerNotification(`Article non trouvé : ${code}`);
        setTimeout(() => setScannerNotification(null), 3000);
      }
    } catch (e) {
      console.error("Erreur scan :", e);
    }
  }, []);

  useScanner({ actif: scannerActif, onScan: handleScan });

  async function rechargerArticles() {
    const role = UTILISATEUR_ACTIF?.role ?? "employe";
    const articles = await invoke<Article[]>(
      "lire_articles_avec_unites", { role }
    );
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
    setAvoirAAppliquer(0);
    inputArticleRef.current?.focus();
  }

  // ---- Article ----
  function ouvrirNouvelArticle() {
    setNaNom(rechercheArticle.trim());
    setNaUnite(""); setNaPrix("");
    setModalNouvelArticle(true);
  }

  async function handleCreerArticle() {
    if (!naNom.trim() || !naUnite.trim() || !naPrix) return;
    setNaEnCours(true);
    try {
      await invoke("creer_article_rapide", {
        nom: naNom.trim(),
        uniteBase: naUnite.trim(),
        prixReference: parseMontant(naPrix),
        prixAchat: null,
      });
      const role = UTILISATEUR_ACTIF?.role ?? "employe";
      const articles = await invoke<Article[]>(
        "lire_articles_avec_unites", { role });
      setTousArticles(articles);
      const cree = articles.find(
        a => a.nom.toLowerCase() === naNom.trim().toLowerCase());
      setModalNouvelArticle(false);
      // Le stock est à zéro : la vente partira à découvert, ce qui est
      // le comportement voulu (D32 ne restreint que les transferts).
      if (cree) selectionnerArticle(cree);
    } catch (e) {
      await message(`${e}`, { title: "Création impossible", kind: "error" });
    } finally {
      setNaEnCours(false);
    }
  }

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
    setRemisePct("");
    setQuantite("1");
    setRechercheArticle("");
    setArticlesFiltres([]);
  }

  const stockDispo = articleSelectionne && uniteSelectionnee
    ? stockUV(articleSelectionne.stock, uniteSelectionnee.facteur) : 0;
  const qteNum = parseFloat(quantite) || 0;
  // Stock de l'article selectionne, depot par depot.
  const stockAilleurs = articleSelectionne
    ? stockDepots.filter(sd =>
        sd.article_id === articleSelectionne.id
        && sd.depot_id !== depotActif?.id
        && sd.quantite > 0)
    : [];
  const totalAilleurs = stockAilleurs.reduce((s, x) => s + x.quantite, 0);

  const stockIci = articleSelectionne
    ? (stockDepots.find(sd => sd.article_id === articleSelectionne.id
        && sd.depot_id === depotActif?.id)?.quantite ?? 0)
    : 0;

  const aDecouvert = !!articleSelectionne && qteNum > stockDispo && stockDispo >= 0;
  // Le depot actif ne suffit pas, mais un autre a le complement :
  // c'est le cas ou l'on envoie quelqu'un chercher.
  const completableAilleurs = !!articleSelectionne
    && qteNum > stockIci && qteNum <= stockIci + totalAilleurs;
  // Le modal s'ouvre des qu'un AUTRE depot a du stock, meme si le total
  // ne suffit pas. Sinon la vente entiere etait imputee au depot actif
  // et le stock des autres restait faux — un decouvert etait possible
  // en mono-depot mais pas en multi-depot, ce qui n'a pas de sens :
  // seul le TRANSFERT refuse le decouvert (D32), pas la vente.
  const repartitionNecessaire = !!articleSelectionne
    && qteNum > stockIci && totalAilleurs > 0;
  const enRupture = !!articleSelectionne && articleSelectionne.stock <= 0;

  // Ecart entre le prix de reference et le prix pratique, sur la
  // quantite saisie. Positif = remise, negatif = majoration.
  const ecartUnitaire = uniteSelectionnee
    ? uniteSelectionnee.prix_reference - (parseMontant(prixPratique) || 0)
    : 0;
  const economie   = Math.max(0,  ecartUnitaire) * (parseFloat(quantite) || 1);
  const majoration = Math.max(0, -ecartUnitaire) * (parseFloat(quantite) || 1);

  // Texte affiche au vendeur : « 5 ici · 30 à Djelibougou ».
  const resumeStock = articleSelectionne
    ? [`${stockIci} ici`,
       ...stockAilleurs.map(sd => `${sd.quantite} à ${sd.depot_nom}`)].join(" · ")
    : "";

  // ---- Panier ----
  function ajouterAuPanier() {
    if (!articleSelectionne || !uniteSelectionnee) return;
    const qte = parseFloat(quantite) || 1;
    const prix = parseMontant(prixPratique) || uniteSelectionnee.prix_reference;

    // Le depot actif ne suffit pas mais un autre a le complement :
    // on met la vente EN PAUSE et on demande la repartition. Quelqu'un
    // va chercher la marchandise, puis on valide.
    if (repartitionNecessaire) {
      setRepartition({
        article: articleSelectionne, unite: uniteSelectionnee,
        quantite: qte, prix,
      });
      return;
    }

    setPanier(prev => [...prev, {
      id: genId(), article: articleSelectionne, unite: uniteSelectionnee,
      quantite: qte, prix_pratique: prix, montant: Math.round(prix * qte),
      a_decouvert: qte > stockDispo && articleSelectionne.stock >= 0,
      depot_id: depotActif?.id ?? "",
      depot_nom: depotActif?.nom ?? "",
    }]);
    setArticleSelectionne(null); setUniteSelectionnee(null);
    setQuantite("1"); setPrixPratique(""); setRemisePct("");
    inputArticleRef.current?.focus();
  }

  function viderPanier() {
    setPanier([]); setClient(clientGenerique);
    setModeReglement("comptant"); setAcompte("");
    setAvoirAAppliquer(0);
  }

  // ---- Avoir ----
  function appliquerAvoir() {
    const montantAvoir = Math.min(totalAvoirs, total);
    setAvoirAAppliquer(montantAvoir);
  }

  function retirerAvoir() {
    setAvoirAAppliquer(0);
  }

  // ---- Confirmation vente ----
  async function handleConfirmerVente() {
    if (!client || !depotActif) return;
    setChargementVente(true);

    try {
      const role = UTILISATEUR_ACTIF?.role ?? "employe";
      const lignes = panier.map(l => ({
        article_id: l.article.id,
        unite_vente_id: l.unite.id,
        // Depot de la ligne, pas le depot actif : une vente repartie
        // puise dans plusieurs depots.
        depot_source_id: l.depot_id || depotActif.id,
        source_approvisionnement: "stock",
        quantite: l.quantite,
        facteur: l.unite.facteur,
        // Envoyes en TTC : le backend extrait la TVA du TTC (D8).
        prix_reference: Math.round(
          l.unite.prix_reference * (1 + (l.article.taux_tva_defaut ?? 0))
        ),
        prix_pratique: puTTC(l),
        taux_tva: l.article.taux_tva_defaut ?? 0.0,
      }));

      const { vente_id } = await invoke<CreerVenteResultat>("creer_vente", {
        clientId: client.id, depotId: depotActif.id,
        modeReglement, lignes, utilisateurRole: role,
        // Reglement dans la MEME transaction que la vente : plus de
        // fenetre ou le stock est sorti sans que le paiement soit passe.
        montantPaye: modeReglement === "comptant" ? totalApresAvoir : acompteNum,
        modePaiement: modeReglement === "comptant" ? modePaiementComptant : modeAcompte,
        avoirMontant: avoirAAppliquer > 0 ? avoirAAppliquer : null,
      });

      // Cheque : le tracer pour pouvoir suivre son encaissement.
      // Non bloquant — la vente est faite, le client est parti.
      const moyenUtilise = modeReglement === "comptant"
        ? modePaiementComptant : modeAcompte;
      const montantCheque = modeReglement === "comptant"
        ? totalApresAvoir : acompteNum;
      if (moyenUtilise === "cheque" && montantCheque > 0 && chequeNumero.trim()) {
        try {
          await invoke("enregistrer_cheque", {
            paiementId: null,
            venteId: vente_id,
            numero: chequeNumero.trim(),
            banque: chequeBanque.trim() || "—",
            tireur: client.nom,
            montant: montantCheque,
            dateEmission: null,
            dateEcheance: null,
          });
        } catch (e) {
          console.error("Cheque non enregistre :", e);
          await message(
            `Vente enregistrée, mais le chèque n'a pas été tracé.\n${e}\n\n` +
            `L'ajouter manuellement dans l'écran Chèques.`,
            { title: "Chèque non tracé", kind: "warning" },
          );
        }
      }

      // Facture commerciale (D16) — seul document de la vente depuis que
      // la table `facture` legacy n'est plus alimentee.
      //   comptant -> statut "validee"   |   credit -> statut "emis"
      // Toujours non bloquant : la vente et le paiement sont deja
      // enregistres, on ne bloque pas le caissier devant son client.
      // Mais l'echec doit se VOIR : sans facture, rien a imprimer.
      try {
        await invoke("creer_facture_depuis_vente", {
          venteId: vente_id,
          clientId: client.id,
          modeReglement,
          utilisateurRole: role,
        });
      } catch (e) {
        console.error("Facture POS non creee :", e);
        await message(
          `Vente enregistrée, mais la facture n'a pas pu être créée.\n${e}`,
          { title: "Facture manquante", kind: "warning" },
        );
      }

      await rechargerArticles();
      setModalConfirmation(false);
      setChequeNumero(""); setChequeBanque("");
      viderPanier();
      setVenteIdPourImpression(vente_id);
      setModalImpression(true);

    } catch (e) {
      setModalConfirmation(false);
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargementVente(false);
    }
  }

  // Confirme la repartition : une ligne de panier PAR depot.
  function validerRepartition(parts: {
    depot_id: string; depot_nom: string; qte: number; a_decouvert: boolean;
  }[]) {
    if (!repartition) return;
    const { article, unite, prix } = repartition;
    // Le decouvert s'impute au depot actif, qui a deja sa propre ligne :
    // deux lignes sur le meme depot passeraient deux fois en base et
    // fausseraient le stock. On les cumule, en gardant le drapeau.
    const cumul = new Map<string, { nom: string; qte: number; dec: boolean }>();
    for (const p of parts) {
      if (p.qte <= 0) continue;
      const e = cumul.get(p.depot_id);
      if (e) {
        e.qte += p.qte;
        e.dec = e.dec || p.a_decouvert;
      } else {
        cumul.set(p.depot_id, { nom: p.depot_nom, qte: p.qte, dec: p.a_decouvert });
      }
    }
    const nouvelles: LignePanier[] = [...cumul.entries()]
      .map(([depot_id, e]) => ({
        id: genId(), article, unite,
        quantite: e.qte, prix_pratique: prix,
        montant: Math.round(prix * e.qte),
        a_decouvert: e.dec,
        depot_id, depot_nom: e.nom,
      }));
    setPanier(prev => [...prev, ...nouvelles]);
    setRepartition(null);
    setArticleSelectionne(null); setUniteSelectionnee(null);
    setQuantite("1"); setPrixPratique("");
    inputArticleRef.current?.focus();
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <ModalRepartition
        demande={repartition}
        depotActif={depotActif}
        stockDepots={stockDepots}
        onAnnuler={() => setRepartition(null)}
        onValider={validerRepartition}
      />

      {/* En-tête */}
      <div className="flex items-center justify-between px-4 h-12 border-b border-border bg-card shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="font-semibold text-sm">Nouvelle vente</h1>
          {/* Indicateur scanner */}
          {scannerActif && (
            <div className="flex items-center gap-1 text-xs text-green-600">
              <Scan className="h-3 w-3" />
              <span>Scanner actif</span>
            </div>
          )}
          {depots.length > 1 ? (
            <Select value={depotActif?.id ?? ""}
              onValueChange={v => {
                const d = depots.find(dep => dep.id === v);
                if (!d) return;
                setDepotActif(d);
                // Le choix fait ici devient le depot actif global :
                // deux selecteurs qui divergent sont une source d'erreur.
                definirDepotActif(d.id);
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
            disabled={estComptant}
            title={estComptant
              ? "Sélectionner ou créer un client pour vendre à crédit"
              : undefined}
            onClick={() => setModeReglement("credit")}>
            Crédit
          </Button>
        </div>
      </div>

      {/* Notification scanner */}
      {scannerNotification && (
        <div className={cn(
          "px-4 py-1.5 text-xs font-medium text-center",
          scannerNotification.startsWith("✓")
            ? "bg-green-50 text-green-700"
            : scannerNotification.startsWith("Article non trouvé")
            ? "bg-red-50 text-red-700"
            : "bg-blue-50 text-blue-700"
        )}>
          {scannerNotification}
        </div>
      )}

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
                <button onClick={() => { setClient(clientGenerique); setAvoirAAppliquer(0); }}
                  className="text-xs text-muted-foreground hover:text-foreground">✕</button>
              )}
              {/* Badge avoir disponible */}
              {totalAvoirs > 0 && (
                <Badge variant="outline" className="text-xs text-green-600 border-green-300 flex items-center gap-1">
                  <Gift className="h-2.5 w-2.5" />
                  Avoir : {fmt(totalAvoirs)}
                </Badge>
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
              {scannerActif && (
                <span className="text-xs text-muted-foreground ml-1">(ou scannez)</span>
              )}
            </Label>
            <div className="relative">
              <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
              <Input ref={inputArticleRef} value={rechercheArticle}
                onChange={e => handleRechercheArticle(e.target.value)}
                placeholder="Rechercher un article..."
                className="pl-8 h-8 text-sm" />
              {rechercheArticle.trim().length > 0 && (
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
                          <Badge variant={a.stock < 0 ? "destructive" : a.stock === 0 ? "outline" : "secondary"}
                            className={cn("text-xs", a.stock === 0 && "text-orange-500 border-orange-300")}>
                            {a.stock < 0 ? "Découvert" : a.stock === 0 ? "Rupture" : fmtQte(a.stock, a.unite_base)}
                          </Badge>
                        </div>
                      </div>
                    </button>
                  ))}
                  {/* Proposé même quand la recherche trouve : « Sucre »
                      existe peut-être en sac alors qu'on vend du détail. */}
                  <button onClick={ouvrirNouvelArticle}
                    className="w-full text-left px-3 py-2 text-sm border-t border-border
                               hover:bg-accent transition-colors flex items-center gap-2
                               text-primary sticky bottom-0 bg-card">
                    <PackagePlus className="h-3.5 w-3.5 shrink-0" />
                    Créer « {rechercheArticle.trim()} »
                  </button>
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
              {/* Ou est la marchandise. Sans cette ligne, le vendeur
                  refuse une vente qu'il pouvait honorer, ou promet une
                  quantite qui n'existe nulle part. */}
              {stockAilleurs.length > 0 && (
                <p className="text-xs text-muted-foreground flex items-center gap-1">
                  <Warehouse className="h-3 w-3 shrink-0" /> {resumeStock}
                </p>
              )}
              {completableAilleurs && (
                <p className="text-xs text-blue-600 flex items-center gap-1">
                  <Warehouse className="h-3 w-3 shrink-0" />
                  Complément disponible ailleurs — la répartition sera demandée
                </p>
              )}
              {repartitionNecessaire && !completableAilleurs && (
                <p className="text-xs text-orange-500 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3 shrink-0" />
                  Stock insuffisant partout — répartition puis découvert
                </p>
              )}
              {aDecouvert && !repartitionNecessaire && (
                <p className="text-xs text-orange-500 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3" /> Vente à découvert autorisée
                </p>
              )}
              {/* Toujours affiché : avec les conditionnements, masquer le
                  choix quand il n'y a qu'une unité faisait vendre un carton
                  au prix d'une pièce sans que personne ne le voie. */}
              {articleSelectionne.unites.length >= 1 && (
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
                {/* La remise n'est pas stockee a part : elle s'exprime en
                    baissant prix_pratique. L'ecart avec prix_reference
                    est deja trace en base. Ce champ evite au vendeur de
                    calculer de tete. */}
                <div>
                  <Label className="text-xs">Remise %</Label>
                  <Input type="number" min="0" max="100" step="1"
                    value={remisePct}
                    onChange={e => {
                      const v = e.target.value;
                      setRemisePct(v);
                      const pct = parseFloat(v) || 0;
                      const ref = uniteSelectionnee?.prix_reference ?? 0;
                      if (pct > 0 && pct <= 100) {
                        setPrixPratique(String(Math.round(ref * (1 - pct / 100))));
                      } else if (v === "" || pct === 0) {
                        setPrixPratique(String(ref));
                      }
                    }}
                    className="h-8 mt-0.5 text-sm text-right"
                    onKeyDown={e => e.key === "Enter" && ajouterAuPanier()} />
                </div>
              </div>
              <div className="flex items-center justify-between">
                <div className="flex flex-col">
                  <span className="text-xs text-muted-foreground">
                    Sous-total : {fmt(Math.round(
                      (parseMontant(prixPratique) || 0) * (parseFloat(quantite) || 1)
                    ))}
                  </span>
                  {economie > 0 && (
                    <span className="text-xs text-green-600">
                      Remise accordée : {fmt(economie)}
                    </span>
                  )}
                  {majoration > 0 && (
                    <span className="text-xs text-orange-500">
                      Prix majoré de {fmt(majoration)}
                    </span>
                  )}
                </div>
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
                {scannerActif && (
                  <p className="text-xs mt-1 flex items-center gap-1">
                    <Scan className="h-3 w-3" /> Scannez un article
                  </p>
                )}
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
                      {ligne.a_decouvert && <span className="text-red-500 ml-1">⚠</span>}
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

            {/* Avoir disponible */}
            {totalAvoirs > 0 && panier.length > 0 && (
              <div className="flex items-center justify-between py-2 px-3 rounded-md
                bg-green-50 dark:bg-green-950/20 border border-green-200">
                <div className="flex items-center gap-2">
                  <Gift className="h-4 w-4 text-green-600" />
                  <div>
                    <p className="text-xs font-medium text-green-700">
                      Avoir disponible : {fmt(totalAvoirs)}
                    </p>
                    {avoirAAppliquer > 0 && (
                      <p className="text-xs text-green-600">
                        Appliqué : {fmt(avoirAAppliquer)}
                      </p>
                    )}
                  </div>
                </div>
                {avoirAAppliquer > 0 ? (
                  <Button size="sm" variant="outline" onClick={retirerAvoir}
                    className="h-7 text-xs border-green-300 text-green-700 hover:bg-green-100">
                    Retirer
                  </Button>
                ) : (
                  <Button size="sm" onClick={appliquerAvoir}
                    className="h-7 text-xs bg-green-600 hover:bg-green-700">
                    Appliquer
                  </Button>
                )}
              </div>
            )}

            {/* Acompte crédit */}
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
              </div>
            )}

            {/* Total */}
            <div className="space-y-1">
              {/* TVA si active */}
              {aTVA && (
                <>
                  <div className="flex items-center justify-between text-xs text-muted-foreground">
                    <span>Sous-total HT</span>
                    <span>{fmt(totalHT)}</span>
                  </div>
                  <div className="flex items-center justify-between text-xs text-blue-600">
                    <span>TVA</span>
                    <span>{fmt(totalTVA)}</span>
                  </div>
                </>
              )}
              {avoirAAppliquer > 0 && (
                <>
                  <div className="flex items-center justify-between text-sm text-muted-foreground">
                    <span>{aTVA ? "Total TTC" : "Sous-total"}</span>
                    <span>{fmt(total)}</span>
                  </div>
                  <div className="flex items-center justify-between text-sm text-green-600">
                    <span>Avoir appliqué</span>
                    <span>- {fmt(avoirAAppliquer)}</span>
                  </div>
                </>
              )}
              <div className="flex items-center justify-between">
                <span className="font-medium">
                  {avoirAAppliquer > 0 ? "Reste à payer" : aTVA ? "Total TTC" : "Total"}
                </span>
                <span className="text-xl font-bold">{fmt(totalApresAvoir)}</span>
              </div>
            </div>

            <Button className="w-full" size="lg"
              disabled={panier.length === 0 || chargementVente}
              onClick={() => modeReglement === "comptant" && totalApresAvoir > 0
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
        onCreer={c => { setTousClients(prev => [...prev, c]); selectionnerClient(c); setModalNouveauClient(false); }} />

      <ModalEncaissement ouvert={modalEncaissement} total={totalApresAvoir}
        chequeNumero={chequeNumero} setChequeNumero={setChequeNumero}
        chequeBanque={chequeBanque} setChequeBanque={setChequeBanque}
        onFermer={() => setModalEncaissement(false)}
        onConfirmer={(mode, _) => {
          setModePaiementComptant(mode);
          setModalEncaissement(false);
          setModalConfirmation(true);
        }} />

      <ModalConfirmation
        ouvert={modalConfirmation} total={total}
        totalApresAvoir={totalApresAvoir} avoirApplique={avoirAAppliquer}
        acompte={modeReglement === "comptant" ? totalApresAvoir : acompteNum}
        modeAcompte={modeReglement === "comptant" ? modePaiementComptant : modeAcompte}
        modeReglement={modeReglement}
        client={client ?? { id: "", code: "", nom: "Comptant" }}
        chargement={chargementVente}
        onFermer={() => setModalConfirmation(false)}
        onConfirmer={handleConfirmerVente} />

      <ModalImpression ouvert={modalImpression}
        venteId={venteIdPourImpression}
        onFermer={() => setModalImpression(false)} />

      <Dialog open={modalNouvelArticle}
        onOpenChange={o => !o && setModalNouvelArticle(false)}>
        <DialogContent className="max-w-sm">
          <DialogHeader><DialogTitle>Nouvel article</DialogTitle></DialogHeader>
          <div className="space-y-3 pt-2">
            <div>
              <Label>Nom *</Label>
              <Input value={naNom} onChange={e => setNaNom(e.target.value)}
                placeholder="Sucre en poudre" autoFocus className="mt-1" />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label>Unité de base *</Label>
                <div className="mt-1">
                  <SelectUnite valeur={naUnite} onChange={setNaUnite} />
                </div>
              </div>
              <div>
                <Label>Prix de vente *</Label>
                <MoneyInput value={naPrix} onChange={setNaPrix}
                  placeholder="0" className="mt-1" />
              </div>
            </div>
            {/* D39 : les conditionnements se déclarent ensuite, avec un
                facteur exprimé dans cette unité. Partir du carton rend
                la vente au détail impossible sans fraction. */}
            <p className="text-xs text-muted-foreground">
              La plus petite unité que vous vendez. Un carton de 12 se
              déclare après, dans Paramètres. Le stock démarre à zéro —
              la vente partira à découvert.
            </p>
            <div className="flex gap-2 pt-1">
              <Button variant="outline" className="flex-1"
                onClick={() => setModalNouvelArticle(false)}>Annuler</Button>
              <Button className="flex-1" onClick={handleCreerArticle}
                disabled={naEnCours || !naNom.trim() || !naUnite.trim() || !naPrix}>
                {naEnCours
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : "Créer et vendre"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}