// components/SelectUnite.tsx — Choix de l'unité de base d'un article.

import { useState } from "react";
import { Input } from "@/components/ui/input";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { UNITES_COURANTES, normaliserUnite } from "@/lib/unites";

const AUTRE = "__autre__";

interface Props {
  valeur: string;
  onChange: (v: string) => void;
  /** Passé au champ libre uniquement. */
  onEnter?: () => void;
}

/**
 * Liste les unités courantes, avec « Autre… » qui bascule sur un champ
 * libre — aucune liste ne couvrira toute une quincaillerie.
 *
 * La saisie libre passe par `normaliserUnite` à la sortie du champ :
 * « M2 » devient « m² », et on évite trois orthographes pour la même
 * unité réparties sur trois articles.
 */
export function SelectUnite({ valeur, onChange, onEnter }: Props) {
  const connue = UNITES_COURANTES.some(g => g.unites.includes(valeur));
  const [libre, setLibre] = useState(!!valeur && !connue);

  if (libre) {
    return (
      <div className="space-y-1">
        <Input
          value={valeur}
          onChange={e => onChange(e.target.value)}
          onBlur={() => onChange(normaliserUnite(valeur))}
          onKeyDown={e => e.key === "Enter" && onEnter?.()}
          placeholder="botte, voyage, fût…"
          autoFocus
        />
        <button type="button"
          onClick={() => { setLibre(false); onChange(""); }}
          className="text-xs text-muted-foreground hover:text-foreground
                     transition-colors">
          ← Revenir à la liste
        </button>
      </div>
    );
  }

  return (
    <Select
      value={valeur || undefined}
      onValueChange={v => {
        if (!v) return;
        if (v === AUTRE) { setLibre(true); onChange(""); return; }
        onChange(v);
      }}>
      <SelectTrigger>
        <SelectValue placeholder="Choisir…" />
      </SelectTrigger>
      {/* Liste plate : SelectGroup / SelectLabel ne sont utilisés
          nulle part ailleurs dans le projet, rien ne garantit que
          `ui/select` les exporte. Le nom du groupe est repris dans
          l'intitulé de chaque unité. */}
      <SelectContent>
        {UNITES_COURANTES.flatMap(g =>
          g.unites.map(u => (
            <SelectItem key={u} value={u}>
              {u}
              <span className="text-muted-foreground text-xs ml-2">
                {g.groupe}
              </span>
            </SelectItem>
          ))
        )}
        <SelectItem value={AUTRE}>Autre…</SelectItem>
      </SelectContent>
    </Select>
  );
}
