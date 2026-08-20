import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Plus, Printer, Loader2, Search, X, Filter,
  ChevronDown, ArrowRight, FileText,
  ClipboardList, Package, Truck, Receipt, Gift,
  ShoppingBag, RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { genererPieceHTML } from "@/lib/genererPiece";
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
  note?: string; auteur_nom?: string;
}

// =====================================================================
//  Constantes
// =====================================================================

const TYPES_CLIENT = [
  { value: "tous",            label: "Tous",           icone: FileText      },
  { value: "devis",           label: "Devis",          icone: ClipboardList },
  { value: "proforma",        label: "Proforma",       icone: ClipboardList },
  { value: "commande_client", label: "Commandes",      icone: Package       },
  { value: "bon_livraison",   label: "Bons de livr.",  icone: Truck         },
  { value: "facture",         label: "Factures",       icone: Receipt       },
  { value: "avoir_client",    label: "Avoirs",         icone: Gift          },
];

const TYPES_FOURNISSEUR = [
  { value: "tous",                    label: "Tous",           icone: FileText  },
  { value: "bon_commande_fournisseur",label: "Bons de cmd.",   icone: ShoppingBag },
  { value: "bon_reception",           label: "Bons de récep.", icone: Truck     },
  { value: "facture_fournisseur",     label: "Factures fourn.",icone: Receipt   },
  { value: "avoir_fournisseur",       label: "Avoirs fourn.",  icone: Gift      },
];

const STATUTS = [
  { value: "tous",       label: "Tous statuts"  },
  { value: "brouillon",  label: "Brouillon"     },
  { value: "emis",       label: "Émis"          },
  { value: "accepte",    label: "Accepté"       },
  { value: "livre",      label: "Livré"         },
  { value: "facture",    label: "Facturé"       },
  { value: "paye",       label: "Payé"          },
  { value: "annule",     label: "Annulé"        },
  { value: "refuse",     label: "Refusé"        },
];

const CONVERSIONS: Record<string, string> = {
  devis:                     "commande_client",
  proforma:                  "commande_client",
  commande_client:           "bon_livraison",
  bon_livraison:             "facture",
  bon_commande_fournisseur:  "bon_reception",
  bon_reception:             "facture_fournisseur",
};

const LABELS_TYPE: Record<string, string> = {
  devis:                     "Devis",
  proforma:                  "Proforma",
  commande_client:           "Commande",
  bon_livraison:             "Bon de livraison",
  facture:                   "Facture",
  facture_acompte:           "Facture d'acompte",
  avoir_client:              "Avoir client",
  bon_commande_fournisseur:  "Bon de commande",
  bon_reception:             "Bon de réception",
  facture_fournisseur:       "Facture fournisseur",
  avoir_fournisseur:         "Avoir fournisseur",
};

