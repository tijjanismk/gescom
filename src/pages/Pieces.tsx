import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Plus, Printer, Loader2, Search, X,
  ArrowRight, FileText, ClipboardList,
  Package, Truck, Receipt, Gift,
  ShoppingBag, CheckCircle2, Copy, Ban, Edit2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { genererPieceHTML, genererTicketThermique } from "@/lib/genererPDF";
import {
  FiltresAvances, FILTRES_VIDES, type FiltresState,
} from "@/components/FiltresAvances";
import { ModalNouvellePiece } from "@/components/ModalNouvellePiece";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface Piece {
  id: string; type_piece: string; numero: string; statut: string;
  date_piece: string; date_echeance?: string;
  tiers_nom: string; tiers_code: string; tiers_id: string;
  total_ht: number; total_net: number; total_ttc: number;
  remise_globale: number; remise_montant: number;
  note?: string; auteur_nom?: string; piece_origine_id?: string;
}

// =====================================================================
//  Constantes
// =====================================================================

const TYPES_PILLS_CLIENT = [
  { value: "tous",            label: "Tout",       icone: FileText      },
  { value: "devis",           label: "Devis/Pro",  icone: ClipboardList },
  { value: "commande_client", label: "Commandes",  icone: Package       },
  { value: "bon_livraison",   label: "BL",         icone: Truck         },
  { value: "facture",         label: "Factures",   icone: Receipt       },
  { value: "avoir_client",    label: "Avoirs",     icone: Gift          },
];

const TYPES_PILLS_FOURNISSEUR = [
  { value: "tous",                     label: "Tout",       icone: FileText  },
  { value: "bon_commande_fournisseur", label: "Bons cmd.",  icone: Package   },
  { value: "bon_reception",            label: "Réceptions", icone: Truck     },
  { value: "facture_fournisseur",      label: "Factures",   icone: Receipt   },
  { value: "avoir_fournisseur",        label: "Avoirs",     icone: Gift      },
];

const CONVERSIONS_CLIENT: Record<string, { type: string; label: string }> = {
  devis:           { type: "commande_client", label: "→ Commande" },
  proforma:        { type: "commande_client", label: "→ Commande" },
  commande_client: { type: "facture",         label: "→ Facture"  },
  bon_livraison:   { type: "facture",         label: "→ Facture"  },
};

const CONVERSIONS_FOURNISSEUR: Record<string, { type: string; label: string }> = {
  bon_commande_fournisseur: { type: "bon_reception",       label: "→ Réception"        },
  bon_reception:            { type: "facture_fournisseur", label: "→ Facture fourn."   },
};

const LABELS_TYPE: Record<string, string> = {
  devis:                    "Devis",
  proforma:                 "Proforma",
  commande_client:          "Commande",
  bon_livraison:            "Bon livraison",
  facture:                  "Facture",
  avoir_client:             "Avoir client",
  bon_commande_fournisseur: "Bon commande",
  bon_reception:            "Bon réception",
  facture_fournisseur:      "Facture fourn.",
  avoir_fournisseur:        "Avoir fourn.",
};

const LABELS_TYPE_CREATION_CLIENT: Record<string, string> = {
  devis:                    "Devis",
  proforma:                 "Proforma",
  commande_client:          "Commande client",
  bon_livraison:            "Bon de livraison",
  facture:                  "Facture",
};

const LABELS_TYPE_CREATION_FOURNISSEUR: Record<string, string> = {
  bon_commande_fournisseur: "Bon de commande",
  bon_reception:            "Bon de réception",
  facture_fournisseur:      "Facture fournisseur",
};

const COULEURS_STATUT: Record<string, string> = {
  brouillon: "bg-gray-100 text-gray-600",
  emis:      "bg-blue-100 text-blue-700",
  accepte:   "bg-green-100 text-green-700",
  refuse:    "bg-red-100 text-red-600",
  transfere: "bg-purple-100 text-purple-600",
  validee:   "bg-green-100 text-green-800",
  paye:      "bg-green-100 text-green-800",
  annule:    "bg-gray-100 text-gray-400",
};

