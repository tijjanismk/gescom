// components/FiltresAvances.tsx — Filtres avancés inline au-dessus du tableau

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, RotateCcw, ChevronDown, ChevronUp } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";

// =====================================================================
//  Types & constantes
// =====================================================================

export interface FiltresState {
  type_piece: string;
  client_id: string;
  statut: string;
  date_debut: string;
  date_fin: string;
  echeance: string;
  montant_min: string;
  montant_max: string;
  impaye_seulement: boolean;
  en_retard_seulement: boolean;
}

export const FILTRES_VIDES: FiltresState = {
  type_piece: "tous",
  client_id: "tous",
  statut: "tous",
  date_debut: "",
  date_fin: "",
  echeance: "toutes",
  montant_min: "",
  montant_max: "",
  impaye_seulement: false,
  en_retard_seulement: false,
};

function nbFiltresActifs(f: FiltresState): number {
  return Object.entries(f).filter(([k, v]) => {
    if (k === "type_piece" || k === "statut" || k === "client_id")
      return v !== "tous";
    if (k === "echeance") return v !== "toutes";
    if (typeof v === "boolean") return v === true;
    return v !== "";
  }).length;
}

interface Client { id: string; nom: string; code: string; }

const TYPES_CLIENT = [
  { value: "tous",            label: "Tous types"        },
  { value: "devis",           label: "Devis"             },
  { value: "proforma",        label: "Proforma"          },
  { value: "commande_client", label: "Commande"          },
  { value: "bon_livraison",   label: "Bon de livraison"  },
  { value: "facture",         label: "Facture"           },
  { value: "avoir_client",    label: "Avoir"             },
];

const TYPES_FOURNISSEUR = [
  { value: "tous",                     label: "Tous types"          },
  { value: "bon_commande_fournisseur", label: "Bon de commande"     },
  { value: "bon_reception",            label: "Bon de réception"    },
  { value: "facture_fournisseur",      label: "Facture fournisseur" },
  { value: "avoir_fournisseur",        label: "Avoir fournisseur"   },
];

// "accepte" retire : aucune commande ne l'ecrit en base, le filtre ne
// remontait jamais rien. Les statuts ci-dessous sont ceux reellement
// produits par le code.
const STATUTS = [
  { value: "tous",      label: "Tous statuts" },
  { value: "brouillon", label: "Brouillon"    },
  { value: "emis",      label: "Émis"         },
  { value: "transfere", label: "Transféré"    },
  { value: "validee",   label: "Validée (ancien)" },
  { value: "paye",      label: "Payé"         },
  { value: "annule",    label: "Annulé"       },
];

const ECHEANCES = [
  { value: "toutes",        label: "Toutes échéances" },
  { value: "depassee",      label: "Dépassée"         },
  { value: "cette_semaine", label: "Cette semaine"    },
  { value: "ce_mois",       label: "Ce mois"          },
];

// =====================================================================
//  Composant FiltresAvances — barre inline collapsible
// =====================================================================

interface FiltresAvancesProps {
  filtres: FiltresState;
  onChange: (f: FiltresState) => void;
  cote: "client" | "fournisseur";
}