const COULEURS_STATUT: Record<string, string> = {
  brouillon:  "bg-gray-100 text-gray-600",
  emis:       "bg-blue-100 text-blue-700",
  accepte:    "bg-green-100 text-green-700",
  refuse:     "bg-red-100 text-red-600",
  partiellement_livre: "bg-orange-100 text-orange-700",
  livre:      "bg-teal-100 text-teal-700",
  facture:    "bg-purple-100 text-purple-700",
  paye:       "bg-green-100 text-green-800",
  annule:     "bg-gray-100 text-gray-400",
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

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Page Pièces
// =====================================================================

export function Pieces({ onOuvrirFicheClient }: {
  onOuvrirFicheClient?: (clientId: string) => void;
}) {
  const [onglet, setOnglet] = useState<"client" | "fournisseur">("client");
  const [pieces, setPieces] = useState<Piece[]>([]);
  const [chargement, setChargement] = useState(true);
  const [filtreType, setFiltreType] = useState("tous");
  const [filtreStatut, setFiltreStatut] = useState("tous");
  const [recherche, setRecherche] = useState("");
  const [triColonne, setTriColonne] = useState<"date" | "numero" | "tiers" | "montant">("date");
  const [triSens, setTriSens] = useState<"desc" | "asc">("desc");
  const [impressionEnCours, setImpressionEnCours] = useState<string | null>(null);

  const types = onglet === "client" ? TYPES_CLIENT : TYPES_FOURNISSEUR;

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      // Charger toutes les pièces du bon côté
      // Pour l'instant on charge côté client depuis lire_pieces_client
      // et on filtre localement — la version fournisseur sera ajoutée en v1.1
      if (onglet === "client") {
        const data = await invoke<Piece[]>("lire_toutes_pieces_client", {
          typeFiltre: filtreType === "tous" ? null : filtreType,
          statut: filtreStatut === "tous" ? null : filtreStatut,
          recherche: recherche || null,
        });
        setPieces(data);
      } else {
        // Fournisseur — v1.1
        setPieces([]);
      }
    } catch (e) {
      console.error("Erreur pièces :", e);
      setPieces([]);
    } finally {
      setChargement(false);
    }
  }, [onglet, filtreType, filtreStatut, recherche]);

  useEffect(() => {
    setFiltreType("tous");
    setFiltreStatut("tous");
    setRecherche("");
  }, [onglet]);

  useEffect(() => { charger(); }, [charger]);

  // Tri local
  const piecesTri = [...pieces].sort((a, b) => {
    let va: any, vb: any;
    switch (triColonne) {
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

  function toggleTri(col: typeof triColonne) {
    if (triColonne === col) {
      setTriSens(s => s === "asc" ? "desc" : "asc");
    } else {
      setTriColonne(col);
      setTriSens("desc");
    }
  }

  function enteteCol(col: typeof triColonne, label: string, align = "left") {
    const actif = triColonne === col;
    return (
      <th
        onClick={() => toggleTri(col)}
        className={`px-3 py-2.5 text-${align} text-xs font-medium text-muted-foreground
                    cursor-pointer hover:text-foreground select-none whitespace-nowrap`}>
        {label}
        {actif && (
          <span className="ml-1">{triSens === "asc" ? "↑" : "↓"}</span>
        )}
      </th>
    );
  }

  async function handleImprimer(piece: Piece) {
    setImpressionEnCours(piece.id);
    try {
      const [donnees, logo] = await Promise.all([
        invoke<any>("lire_donnees_piece", { pieceId: piece.id }),
        invoke<string | null>("lire_logo_base64"),
      ]);
      const html = genererPieceHTML(donnees, logo);
      await invoke("imprimer_piece", {
        html,
        nomFichier: `${piece.numero.replace(/\//g, "-")}.html`,
      });
    } catch (e) {
      await message(`Erreur impression : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setImpressionEnCours(null);
    }
  }

  async function handleConvertir(piece: Piece) {
    const suivant = CONVERSIONS[piece.type_piece];
    if (!suivant) return;
    try {
      const result = await invoke<any>("convertir_piece", {
        pieceId: piece.id,
        nouveauType: suivant,
      });
      await message(
        `${LABELS_TYPE[suivant]} N° ${result.numero} créé`,
        { title: "Conversion réussie", kind: "info" }
      );
      charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  // Totaux du filtre courant
  const totalNet = piecesTri.reduce((s, p) => s + p.total_net, 0);
  const nbPieces = piecesTri.length;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* ── En-tête ── */}
      <div className="px-6 pt-5 pb-0 bg-card border-b border-border shrink-0">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-xl font-semibold">Pièces commerciales</h1>
          <Button size="sm" onClick={charger} variant="outline">
            <RefreshCw className="h-3.5 w-3.5 mr-1" /> Actualiser
          </Button>
        </div>

        {/* Onglets Client / Fournisseur */}
        <div className="flex gap-1">
          {[
            { key: "client",      label: "Pièces client",      icone: Receipt    },
            { key: "fournisseur", label: "Pièces fournisseur", icone: ShoppingBag },
          ].map(o => {
            const Icone = o.icone;
            return (
              <button key={o.key}
                onClick={() => setOnglet(o.key as any)}
                className={`flex items-center gap-2 px-5 py-2.5 text-sm font-medium
                            border-b-2 transition-colors -mb-px ${
                  onglet === o.key
                    ? "border-primary text-primary"
                    : "border-transparent text-muted-foreground hover:text-foreground"
                }`}>
                <Icone className="h-4 w-4" />
                {o.label}
              </button>
            );
          })}
        </div>
      </div>

      {/* ── Barre de filtres style Ciel ── */}
      <div className="px-6 py-3 bg-muted/30 border-b border-border
                      flex items-center gap-3 flex-wrap shrink-0">

        {/* Filtre type — boutons pills */}
        <div className="flex items-center gap-1 flex-wrap">
          {types.map(t => {
            const Icone = t.icone;
            return (
              <button key={t.value}
                onClick={() => setFiltreType(t.value)}
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full
                            text-xs font-medium border transition-colors ${
                  filtreType === t.value
                    ? "bg-primary text-primary-foreground border-primary"
                    : "bg-background border-border text-muted-foreground hover:border-foreground"
                }`}>
                <Icone className="h-3 w-3" />
                {t.label}
              </button>
            );
          })}
        </div>

        <div className="h-5 w-px bg-border" />

        {/* Filtre statut */}
        <Select value={filtreStatut} onValueChange={v => { if (v) setFiltreStatut(v); }}>
          <SelectTrigger className="h-8 text-xs w-36 bg-background">
            <Filter className="h-3 w-3 mr-1.5" />
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {STATUTS.map(s => (
              <SelectItem key={s.value} value={s.value} className="text-xs">
                {s.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {/* Recherche */}
        <div className="relative">
          <Search className="absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
            placeholder="N°, client, article..."
            className="h-8 text-xs w-44 pl-8 bg-background" />
          {recherche && (
            <button onClick={() => setRecherche("")}
              className="absolute right-2 top-2 text-muted-foreground hover:text-foreground">
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        {/* Compteur */}
        <div className="ml-auto flex items-center gap-3 text-xs text-muted-foreground">
          <span>{nbPieces} pièce{nbPieces > 1 ? "s" : ""}</span>
          {nbPieces > 0 && (
            <span className="font-medium text-foreground">{fmt(totalNet)}</span>
          )}
        </div>
      </div>

      {/* ── Tableau style Ciel ── */}
      <div className="flex-1 overflow-auto">
        {onglet === "fournisseur" ? (
          <div className="flex flex-col items-center justify-center h-full
                          text-muted-foreground gap-3">
            <ShoppingBag className="h-10 w-10 opacity-20" />
            <p className="text-sm font-medium">Pièces fournisseur</p>
            <p className="text-xs">Disponible en version 1.1</p>
          </div>
        ) : chargement ? (
          <div className="flex justify-center py-16">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : piecesTri.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full
                          text-muted-foreground gap-3">
            <FileText className="h-10 w-10 opacity-20" />
            <p className="text-sm">Aucune pièce trouvée</p>
            {(filtreType !== "tous" || filtreStatut !== "tous" || recherche) && (
              <Button size="sm" variant="outline"
                onClick={() => {
                  setFiltreType("tous");
                  setFiltreStatut("tous");
                  setRecherche("");
                }}>
                Effacer les filtres
              </Button>
            )}
          </div>
        ) : (
          <table className="w-full text-sm border-collapse">
            <thead className="sticky top-0 bg-muted/80 backdrop-blur-sm z-10">
              <tr className="border-b border-border">
                {enteteCol("date",    "Date")}
                {enteteCol("numero",  "Numéro")}
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Type
                </th>
                {enteteCol("tiers",   "Client")}
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Statut
                </th>
                <th className="px-3 py-2.5 text-left text-xs font-medium text-muted-foreground">
                  Échéance
                </th>
                {enteteCol("montant", "Montant", "right")}
                <th className="px-3 py-2.5 text-xs font-medium text-muted-foreground w-28">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {piecesTri.map((p, i) => {
                const suivant = CONVERSIONS[p.type_piece];
                const peutConvertir = suivant &&
                  !["annule", "facture", "paye"].includes(p.statut);
                const estImpair = i % 2 === 1;

                return (
                  <tr key={p.id}
                    className={`border-b border-border/50 hover:bg-accent/50
                                transition-colors group ${
                      estImpair ? "bg-muted/20" : "bg-background"
                    }`}>

                    {/* Date */}
                    <td className="px-3 py-2 text-xs text-muted-foreground whitespace-nowrap">
                      {fmtDate(p.date_piece)}
                    </td>

                    {/* Numéro */}
                    <td className="px-3 py-2 font-mono text-xs font-medium">
                      {p.numero}
                    </td>

                    {/* Type */}
                    <td className="px-3 py-2 text-xs text-muted-foreground">
                      {LABELS_TYPE[p.type_piece] ?? p.type_piece}
                    </td>

                    {/* Client */}
                    <td className="px-3 py-2">
                      <button
                        onClick={() => onOuvrirFicheClient?.(p.tiers_id)}
                        className="text-sm font-medium hover:text-primary
                                   hover:underline text-left truncate max-w-[160px]
                                   transition-colors">
                        {p.tiers_nom}
                      </button>
                      <p className="text-xs text-muted-foreground">{p.tiers_code}</p>
                    </td>

                    {/* Statut */}
                    <td className="px-3 py-2">
                      <span className={`inline-flex items-center px-2 py-0.5
                                        rounded-full text-xs font-medium
                                        ${COULEURS_STATUT[p.statut] ?? "bg-gray-100 text-gray-600"}`}>
                        {LABELS_STATUT[p.statut] ?? p.statut}
                      </span>
                    </td>

                    {/* Échéance */}
                    <td className="px-3 py-2 text-xs text-muted-foreground whitespace-nowrap">
                      {p.date_echeance ? fmtDate(p.date_echeance) : "—"}
                    </td>

                    {/* Montant */}
                    <td className="px-3 py-2 text-right font-semibold text-sm whitespace-nowrap">
                      {fmt(p.total_net)}
                      {p.remise_montant > 0 && (
                        <p className="text-xs text-orange-500 font-normal">
                          − {fmt(p.remise_montant)}
                        </p>
                      )}
                    </td>

                    {/* Actions */}
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-1">
                        {/* Imprimer */}
                        <button
                          onClick={() => handleImprimer(p)}
                          disabled={impressionEnCours === p.id}
                          title="Imprimer"
                          className="p-1.5 rounded hover:bg-muted transition-colors
                                     text-muted-foreground hover:text-foreground">
                          {impressionEnCours === p.id
                            ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            : <Printer className="h-3.5 w-3.5" />
                          }
                        </button>

                        {/* Convertir → pièce suivante */}
                        {peutConvertir && (
                          <button
                            onClick={() => handleConvertir(p)}
                            title={`Convertir en ${LABELS_TYPE[suivant]}`}
                            className="flex items-center gap-0.5 px-2 py-1 rounded
                                       text-xs hover:bg-muted transition-colors
                                       text-muted-foreground hover:text-primary">
                            <ArrowRight className="h-3 w-3" />
                            <span className="hidden group-hover:inline">
                              {LABELS_TYPE[suivant]}
                            </span>
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
    </div>
  );
}