const LABELS_STATUT: Record<string, string> = {
  brouillon: "Brouillon",
  emis:      "Émis",
  accepte:   "Accepté",
  refuse:    "Refusé",
  transfere: "Transféré",
  validee:   "Validée",
  paye:      "Payé",
  annule:    "Annulé",
};

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Modal : Valider une facture → Vente
// =====================================================================

function ModalValiderFacture({
  ouvert, piece, onFermer, onValide,
}: {
  ouvert: boolean; piece: Piece | null;
  onFermer: () => void; onValide: () => void;
}) {
  const [modeReglement, setModeReglement] = useState<"comptant"|"credit">("comptant");
  const [modePaiement, setModePaiement] = useState("especes");
  const [acompte, setAcompte] = useState("");
  const [chargement, setChargement] = useState(false);
  const [resultat, setResultat] = useState<any>(null);

  useEffect(() => {
    if (ouvert) {
      setModeReglement("comptant");
      setModePaiement("especes");
      setAcompte("");
      setResultat(null);
    }
  }, [ouvert]);

  async function handleValider() {
    if (!piece) return;
    setChargement(true);
    try {
      const role = UTILISATEUR_ACTIF?.role ?? "employe";
      const res = await invoke<any>("valider_facture", {
        pieceId: piece.id,
        modeReglement,
        modePaiement,
        acompte: modeReglement === "credit" ? parseMontant(acompte) : null,
        utilisateurRole: role,
      });
      setResultat(res);
      setTimeout(() => { onValide(); }, 1500);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  if (!piece) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Receipt className="h-4 w-4" /> Valider la facture
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          {resultat ? (
            <div className="flex items-center gap-3 p-4 rounded-lg
                            bg-green-50 border border-green-200">
              <CheckCircle2 className="h-6 w-6 text-green-600 shrink-0" />
              <div>
                <p className="text-sm font-medium text-green-800">
                  Vente créée — {resultat.numero_facture}
                </p>
                <p className="text-xs text-green-600">
                  {fmt(resultat.total_net)} ·{" "}
                  {resultat.statut_vente === "payee" ? "Payée" : "Créance ouverte"}
                </p>
              </div>
            </div>
          ) : (
            <>
              <div className="bg-muted rounded-md px-3 py-2.5 space-y-1">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{piece.numero}</span>
                  <span className="font-semibold">{fmt(piece.total_net)}</span>
                </div>
                <div className="flex justify-between text-xs text-muted-foreground">
                  <span>{piece.tiers_nom}</span>
                  <span>{fmtDate(piece.date_piece)}</span>
                </div>
              </div>

              <div>
                <Label className="text-xs mb-1.5 block">Règlement</Label>
                <div className="flex gap-2">
                  {(["comptant","credit"] as const).map(m => (
                    <Button key={m} size="sm" className="flex-1"
                      variant={modeReglement === m ? "default" : "outline"}
                      onClick={() => setModeReglement(m)}>
                      {m === "comptant" ? "Comptant" : "Crédit"}
                    </Button>
                  ))}
                </div>
              </div>

              <div>
                <Label className="text-xs mb-1.5 block">
                  {modeReglement === "comptant" ? "Mode paiement" : "Mode acompte"}
                </Label>
                <Select value={modePaiement}
                  onValueChange={v => { if (v) setModePaiement(v); }}>
                  <SelectTrigger className="h-8 text-sm"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="especes">Espèces</SelectItem>
                    <SelectItem value="orange_money">Orange Money</SelectItem>
                    <SelectItem value="moov_money">Moov Money</SelectItem>
                    <SelectItem value="cheque">Chèque</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {modeReglement === "credit" && (
                <div>
                  <Label className="text-xs mb-1.5 block">Acompte (optionnel)</Label>
                  <MoneyInput value={acompte} onChange={setAcompte}
                    placeholder="0" className="h-8" />
                </div>
              )}

              <div className="flex gap-2">
                <Button variant="outline" onClick={onFermer}
                  disabled={chargement} className="flex-1">Annuler</Button>
                <Button onClick={handleValider}
                  disabled={chargement} className="flex-1">
                  {chargement
                    ? <Loader2 className="h-4 w-4 animate-spin" />
                    : "Valider → Vente"}
                </Button>
              </div>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Modifier une pièce (note + échéance)
// =====================================================================

function ModalModifierPiece({
  ouvert, piece, onFermer, onModifie,
}: {
  ouvert: boolean; piece: Piece | null;
  onFermer: () => void; onModifie: () => void;
}) {
  const [note, setNote] = useState(piece?.note ?? "");
  const [dateEcheance, setDateEcheance] = useState(piece?.date_echeance ?? "");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert && piece) {
      setNote(piece.note ?? "");
      setDateEcheance(piece.date_echeance ?? "");
    }
  }, [ouvert, piece]);

  async function handleSauvegarder() {
    if (!piece) return;
    setChargement(true);
    try {
      await invoke("modifier_piece", {
        pieceId: piece.id,
        note: note || null,
        dateEcheance: dateEcheance || null,
        remiseGlobale: null,
        lignes: null,
      });
      onModifie();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  if (!piece) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Edit2 className="h-4 w-4" /> Modifier la pièce
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 text-sm">
            <span className="text-muted-foreground">{piece.numero}</span>
            <span className="ml-2 font-medium">{piece.tiers_nom}</span>
          </div>
          <div>
            <Label className="text-xs mb-1.5 block">Date d'échéance</Label>
            <Input type="date" value={dateEcheance}
              onChange={e => setDateEcheance(e.target.value)} className="h-9" />
          </div>
          <div>
            <Label className="text-xs mb-1.5 block">Note</Label>
            <Input value={note} onChange={e => setNote(e.target.value)}
              placeholder="Conditions, remarques..." className="h-9" />
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleSauvegarder}
              disabled={chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Sauvegarder"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Pièces
// =====================================================================

export function Pieces({ onOuvrirFicheClient, onOuvrirFicheFournisseur }: {
  onOuvrirFicheClient?: (clientId: string) => void;
  onOuvrirFicheFournisseur?: (fournisseurId: string) => void;
}) {
  const [onglet, setOnglet] = useState<"client"|"fournisseur">("client");
  const [pieces, setPieces] = useState<Piece[]>([]);
  const [chargement, setChargement] = useState(true);

  // Filtre rapide (pills) + recherche
  const [filtreTypeRapide, setFiltreTypeRapide] = useState("tous");
  const [recherche, setRecherche] = useState("");

  // Filtres avancés
  const [filtresAvances, setFiltresAvances] = useState<FiltresState>(FILTRES_VIDES);

  // Tri
  const [triCol, setTriCol] = useState<"date"|"numero"|"tiers"|"montant">("date");
  const [triSens, setTriSens] = useState<"desc"|"asc">("desc");

  // Modals
  const [modalNouv, setModalNouv] = useState(false);
  const [factureAValider, setFactureAValider] = useState<Piece | null>(null);
  const [pieceAModifier, setPieceAModifier] = useState<Piece | null>(null);
  const [impressionEnCours, setImpressionEnCours] = useState<string | null>(null);

  // Type effectif
  const filtreTypeEffectif = filtresAvances.type_piece !== "tous"
    ? filtresAvances.type_piece : filtreTypeRapide;

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      let data: Piece[];
      if (onglet === "client") {
        data = await invoke<Piece[]>("lire_toutes_pieces_client", {
          typeFiltre:        filtreTypeEffectif === "tous" ? null : filtreTypeEffectif,
          statut:            filtresAvances.statut === "tous" ? null : filtresAvances.statut,
          recherche:         recherche || null,
          dateDebut:         filtresAvances.date_debut ? filtresAvances.date_debut + "T00:00:00" : null,
          dateFin:           filtresAvances.date_fin   ? filtresAvances.date_fin   + "T23:59:59" : null,
          montantMin:        filtresAvances.montant_min ? parseInt(filtresAvances.montant_min) : null,
          montantMax:        filtresAvances.montant_max ? parseInt(filtresAvances.montant_max) : null,
          impaySeulement:    filtresAvances.impaye_seulement    || null,
          enRetardSeulement: filtresAvances.en_retard_seulement || null,
          clientId:          filtresAvances.client_id !== "tous" ? filtresAvances.client_id : null,
        });
      } else {
        data = await invoke<Piece[]>("lire_toutes_pieces_fournisseur", {
          typeFiltre: filtreTypeEffectif === "tous" ? null : filtreTypeEffectif,
          statut:     filtresAvances.statut === "tous" ? null : filtresAvances.statut,
          recherche:  recherche || null,
          fournisseurId: filtresAvances.client_id !== "tous" ? filtresAvances.client_id : null,
        });
      }
      setPieces(data);
    } catch (e) {
      console.error("Erreur pièces :", e);
      setPieces([]);
    } finally { setChargement(false); }
  }, [onglet, filtreTypeEffectif, filtresAvances, recherche]);

  useEffect(() => {
    setFiltreTypeRapide("tous");
    setFiltresAvances(FILTRES_VIDES);
    setRecherche("");
  }, [onglet]);

  useEffect(() => { charger(); }, [charger]);

  // Tri local
  const piecesTri = [...pieces].sort((a, b) => {
    let va: any, vb: any;
    switch (triCol) {
      case "date":    va = a.date_piece; vb = b.date_piece; break;
      case "numero":  va = a.numero;     vb = b.numero;     break;
      case "tiers":   va = a.tiers_nom;  vb = b.tiers_nom;  break;
      case "montant": va = a.total_net;  vb = b.total_net;  break;
      default:        va = a.date_piece; vb = b.date_piece;
    }
    if (va < vb) return triSens === "asc" ? -1 : 1;
    if (va > vb) return triSens === "asc" ? 1 : -1;
    return 0;
  });

  function toggleTri(col: typeof triCol) {
    if (triCol === col) setTriSens(s => s === "asc" ? "desc" : "asc");
    else { setTriCol(col); setTriSens("desc"); }
  }

  function Th({ col, label, align = "left" }: {
    col: typeof triCol; label: string; align?: string;
  }) {
    const actif = triCol === col;
    return (
      <th onClick={() => toggleTri(col)}
        className={`px-3 py-2.5 text-${align} text-xs font-medium
                    text-muted-foreground cursor-pointer select-none
                    hover:text-foreground whitespace-nowrap`}>
        {label}{actif && <span className="ml-1">{triSens === "asc" ? "↑" : "↓"}</span>}
      </th>
    );
  }

  async function handleDupliquer(p: Piece) {
    try {
      const res = await invoke<any>("dupliquer_piece", { pieceId: p.id });
      await message(`Copie créée — ${res.numero}`, { title: "Dupliqué", kind: "info" });
      charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  async function handleAnnuler(p: Piece) {
    if (!window.confirm(`Annuler ${p.numero} ? Cette action est irréversible.`)) return;
    try {
      await invoke("annuler_piece", { pieceId: p.id, motif: null });
      charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  async function handleImprimer(p: Piece, thermique = false, format: "a4"|"a5" = "a4") {
    setImpressionEnCours(p.id);
    try {
      const [donnees, logo] = await Promise.all([
        invoke<any>("lire_donnees_piece", { pieceId: p.id }),
        invoke<string | null>("lire_logo_base64"),
      ]);
      const html = thermique
        ? genererTicketThermique(donnees, logo)
        : genererPieceHTML(donnees, logo, format);
      await invoke("imprimer_piece", {
        html, nomFichier: `${p.numero.replace(/\//g, "-")}.html`,
      });
    } catch (e) {
      await message(`Erreur impression : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setImpressionEnCours(null); }
  }

  async function handleConvertir(p: Piece) {
    const conv = onglet === "client"
      ? CONVERSIONS_CLIENT[p.type_piece]
      : CONVERSIONS_FOURNISSEUR[p.type_piece];
    if (!conv) return;
    try {
      const res = await invoke<any>("convertir_piece", {
        pieceId: p.id, nouveauType: conv.type,
      });
      await message(`${res.numero} créé`, { title: "Transfert réussi", kind: "info" });
      charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  const totalNet = piecesTri.reduce((s, p) => s + p.total_net, 0);
  const pillsActuels = onglet === "client" ? TYPES_PILLS_CLIENT : TYPES_PILLS_FOURNISSEUR;
  const labelTiers = onglet === "client" ? "Client" : "Fournisseur";

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* ── En-tête ── */}
      <div className="px-6 pt-5 pb-0 bg-card border-b border-border shrink-0">
        <h1 className="text-xl font-semibold mb-4">Pièces commerciales</h1>

        {/* Onglets Client / Fournisseur */}
        <div className="flex gap-1">
          {[
            { key: "client",      label: "Pièces client",      icone: Receipt    },
            { key: "fournisseur", label: "Pièces fournisseur", icone: ShoppingBag },
          ].map(o => {
            const Icone = o.icone;
            return (
              <button key={o.key} onClick={() => setOnglet(o.key as any)}
                className={`flex items-center gap-2 px-5 py-2.5 text-sm
                            font-medium border-b-2 transition-colors -mb-px ${
                  onglet === o.key
                    ? "border-primary text-primary"
                    : "border-transparent text-muted-foreground hover:text-foreground"
                }`}>
                <Icone className="h-4 w-4" />{o.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* ── Barre pills + recherche + CRÉER ── */}
      <div className="px-6 py-2.5 border-b border-border bg-muted/20
                      flex items-center gap-2 flex-wrap shrink-0">

        {/* Pills type */}
        <div className="flex items-center gap-1 flex-wrap">
          {pillsActuels.map(t => {
            const Icone = t.icone;
            const actif = filtreTypeRapide === t.value
              && filtresAvances.type_piece === "tous";
            return (
              <button key={t.value}
                onClick={() => {
                  setFiltreTypeRapide(t.value);
                  setFiltresAvances(prev => ({ ...prev, type_piece: "tous" }));
                }}
                className={`flex items-center gap-1.5 px-2.5 py-1.5
                            rounded-full text-xs font-medium border
                            transition-colors ${
                  actif
                    ? "bg-primary text-primary-foreground border-primary"
                    : "bg-background border-border text-muted-foreground hover:border-foreground"
                }`}>
                <Icone className="h-3 w-3" />{t.label}
              </button>
            );
          })}
        </div>

        <div className="w-px h-5 bg-border" />

        {/* Recherche */}
        <div className="relative">
          <Search className="absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={recherche}
            onChange={e => setRecherche(e.target.value)}
            placeholder={`Numéro, ${labelTiers.toLowerCase()}...`}
            className="h-8 text-xs w-44 pl-8 bg-background" />
          {recherche && (
            <button onClick={() => setRecherche("")}
              className="absolute right-2 top-2 text-muted-foreground">
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        {/* ← BOUTON CRÉER contextuel */}
        <Button size="sm" onClick={() => setModalNouv(true)}
          className="gap-1.5">
          <Plus className="h-3.5 w-3.5" />
          {onglet === "client" ? "Nouvelle pièce client" : "Nouvelle pièce fournisseur"}
        </Button>

        {/* Compteur */}
        <div className="ml-auto flex items-center gap-3 text-xs text-muted-foreground">
          <span>{piecesTri.length} pièce{piecesTri.length > 1 ? "s" : ""}</span>
          {piecesTri.length > 0 && (
            <span className="font-semibold text-foreground">{fmt(totalNet)}</span>
          )}
        </div>
      </div>

      {/* ── Filtres avancés inline ── */}
      <div className="shrink-0">
        <FiltresAvances
          filtres={filtresAvances}
          onChange={f => {
            setFiltresAvances(f);
            if (f.type_piece !== "tous") setFiltreTypeRapide("tous");
          }}
          cote={onglet}
        />
      </div>

      {/* ── Tableau ── */}
      <div className="flex-1 overflow-auto">
        {chargement ? (
          <div className="flex justify-center py-16">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : piecesTri.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full
                          text-muted-foreground gap-3">
            <FileText className="h-10 w-10 opacity-20" />
            <p className="text-sm">Aucune pièce trouvée</p>
            <Button size="sm" variant="outline" onClick={() => setModalNouv(true)}>
              <Plus className="h-3.5 w-3.5 mr-1" />
              Créer une pièce {onglet === "client" ? "client" : "fournisseur"}
            </Button>
          </div>
        ) : (
          <table className="w-full text-sm border-collapse">
            <thead className="sticky top-0 bg-muted/90 backdrop-blur-sm z-10">
              <tr className="border-b border-border">
                <Th col="date"    label="Date" />
                <Th col="numero"  label="Numéro" />
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Type
                </th>
                <Th col="tiers"   label={labelTiers} />
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Statut
                </th>
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Échéance
                </th>
                <Th col="montant" label="Montant" align="right" />
                <th className="px-3 py-2.5 text-xs font-medium text-muted-foreground w-36">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {piecesTri.map((p, i) => {
                const conv = onglet === "client"
                  ? CONVERSIONS_CLIENT[p.type_piece]
                  : CONVERSIONS_FOURNISSEUR[p.type_piece];
                const peutTransferer = conv &&
                  !["transfere","annule","validee","paye"].includes(p.statut);
                const peutValider =
                  p.type_piece === "facture" && p.statut === "brouillon";
                const enRetard = p.date_echeance &&
                  new Date(p.date_echeance) < new Date() &&
                  !["paye","annule","validee"].includes(p.statut);

                return (
                  <tr key={p.id}
                    className={`border-b border-border/50 hover:bg-accent/40
                                transition-colors group ${
                      i % 2 === 1 ? "bg-muted/20" : "bg-background"
                    }`}>

                    <td className="px-3 py-2 text-xs text-muted-foreground whitespace-nowrap">
                      {fmtDate(p.date_piece)}
                    </td>
                    <td className="px-3 py-2 font-mono text-xs font-medium">
                      {p.numero}
                      {p.piece_origine_id && (
                        <span className="ml-1 text-[10px] text-muted-foreground">↗</span>
                      )}
                    </td>
                    <td className="px-3 py-2 text-xs text-muted-foreground">
                      {LABELS_TYPE[p.type_piece] ?? p.type_piece}
                    </td>
                    <td className="px-3 py-2">
                      <button
                        onClick={() => {
                          if (onglet === "client") onOuvrirFicheClient?.(p.tiers_id);
                          else onOuvrirFicheFournisseur?.(p.tiers_id);
                        }}
                        className="text-sm font-medium hover:text-primary
                                   hover:underline text-left transition-colors
                                   truncate max-w-[150px] block">
                        {p.tiers_nom}
                      </button>
                      <p className="text-xs text-muted-foreground">{p.tiers_code}</p>
                    </td>
                    <td className="px-3 py-2">
                      <span className={`inline-flex items-center px-2 py-0.5
                                        rounded-full text-xs font-medium
                                        ${COULEURS_STATUT[p.statut] ?? "bg-gray-100 text-gray-600"}`}>
                        {LABELS_STATUT[p.statut] ?? p.statut}
                      </span>
                    </td>
                    <td className={`px-3 py-2 text-xs whitespace-nowrap ${
                      enRetard ? "text-red-500 font-medium" : "text-muted-foreground"
                    }`}>
                      {p.date_echeance ? fmtDate(p.date_echeance) : "—"}
                      {enRetard && " ⚠"}
                    </td>
                    <td className="px-3 py-2 text-right whitespace-nowrap">
                      <span className="font-semibold text-sm">{fmt(p.total_net)}</span>
                      {p.remise_montant > 0 && (
                        <p className="text-xs text-orange-500 font-normal">
                          −{fmt(p.remise_montant)}
                        </p>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-1">
                        {/* Imprimer A4 — 1 copie */}
                        <button onClick={() => handleImprimer(p, false, "a4")}
                          disabled={impressionEnCours === p.id}
                          title="Imprimer A4"
                          className="p-1.5 rounded hover:bg-muted transition-colors
                                     text-muted-foreground hover:text-foreground">
                          {impressionEnCours === p.id
                            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            : <Printer className="h-3.5 w-3.5" />}
                        </button>
                        {/* A5 */}
                        <button onClick={() => handleImprimer(p, false, "a5")}
                          disabled={impressionEnCours === p.id}
                          title="Imprimer A5"
                          className="p-1.5 rounded hover:bg-muted transition-colors
                                     text-muted-foreground hover:text-foreground
                                     text-[10px] font-bold px-1">
                          A5
                        </button>
                        {/* Ticket thermique 80mm */}
                        <button onClick={() => handleImprimer(p, true)}
                          disabled={impressionEnCours === p.id}
                          title="Ticket thermique 80mm"
                          className="p-1.5 rounded hover:bg-muted transition-colors
                                     text-muted-foreground hover:text-foreground text-[10px]
                                     font-bold leading-none">
                          🧾
                        </button>
                        {peutTransferer && (
                          <button onClick={() => handleConvertir(p)}
                            title={conv!.label}
                            className="flex items-center gap-0.5 px-2 py-1 rounded
                                       text-xs hover:bg-muted transition-colors
                                       text-muted-foreground hover:text-primary
                                       whitespace-nowrap">
                            <ArrowRight className="h-3 w-3" />
                            <span className="hidden group-hover:inline">{conv!.label}</span>
                          </button>
                        )}
                        {peutValider && (
                          <button onClick={() => setFactureAValider(p)}
                            title="Valider → Vente"
                            className="flex items-center gap-0.5 px-2 py-1 rounded
                                       text-xs font-medium border transition-colors
                                       text-green-700 border-green-200
                                       hover:bg-green-50 hover:border-green-400
                                       whitespace-nowrap">
                            ✓ Valider
                          </button>
                        )}

                        {/* Modifier — si pas validée/annulée */}
                        {!["validee","annule","transfere"].includes(p.statut) && (
                          <button onClick={() => setPieceAModifier(p)}
                            title="Modifier"
                            className="p-1.5 rounded hover:bg-muted transition-colors
                                       text-muted-foreground hover:text-blue-600">
                            <Edit2 className="h-3.5 w-3.5" />
                          </button>
                        )}

                        {/* Dupliquer */}
                        <button onClick={() => handleDupliquer(p)}
                          title="Dupliquer"
                          className="p-1.5 rounded hover:bg-muted transition-colors
                                     text-muted-foreground hover:text-foreground">
                          <Copy className="h-3.5 w-3.5" />
                        </button>

                        {/* Annuler — si pas validée/annulée */}
                        {!["validee","annule"].includes(p.statut) && (
                          <button onClick={() => handleAnnuler(p)}
                            title="Annuler"
                            className="p-1.5 rounded hover:bg-muted transition-colors
                                       text-muted-foreground hover:text-red-500">
                            <Ban className="h-3.5 w-3.5" />
                          </button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* ── Modal nouvelle pièce — contextuelle à l'onglet ── */}
      <ModalNouvellePiece
        ouvert={modalNouv}
        cote={onglet}
        onFermer={() => setModalNouv(false)}
        onCree={() => { setModalNouv(false); charger(); }}
      />

      {/* ── Modal modifier pièce ── */}
      <ModalModifierPiece
        ouvert={!!pieceAModifier}
        piece={pieceAModifier}
        onFermer={() => setPieceAModifier(null)}
        onModifie={() => { setPieceAModifier(null); charger(); }}
      />

      {/* ── Modal validation facture ── */}
      <ModalValiderFacture
        ouvert={!!factureAValider}
        piece={factureAValider}
        onFermer={() => setFactureAValider(null)}
        onValide={() => { setFactureAValider(null); charger(); }}
      />
    </div>
  );
}