export function FiltresAvances({ filtres, onChange, cote }: FiltresAvancesProps) {
  // Ouverts par defaut : un filtre replie est un filtre qu'on oublie.
  const [ouvert, setOuvert] = useState(true);
  const [clients, setClients] = useState<Client[]>([]);

  const nb = nbFiltresActifs(filtres);
  const types = cote === "client" ? TYPES_CLIENT : TYPES_FOURNISSEUR;

  useEffect(() => {
    if (!ouvert || cote !== "client" || clients.length > 0) return;
    invoke<Client[]>("lire_clients").then(setClients).catch(console.error);
  }, [ouvert, cote]);

  function set<K extends keyof FiltresState>(k: K, v: FiltresState[K]) {
    onChange({ ...filtres, [k]: v });
  }

  function reinitialiser() {
    onChange(FILTRES_VIDES);
  }

  return (
    <div className="border-b border-border">

      {/* ── Ligne d'en-tête cliquable ── */}
      <button
        onClick={() => setOuvert(!ouvert)}
        className="w-full flex items-center gap-3 px-6 py-2
                   hover:bg-muted/30 transition-colors text-left">
        <span className="text-xs font-medium text-muted-foreground">
          Filtres avancés
        </span>
        {nb > 0 && (
          <span className="inline-flex items-center justify-center
                           w-5 h-5 rounded-full bg-primary
                           text-primary-foreground text-[10px] font-bold shrink-0">
            {nb}
          </span>
        )}
        {nb > 0 && (
          <button
            onClick={e => { e.stopPropagation(); reinitialiser(); }}
            className="ml-1 flex items-center gap-1 text-xs text-muted-foreground
                       hover:text-destructive transition-colors">
            <X className="h-3 w-3" /> Effacer
          </button>
        )}
        <span className="ml-auto text-muted-foreground">
          {ouvert
            ? <ChevronUp className="h-3.5 w-3.5" />
            : <ChevronDown className="h-3.5 w-3.5" />
          }
        </span>
      </button>

      {/* ── Panel filtres ── */}
      {ouvert && (
        <div className="px-6 pb-4 pt-3 bg-muted/20 space-y-4">

          {/* Ligne 1 — Type, Client, Statut, Échéance */}
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div>
              <Label className="text-xs mb-1 block">Type de pièce</Label>
              <Select value={filtres.type_piece}
                onValueChange={v => { if (v) set("type_piece", v); }}>
                <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {types.map(t => (
                    <SelectItem key={t.value} value={t.value} className="text-xs">
                      {t.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {cote === "client" && (
              <div>
                <Label className="text-xs mb-1 block">Client</Label>
                <Select value={filtres.client_id}
                  onValueChange={v => { if (v) set("client_id", v); }}>
                  <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="tous" className="text-xs">Tous</SelectItem>
                    {clients.map(c => (
                      <SelectItem key={c.id} value={c.id} className="text-xs">
                        {c.nom}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            <div>
              <Label className="text-xs mb-1 block">Statut</Label>
              <Select value={filtres.statut}
                onValueChange={v => { if (v) set("statut", v); }}>
                <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {STATUTS.map(s => (
                    <SelectItem key={s.value} value={s.value} className="text-xs">
                      {s.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div>
              <Label className="text-xs mb-1 block">Échéance</Label>
              <Select value={filtres.echeance}
                onValueChange={v => { if (v) set("echeance", v); }}>
                <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {ECHEANCES.map(e => (
                    <SelectItem key={e.value} value={e.value} className="text-xs">
                      {e.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Ligne 2 — Dates, Montants, Cases */}
          <div className="flex items-end gap-4 flex-wrap">

            {/* Date création */}
            <div>
              <Label className="text-xs mb-1 block">Date création</Label>
              <div className="flex items-center gap-1.5">
                <Input type="date" value={filtres.date_debut}
                  onChange={e => set("date_debut", e.target.value)}
                  className="h-8 text-xs w-32" />
                <span className="text-xs text-muted-foreground">→</span>
                <Input type="date" value={filtres.date_fin}
                  onChange={e => set("date_fin", e.target.value)}
                  className="h-8 text-xs w-32" />
              </div>
            </div>

            {/* Montant */}
            <div>
              <Label className="text-xs mb-1 block">Montant (F)</Label>
              <div className="flex items-center gap-1.5">
                <Input type="number" value={filtres.montant_min}
                  onChange={e => set("montant_min", e.target.value)}
                  placeholder="Min"
                  className="h-8 text-xs w-24" />
                <span className="text-xs text-muted-foreground">—</span>
                <Input type="number" value={filtres.montant_max}
                  onChange={e => set("montant_max", e.target.value)}
                  placeholder="Max"
                  className="h-8 text-xs w-24" />
              </div>
            </div>

            {/* Cases à cocher */}
            <div className="flex flex-col gap-2 pb-0.5">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox"
                  checked={filtres.impaye_seulement}
                  onChange={e => set("impaye_seulement", e.target.checked)}
                  className="w-3.5 h-3.5 rounded accent-primary" />
                <span className="text-xs text-muted-foreground">
                  Impayés uniquement
                </span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox"
                  checked={filtres.en_retard_seulement}
                  onChange={e => set("en_retard_seulement", e.target.checked)}
                  className="w-3.5 h-3.5 rounded accent-primary" />
                <span className="text-xs text-muted-foreground">
                  En retard uniquement
                </span>
              </label>
            </div>

            {/* Réinitialiser */}
            {nb > 0 && (
              <Button variant="outline" size="sm"
                onClick={reinitialiser}
                className="h-8 text-xs gap-1.5 ml-auto self-end">
                <RotateCcw className="h-3 w-3" />
                Réinitialiser
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
