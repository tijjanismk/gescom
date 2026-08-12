import { Filter, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";

export interface FiltresVentesState {
  recherche: string;
  statut: string;       // "" | "payee" | "creance_ouverte" | "partiellement_payee"
  periode: string;      // "" | "aujourd_hui" | "semaine" | "mois" | "personnalise"
  date_debut: string;
  date_fin: string;
}

export const FILTRES_VENTES_DEFAUT: FiltresVentesState = {
  recherche: "",
  statut: "",
  periode: "",
  date_debut: "",
  date_fin: "",
};

interface FiltresVentesProps {
  filtres: FiltresVentesState;
  onChanger: (f: FiltresVentesState) => void;
  onReset: () => void;
}

export function FiltresVentes({ filtres, onChanger, onReset }: FiltresVentesProps) {
  const actifs = filtres.statut || filtres.periode || filtres.recherche;

  function set(champ: keyof FiltresVentesState, val: string) {
    onChanger({ ...filtres, [champ]: val });
  }

  return (
    <div className="flex flex-wrap items-center gap-2 mb-4">
      <Input
        value={filtres.recherche}
        onChange={e => set("recherche", e.target.value)}
        placeholder="Client..."
        className="h-8 text-sm w-40"
      />

      <Select value={filtres.statut || "tous"}
        onValueChange={v => set("statut", v === "tous" ? "" : v)}>
        <SelectTrigger className="h-8 text-sm w-36">
          <SelectValue placeholder="Statut" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="tous">Tous statuts</SelectItem>
          <SelectItem value="payee">Payée</SelectItem>
          <SelectItem value="creance_ouverte">Créance ouverte</SelectItem>
          <SelectItem value="partiellement_payee">Partiel</SelectItem>
        </SelectContent>
      </Select>

      <Select value={filtres.periode || "tout"}
        onValueChange={v => set("periode", v === "tout" ? "" : v)}>
        <SelectTrigger className="h-8 text-sm w-36">
          <SelectValue placeholder="Période" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="tout">Toute période</SelectItem>
          <SelectItem value="aujourd_hui">Aujourd'hui</SelectItem>
          <SelectItem value="semaine">Cette semaine</SelectItem>
          <SelectItem value="mois">Ce mois</SelectItem>
          <SelectItem value="personnalise">Personnalisé</SelectItem>
        </SelectContent>
      </Select>

      {filtres.periode === "personnalise" && (
        <>
          <Input type="date" value={filtres.date_debut}
            onChange={e => set("date_debut", e.target.value)}
            className="h-8 text-sm w-36" />
          <Input type="date" value={filtres.date_fin}
            onChange={e => set("date_fin", e.target.value)}
            className="h-8 text-sm w-36" />
        </>
      )}

      {actifs && (
        <Button variant="ghost" size="sm" onClick={onReset} className="h-8 text-xs gap-1">
          <X className="h-3 w-3" /> Réinitialiser
        </Button>
      )}
    </div>
  );
}

// =====================================================================
//  Calcul des dates selon la période
// =====================================================================

export function calculerDatesFiltres(filtres: FiltresVentesState): {
  date_debut: string | null;
  date_fin: string | null;
} {
  const auj = new Date();
  const fmt = (d: Date) => d.toISOString().split("T")[0];

  switch (filtres.periode) {
    case "aujourd_hui":
      return { date_debut: fmt(auj), date_fin: fmt(auj) };
    case "semaine": {
      const lundi = new Date(auj);
      lundi.setDate(auj.getDate() - auj.getDay() + 1);
      return { date_debut: fmt(lundi), date_fin: fmt(auj) };
    }
    case "mois": {
      const debut = new Date(auj.getFullYear(), auj.getMonth(), 1);
      return { date_debut: fmt(debut), date_fin: fmt(auj) };
    }
    case "personnalise":
      return {
        date_debut: filtres.date_debut || null,
        date_fin: filtres.date_fin || null,
      };
    default:
      return { date_debut: null, date_fin: null };
  }
}
