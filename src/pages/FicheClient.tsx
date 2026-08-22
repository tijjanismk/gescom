import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft, User, Phone, MapPin, Mail, FileText,
  Loader2, Plus, Printer, ChevronRight, ArrowRight,
  Receipt, Package, Truck, ClipboardList, Gift,
  AlertTriangle, TrendingUp, Clock,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { genererPieceHTML } from "@/lib/genererPDF";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface Client {
  id: string; code: string; nom: string;
  telephone?: string; adresse?: string; nif?: string; email?: string;
  cree_le: string;
}
interface Stats {
  ca_total: number; nb_ventes: number; encours: number;
  avoirs_total: number; nb_pieces: number; derniere_vente?: string;
}
interface Piece {
  id: string; type_piece: string; numero: string; statut: string;
  date_piece: string; date_echeance?: string;
  total_ht: number; total_net: number; total_ttc: number;
  remise_globale: number; remise_montant: number;
  note?: string; piece_origine_id?: string;
}
interface Article {
  id: string; nom: string; unite_base: string;
  unites: { id: string; libelle: string; facteur: number; prix_reference: number }[];
}
interface LignePieceInput {
  article_id: string; unite_vente_id: string;
  article_nom: string; unite_libelle: string;
  quantite: number; prix_unitaire: number;
  remise_pct: number; taux_tva: number;
}
interface Avoir {
  id: string; montant: number; statut: string; cree_le: string;
}
interface CreanceVente {
  vente_id: string; numero_facture?: string; date_vente: string; reste: number;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

const LABELS_TYPE: Record<string, string> = {
  devis:           "Devis",
  proforma:        "Proforma",
  commande_client: "Commande",
  bon_livraison:   "Bon de livraison",
  facture:         "Facture",
  facture_acompte: "Facture d'acompte",
  avoir_client:    "Avoir",
};
const ICONES_TYPE: Record<string, React.ElementType> = {
  devis:           ClipboardList,
  proforma:        ClipboardList,
  commande_client: Package,
  bon_livraison:   Truck,
  facture:         Receipt,
  facture_acompte: Receipt,
  avoir_client:    Gift,
};
const COULEURS_STATUT: Record<string, string> = {
  brouillon:           "bg-gray-100 text-gray-700",
  emis:                "bg-blue-100 text-blue-700",
  accepte:             "bg-green-100 text-green-700",
  refuse:              "bg-red-100 text-red-700",
  partiellement_livre: "bg-orange-100 text-orange-700",
  livre:               "bg-teal-100 text-teal-700",
  facture:             "bg-purple-100 text-purple-700",
  paye:                "bg-green-100 text-green-800",
  annule:              "bg-gray-100 text-gray-500",
};
const LABELS_STATUT: Record<string, string> = {
  brouillon:           "Brouillon",
  emis:                "Émis",
  accepte:             "Accepté",
  refuse:              "Refusé",
  partiellement_livre: "Part. livré",
  livre:               "Livré",
  facture:             "Facturé",
  paye:                "Payé",
  annule:              "Annulé",
};
const CONVERSIONS: Record<string, string> = {
  devis:           "commande_client",
  proforma:        "commande_client",
  commande_client: "bon_livraison",
  bon_livraison:   "facture",
  facture:         "avoir_client",
};
const ONGLETS_PIECES = [
  { key: "tous",           label: "Tout",          filtre: undefined },
  { key: "devis",          label: "Devis/Proforma", filtre: "devis" },
  { key: "commande",       label: "Commandes",      filtre: "commande_client" },
  { key: "bon_livraison",  label: "BL",             filtre: "bon_livraison" },
  { key: "facture",        label: "Factures",       filtre: "facture" },
  { key: "avoir_client",   label: "Avoirs",         filtre: "avoir_client" },
];

// =====================================================================
//  Modal : Nouvelle pièce
// =====================================================================

function ModalNouvellePiece({
  ouvert, clientId, onFermer, onCree,
}: {
  ouvert: boolean; clientId: string;
  onFermer: () => void; onCree: () => void;
}) {
  const [typePiece, setTypePiece] = useState("devis");
  const [articles, setArticles] = useState<Article[]>([]);
  const [lignes, setLignes] = useState<LignePieceInput[]>([]);
  const [remiseGlobale, setRemiseGlobale] = useState("0");
  const [note, setNote] = useState("");
  const [dateEcheance, setDateEcheance] = useState("");
  const [chargement, setChargement] = useState(false);
  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<Article[]>([]);

  useEffect(() => {
    if (!ouvert) return;
    invoke<Article[]>("lire_articles_avec_unites", {
      role: UTILISATEUR_ACTIF?.role ?? "employe"
    }).then(setArticles).catch(console.error);
  }, [ouvert]);

  function ajouterLigne(article: Article) {
    const unite = article.unites[0];
    setLignes(prev => [...prev, {
      article_id: article.id, unite_vente_id: unite.id,
      article_nom: article.nom, unite_libelle: unite.libelle,
      quantite: 1, prix_unitaire: unite.prix_reference,
      remise_pct: 0, taux_tva: 0,
    }]);
    setRechercheArticle(""); setArticlesFiltres([]);
  }

  function supprimerLigne(i: number) {
    setLignes(prev => prev.filter((_, idx) => idx !== i));
  }

  function modifierLigne(i: number, champ: string, val: any) {
    setLignes(prev => prev.map((l, idx) => idx === i ? { ...l, [champ]: val } : l));
  }

  async function handleCreer() {
    if (lignes.length === 0) return;
    setChargement(true);
    try {
      await invoke("creer_piece", {
        clientId: clientId,
        typePiece: typePiece,
        lignes: lignes.map(l => ({
          article_id: l.article_id,
          unite_vente_id: l.unite_vente_id,
          quantite: l.quantite,
          prix_unitaire: l.prix_unitaire,
          remise_pct: l.remise_pct,
          taux_tva: l.taux_tva,
        })),
        remiseGlobale: parseFloat(remiseGlobale) || 0,
        dateEcheance: dateEcheance || null,
        note: note || null,
        pieceOrigineId: null,
      });
      setLignes([]); setNote(""); setRemiseGlobale("0"); setDateEcheance("");
      onCree();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  const totalHT = lignes.reduce((s, l) => {
    const brut = Math.round(l.prix_unitaire * l.quantite);
    return s + brut - Math.round(brut * l.remise_pct / 100);
  }, 0);
  const remiseMt = Math.round(totalHT * (parseFloat(remiseGlobale) || 0) / 100);
  const totalNet = totalHT - remiseMt;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-5xl w-[92vw] h-[88vh] flex flex-col p-0 gap-0">
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle>Nouvelle pièce commerciale</DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-auto px-6 py-4 space-y-4">

          {/* Type + Date + Note */}
          <div className="grid grid-cols-3 gap-3">
            <div>
              <Label className="text-xs">Type de pièce</Label>
              <Select value={typePiece} onValueChange={v => { if (v) setTypePiece(v); }}>
                <SelectTrigger className="mt-1 h-9"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="devis">Devis</SelectItem>
                  <SelectItem value="proforma">Proforma</SelectItem>
                  <SelectItem value="commande_client">Commande client</SelectItem>
                  <SelectItem value="bon_livraison">Bon de livraison</SelectItem>
                  <SelectItem value="facture">Facture</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label className="text-xs">Date d'échéance (optionnel)</Label>
              <Input type="date" value={dateEcheance}
                onChange={e => setDateEcheance(e.target.value)}
                className="mt-1 h-9" />
            </div>
            <div>
              <Label className="text-xs">Note (optionnel)</Label>
              <Input value={note} onChange={e => setNote(e.target.value)}
                placeholder="Conditions, délais..." className="mt-1 h-9" />
            </div>
          </div>

          {/* Recherche article */}
          <div className="relative">
            <Input value={rechercheArticle}
              onChange={e => {
                setRechercheArticle(e.target.value);
                setArticlesFiltres(
                  e.target.value.length < 1 ? [] :
                  articles.filter(a =>
                    a.nom.toLowerCase().includes(e.target.value.toLowerCase())
                  ).slice(0, 10)
                );
              }}
              placeholder="🔍 Rechercher un article à ajouter..."
              className="h-9" autoFocus />
            {articlesFiltres.length > 0 && (
              <div className="absolute z-20 w-full mt-1 bg-card border border-border
                              rounded-md shadow-lg max-h-52 overflow-auto">
                {articlesFiltres.map(a => (
                  <button key={a.id} onClick={() => ajouterLigne(a)}
                    className="w-full text-left px-4 py-2.5 text-sm hover:bg-accent
                               flex items-center justify-between gap-4 border-b border-border/50
                               last:border-0">
                    <span className="font-medium">{a.nom}</span>
                    <span className="text-muted-foreground text-xs shrink-0">
                      {fmt(a.unites[0].prix_reference)} / {a.unite_base}
                      {a.stock >= 0 && (
                        <span className={`ml-3 ${a.stock <= 0 ? "text-red-500" : "text-green-600"}`}>
                          Stock : {a.stock % 1 === 0 ? a.stock : a.stock.toFixed(2)} {a.unite_base}
                        </span>
                      )}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Tableau des lignes */}
          {lignes.length === 0 ? (
            <div className="border-2 border-dashed border-border rounded-lg py-12
                            text-center text-muted-foreground">
              <p className="text-sm">Aucun article — recherchez et ajoutez des articles ci-dessus</p>
            </div>
          ) : (
            <div className="border border-border rounded-md overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-muted/80 text-xs">
                  <tr>
                    <th className="text-left px-4 py-2.5 w-[30%]">Article</th>
                    <th className="text-center px-2 py-2.5 w-[12%]">Unité</th>
                    <th className="text-right px-2 py-2.5 w-[10%]">Qté</th>
                    <th className="text-right px-2 py-2.5 w-[16%]">Prix unitaire (F)</th>
                    <th className="text-right px-2 py-2.5 w-[10%]">Remise %</th>
                    <th className="text-right px-2 py-2.5 w-[16%]">Montant (F)</th>
                    <th className="px-2 py-2.5 w-[6%]"></th>
                  </tr>
                </thead>
                <tbody>
                  {lignes.map((l, i) => {
                    const brut = Math.round(l.prix_unitaire * l.quantite);
                    const remise = Math.round(brut * l.remise_pct / 100);
                    const montant = brut - remise;
                    return (
                      <tr key={i} className="border-t border-border hover:bg-muted/20">
                        <td className="px-4 py-2">
                          <p className="font-medium">{l.article_nom}</p>
                        </td>
                        <td className="px-2 py-2 text-center">
                          <p className="text-xs text-muted-foreground">{l.unite_libelle}</p>
                        </td>
                        <td className="px-2 py-2 text-right">
                          <input type="number" min="0.01" step="0.01"
                            value={l.quantite}
                            onChange={e => modifierLigne(i, "quantite", parseFloat(e.target.value) || 1)}
                            className="w-20 h-8 text-right text-sm border border-border
                                       rounded px-2 bg-background focus:outline-none
                                       focus:ring-1 focus:ring-primary" />
                        </td>
                        <td className="px-2 py-2 text-right">
                          <input type="number" min="0"
                            value={l.prix_unitaire}
                            onChange={e => modifierLigne(i, "prix_unitaire", parseInt(e.target.value) || 0)}
                            className="w-28 h-8 text-right text-sm border border-border
                                       rounded px-2 bg-background focus:outline-none
                                       focus:ring-1 focus:ring-primary" />
                        </td>
                        <td className="px-2 py-2 text-right">
                          <div className="relative inline-block">
                            <input type="number" min="0" max="100" step="1"
                              value={l.remise_pct}
                              onChange={e => modifierLigne(i, "remise_pct", parseFloat(e.target.value) || 0)}
                              className="w-16 h-8 text-right text-sm border border-border
                                         rounded pl-2 pr-5 bg-background focus:outline-none
                                         focus:ring-1 focus:ring-primary" />
                            <span className="absolute right-1.5 top-2 text-xs text-muted-foreground">%</span>
                          </div>
                        </td>
                        <td className="px-2 py-2 text-right">
                          <p className="font-semibold">{fmt(montant)}</p>
                          {remise > 0 && (
                            <p className="text-xs text-orange-500">−{fmt(remise)}</p>
                          )}
                        </td>
                        <td className="px-2 py-2 text-center">
                          <button onClick={() => supprimerLigne(i)}
                            className="w-6 h-6 rounded-full flex items-center justify-center
                                       text-muted-foreground hover:bg-destructive/10
                                       hover:text-destructive transition-colors mx-auto">
                            ✕
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          {/* Remise globale + totaux */}
          {lignes.length > 0 && (
            <div className="flex items-center justify-between gap-6 pt-2">
              <div className="flex items-center gap-3">
                <Label className="text-sm shrink-0">Remise globale</Label>
                <div className="relative w-24">
                  <Input type="number" min="0" max="100" step="1"
                    value={remiseGlobale}
                    onChange={e => setRemiseGlobale(e.target.value)}
                    className="h-8 pr-6 text-sm" />
                  <span className="absolute right-2.5 top-2 text-xs text-muted-foreground">%</span>
                </div>
              </div>
              <div className="text-right space-y-1">
                {(remiseMt > 0 || totalHT !== totalNet) && (
                  <p className="text-sm text-muted-foreground">Brut : {fmt(totalHT)}</p>
                )}
                {remiseMt > 0 && (
                  <p className="text-sm text-orange-600">Remise : −{fmt(remiseMt)}</p>
                )}
                <p className="text-xl font-bold">Net : {fmt(totalNet)}</p>
              </div>
            </div>
          )}
        </div>

        {/* Pied fixe */}
        <div className="border-t border-border px-6 py-4 flex gap-3 shrink-0 bg-card">
          <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
          <Button onClick={handleCreer}
            disabled={lignes.length === 0 || chargement} className="flex-1">
            {chargement
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : `Créer ${LABELS_TYPE[typePiece] ?? typePiece}`
            }
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Règlement créance
// =====================================================================

function ModalReglementCreance({
  ouvert, creance, onFermer, onRegle,
}: {
  ouvert: boolean; creance: CreanceVente | null;
  onFermer: () => void; onRegle: () => void;
}) {
  const [montant, setMontant] = useState("");
  const [mode, setMode] = useState("especes");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (creance) setMontant(creance.reste.toString());
  }, [creance]);

  async function handleRegler() {
    if (!creance) return;
    setChargement(true);
    try {
      await invoke("regler_creance", {
        venteId: creance.vente_id,
        montant: parseMontant(montant),
        mode,
      });
      onRegle();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Régler créance</DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 text-sm">
            {creance?.numero_facture && (
              <p className="font-medium">{creance.numero_facture}</p>
            )}
            <p className="text-muted-foreground">
              Reste dû : <span className="font-medium text-foreground">
                {creance ? fmt(creance.reste) : ""}
              </span>
            </p>
          </div>
          <div>
            <Label>Montant (F)</Label>
            <MoneyInput value={montant} onChange={setMontant} className="mt-1" autoFocus />
          </div>
          <div>
            <Label>Mode</Label>
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
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleRegler}
              disabled={!montant || chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirmer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  FicheClient — page principale
// =====================================================================

interface FicheClientProps {
  clientId: string;
  onRetour: () => void;
}

export function FicheClient({ clientId, onRetour }: FicheClientProps) {
  const [fiche, setFiche] = useState<{ client: Client; stats: Stats } | null>(null);
  const [pieces, setPieces] = useState<Piece[]>([]);
  const [creances, setCreances] = useState<CreanceVente[]>([]);
  const [avoirs, setAvoirs] = useState<Avoir[]>([]);
  const [chargement, setChargement] = useState(true);
  const [onglet, setOnglet] = useState("resume");
  const [ongletPieces, setOngletPieces] = useState("tous");
  const [modalNouv, setModalNouv] = useState(false);
  const [creanceActive, setCreanceActive] = useState<CreanceVente | null>(null);
  const [impressionEnCours, setImpressionEnCours] = useState<string | null>(null);

  async function charger() {
    setChargement(true);
    try {
      const [ficheData, piecesData, creancesData, avoirsData] = await Promise.all([
        invoke<{ client: Client; stats: Stats }>("lire_fiche_client", { clientId }),
        invoke<Piece[]>("lire_pieces_client", { clientId, typeFiltre: null }),
        invoke<CreanceVente[]>("lire_creances_ouvertes")
          .then(all => all.filter((c: any) =>
            (c.client_id ?? c.vente_id) && true
          )),
        invoke<Avoir[]>("lire_avoirs_client", { clientId }),
      ]);
      setFiche(ficheData);
      setPieces(piecesData);
      // Filtrer les créances du client
      setCreances((creancesData as any[]).filter(c =>
        c.client_id === clientId || c.client_nom === ficheData.client.nom
      ));
      setAvoirs(avoirsData);
    } catch (e) {
      console.error("Erreur fiche client :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, [clientId]);

  async function handleImprimer(piece: Piece) {
    setImpressionEnCours(piece.id);
    try {
      const donnees = await invoke<any>("lire_donnees_piece", { pieceId: piece.id });
      const logo = await invoke<string | null>("lire_logo_base64");
      const html = genererPieceHTML(donnees, logo);
      await invoke("imprimer_piece", { html,
        nomFichier: `${piece.numero.replace(/\//g, "-")}.html` });
    } catch (e) {
      await message(`Erreur impression : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setImpressionEnCours(null); }
  }

  async function handleConvertir(piece: Piece) {
    const suivant = CONVERSIONS[piece.type_piece];
    if (!suivant) return;
    try {
      const result = await invoke<any>("convertir_piece", {
        pieceId: piece.id, nouveauType: suivant,
      });
      await message(
        `${LABELS_TYPE[suivant]} ${result.numero} créé`,
        { title: "Conversion réussie", kind: "info" }
      );
      charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  const piecesFiltrees = ongletPieces === "tous"
    ? pieces
    : pieces.filter(p => {
        const o = ONGLETS_PIECES.find(o => o.key === ongletPieces);
        return o?.filtre === undefined || p.type_piece === o.filtre
          || (ongletPieces === "devis" && p.type_piece === "proforma");
      });

  if (chargement) return (
    <div className="flex-1 flex items-center justify-center">
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
    </div>
  );
  if (!fiche) return null;

  const { client, stats } = fiche;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* En-tête */}
      <div className="flex items-center gap-3 px-6 h-14 border-b border-border bg-card shrink-0">
        <button onClick={onRetour}
          className="p-1.5 rounded-md hover:bg-accent transition-colors">
          <ArrowLeft className="h-4 w-4" />
        </button>
        <div className="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center">
          <User className="h-4 w-4 text-primary" />
        </div>
        <div>
          <p className="font-semibold text-sm">{client.nom}</p>
          <p className="text-xs text-muted-foreground">{client.code}</p>
        </div>
        <div className="ml-auto flex gap-2">
          <Button size="sm" onClick={() => setModalNouv(true)}>
            <Plus className="h-4 w-4 mr-1" /> Nouvelle pièce
          </Button>
        </div>
      </div>

      {/* Onglets principaux */}
      <div className="flex gap-1 px-6 border-b border-border bg-card">
        {[
          { key: "resume", label: "Résumé" },
          { key: "pieces", label: `Pièces (${pieces.length})` },
          { key: "creances", label: `Créances (${creances.length})` },
          { key: "avoirs", label: `Avoirs` },
        ].map(o => (
          <button key={o.key} onClick={() => setOnglet(o.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors ${
              onglet === o.key
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}>
            {o.label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-auto p-6">

        {/* ---- Résumé ---- */}
        {onglet === "resume" && (
          <div className="space-y-6">
            {/* Infos client */}
            <div className="border border-border rounded-lg p-4 space-y-2">
              <p className="text-sm font-medium mb-3">Informations</p>
              {client.telephone && (
                <div className="flex items-center gap-2 text-sm">
                  <Phone className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{client.telephone}</span>
                </div>
              )}
              {client.adresse && (
                <div className="flex items-center gap-2 text-sm">
                  <MapPin className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{client.adresse}</span>
                </div>
              )}
              {client.email && (
                <div className="flex items-center gap-2 text-sm">
                  <Mail className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{client.email}</span>
                </div>
              )}
              {client.nif && (
                <div className="flex items-center gap-2 text-sm">
                  <FileText className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>NIF : {client.nif}</span>
                </div>
              )}
              <p className="text-xs text-muted-foreground pt-1">
                Client depuis le {fmtDate(client.cree_le)}
              </p>
            </div>

            {/* KPIs */}
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {[
                { label: "CA total", val: fmt(stats.ca_total), icone: TrendingUp, color: "text-primary" },
                { label: "Nb ventes", val: stats.nb_ventes.toString(), icone: Receipt, color: "text-blue-600" },
                { label: "Encours", val: fmt(stats.encours), icone: AlertTriangle, color: stats.encours > 0 ? "text-orange-600" : "text-green-600" },
                { label: "Avoirs disponibles", val: fmt(stats.avoirs_total), icone: Gift, color: "text-green-600" },
                { label: "Pièces", val: stats.nb_pieces.toString(), icone: FileText, color: "text-muted-foreground" },
                { label: "Dernière vente", val: stats.derniere_vente ? fmtDate(stats.derniere_vente) : "—", icone: Clock, color: "text-muted-foreground" },
              ].map(k => {
                const Icone = k.icone;
                return (
                  <div key={k.label}
                    className="border border-border rounded-lg p-3 flex items-center gap-3">
                    <Icone className={`h-5 w-5 shrink-0 ${k.color}`} />
                    <div>
                      <p className="text-xs text-muted-foreground">{k.label}</p>
                      <p className="text-sm font-semibold">{k.val}</p>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ---- Pièces ---- */}
        {onglet === "pieces" && (
          <div className="space-y-4">
            {/* Sous-onglets types */}
            <div className="flex gap-1 flex-wrap">
              {ONGLETS_PIECES.map(o => (
                <button key={o.key} onClick={() => setOngletPieces(o.key)}
                  className={`px-3 py-1.5 text-xs rounded-full border transition-colors ${
                    ongletPieces === o.key
                      ? "bg-primary text-primary-foreground border-primary"
                      : "border-border text-muted-foreground hover:border-foreground"
                  }`}>
                  {o.label}
                </button>
              ))}
            </div>

            {piecesFiltrees.length === 0 ? (
              <div className="text-center py-12 text-muted-foreground">
                <FileText className="h-8 w-8 mx-auto mb-2 opacity-30" />
                <p className="text-sm">Aucune pièce dans cette catégorie</p>
                <Button size="sm" variant="outline" className="mt-3"
                  onClick={() => setModalNouv(true)}>
                  <Plus className="h-4 w-4 mr-1" /> Créer une pièce
                </Button>
              </div>
            ) : (
              <div className="space-y-2">
                {piecesFiltrees.map(p => {
                  const Icone = ICONES_TYPE[p.type_piece] ?? FileText;
                  const suivant = CONVERSIONS[p.type_piece];
                  const peutConvertir = suivant &&
                    !["annule", "facture", "paye"].includes(p.statut);
                  return (
                    <div key={p.id}
                      className="border border-border rounded-lg px-4 py-3 flex items-center gap-3">
                      <Icone className="h-5 w-5 text-muted-foreground shrink-0" />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <p className="text-sm font-medium">{p.numero}</p>
                          <Badge className={`text-xs px-1.5 py-0 ${COULEURS_STATUT[p.statut] ?? ""}`}>
                            {LABELS_STATUT[p.statut] ?? p.statut}
                          </Badge>
                          <span className="text-xs text-muted-foreground">
                            {LABELS_TYPE[p.type_piece]}
                          </span>
                        </div>
                        <div className="flex items-center gap-3 mt-0.5">
                          <p className="text-xs text-muted-foreground">
                            {fmtDate(p.date_piece)}
                            {p.date_echeance && ` · éch. ${fmtDate(p.date_echeance)}`}
                          </p>
                          <p className="text-xs font-medium">{fmt(p.total_net)}</p>
                          {p.remise_montant > 0 && (
                            <p className="text-xs text-orange-600">
                              remise − {fmt(p.remise_montant)}
                            </p>
                          )}
                        </div>
                        {p.note && (
                          <p className="text-xs text-muted-foreground italic mt-0.5 truncate">
                            {p.note}
                          </p>
                        )}
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        {peutConvertir && (
                          <Button size="sm" variant="outline"
                            onClick={() => handleConvertir(p)}
                            className="h-7 text-xs gap-1">
                            <ArrowRight className="h-3 w-3" />
                            {LABELS_TYPE[suivant]}
                          </Button>
                        )}
                        <Button size="sm" variant="ghost"
                          onClick={() => handleImprimer(p)}
                          disabled={impressionEnCours === p.id}
                          className="h-7 w-7 p-0">
                          {impressionEnCours === p.id
                            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            : <Printer className="h-3.5 w-3.5" />
                          }
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* ---- Créances ---- */}
        {onglet === "creances" && (
          <div className="space-y-3">
            {creances.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucune créance ouverte
              </p>
            ) : (
              <>
                <div className="bg-orange-50 border border-orange-200 rounded-lg px-4 py-2.5
                                flex justify-between items-center">
                  <span className="text-sm font-medium text-orange-800">Total encours</span>
                  <span className="text-sm font-bold text-orange-900">
                    {fmt(creances.reduce((s, c) => s + c.reste, 0))}
                  </span>
                </div>
                {creances.map(c => (
                  <div key={c.vente_id}
                    className="flex items-center justify-between px-4 py-3
                               border border-border rounded-lg">
                    <div>
                      <p className="text-sm font-medium">
                        {c.numero_facture ?? c.vente_id?.slice(0, 8) ?? "—"}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {fmtDate(c.date_vente)}
                      </p>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-sm font-bold text-orange-600">
                        {fmt(c.reste)}
                      </span>
                      <Button size="sm" variant="outline"
                        onClick={() => setCreanceActive(c)}>
                        Régler
                      </Button>
                    </div>
                  </div>
                ))}
              </>
            )}
          </div>
        )}

        {/* ---- Avoirs ---- */}
        {onglet === "avoirs" && (
          <div className="space-y-3">
            {avoirs.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun avoir
              </p>
            ) : (
              <>
                <div className="bg-green-50 border border-green-200 rounded-lg px-4 py-2.5
                                flex justify-between items-center">
                  <span className="text-sm font-medium text-green-800">
                    Avoirs disponibles
                  </span>
                  <span className="text-sm font-bold text-green-900">
                    {fmt(avoirs.filter(a => a.statut === "ouvert")
                      .reduce((s, a) => s + a.montant, 0))}
                  </span>
                </div>
                {avoirs.map(a => (
                  <div key={a.id}
                    className="flex items-center justify-between px-4 py-3
                               border border-border rounded-lg">
                    <div>
                      <p className="text-xs text-muted-foreground">{fmtDate(a.cree_le)}</p>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className={`text-sm font-bold ${
                        a.statut === "ouvert" ? "text-green-600" : "text-muted-foreground"
                      }`}>
                        {fmt(a.montant)}
                      </span>
                      <Badge variant="outline" className="text-xs">
                        {a.statut === "ouvert" ? "Disponible"
                          : a.statut === "utilise" ? "Utilisé"
                          : "Expiré"}
                      </Badge>
                    </div>
                  </div>
                ))}
              </>
            )}
          </div>
        )}
      </div>

      {/* Modals */}
      <ModalNouvellePiece
        ouvert={modalNouv} clientId={clientId}
        onFermer={() => setModalNouv(false)}
        onCree={() => { setModalNouv(false); charger(); }} />

      <ModalReglementCreance
        ouvert={!!creanceActive} creance={creanceActive}
        onFermer={() => setCreanceActive(null)}
        onRegle={() => { setCreanceActive(null); charger(); }} />
    </div>
  );
}