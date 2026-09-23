// components/EditeurPlafonds.tsx — trois plafonds, vides = aucun (v3, C-3).
//
// Sert au rôle (Paramètres → Rôles) et à la personne (fenêtre
// Permissions). Le serveur juge : cet écran ne fait que saisir.

import { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export interface Plafonds {
  remise_max_pct: number | null;
  remboursement_max: number | null;
  credit_max: number | null;
}

export const SANS_PLAFOND: Plafonds = { remise_max_pct: null, remboursement_max: null, credit_max: null };

const CHAMPS: { cle: keyof Plafonds; libelle: string; unite: string }[] = [
  { cle: "remise_max_pct", libelle: "Remise max.", unite: "%" },
  { cle: "remboursement_max", libelle: "Remboursement max.", unite: "F" },
  { cle: "credit_max", libelle: "Crédit max. par vente", unite: "F" },
];

function texte(v: number | null): string {
  return v === null || v === undefined ? "" : String(v);
}

export function EditeurPlafonds({ valeur, onEnregistrer, nom, aide }: {
  valeur: Plafonds;
  onEnregistrer: (p: Plafonds) => Promise<void>;
  /** Pour nommer les champs (lecteurs d'écran, parcours du banc). */
  nom: string;
  aide?: string;
}) {
  const [saisie, setSaisie] = useState<Record<keyof Plafonds, string>>({
    remise_max_pct: texte(valeur.remise_max_pct),
    remboursement_max: texte(valeur.remboursement_max),
    credit_max: texte(valeur.credit_max),
  });
  const [etat, setEtat] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);

  useEffect(() => {
    setSaisie({
      remise_max_pct: texte(valeur.remise_max_pct),
      remboursement_max: texte(valeur.remboursement_max),
      credit_max: texte(valeur.credit_max),
    });
  }, [valeur.remise_max_pct, valeur.remboursement_max, valeur.credit_max]);

  async function enregistrer() {
    const lire = (s: string, entier: boolean): number | null => {
      const t = s.replace(/\s/g, "").replace(",", ".");
      if (t === "") return null;
      const n = Number(t);
      return Number.isFinite(n) ? (entier ? Math.round(n) : n) : NaN;
    };
    const p: Plafonds = {
      remise_max_pct: lire(saisie.remise_max_pct, false),
      remboursement_max: lire(saisie.remboursement_max, true),
      credit_max: lire(saisie.credit_max, true),
    };
    if (Object.values(p).some(v => v !== null && Number.isNaN(v))) {
      setEtat({ texte: "Un nombre, ou rien pour « pas de plafond ».", erreur: true });
      return;
    }
    setEnCours(true);
    setEtat(null);
    try {
      await onEnregistrer(p);
      setEtat({ texte: "Plafonds enregistrés." });
    } catch (e) {
      setEtat({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="space-y-2" data-testid={`plafonds-${nom}`}>
      <div className="flex flex-wrap items-end gap-2">
        {CHAMPS.map(c => (
          <label key={c.cle} className="text-xs text-muted-foreground">
            {c.libelle}
            <div className="flex items-center gap-1 mt-1">
              <Input value={saisie[c.cle]} inputMode="decimal"
                aria-label={`${c.libelle} ${nom}`} placeholder="aucun"
                onChange={e => setSaisie(s => ({ ...s, [c.cle]: e.target.value }))}
                className="h-8 w-28 text-sm" />
              <span>{c.unite}</span>
            </div>
          </label>
        ))}
        <Button size="sm" variant="outline" onClick={enregistrer} disabled={enCours}
          aria-label={`Enregistrer les plafonds ${nom}`}>
          {enCours ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : "Enregistrer"}
        </Button>
      </div>
      {aide && <p className="text-xs text-muted-foreground">{aide}</p>}
      {etat && (
        <p className={`text-xs ${etat.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">
          {etat.texte}
        </p>
      )}
    </div>
  );
}
