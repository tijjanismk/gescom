// components/MenuColonnes.tsx — le menu « Colonnes » d'une pièce et son
// bandeau (D58). Le même dans les trois fenêtres où une pièce se saisit :
// Nouvelle pièce (Pièces et fiche client) et Modifier la pièce.
//
// Le choix vaut pour la pièce et reste avec elle. « Garder pour les
// prochaines » réécrit le réglage du genre (Paramètres → Documents), au
// droit `parametres:modifier`. Cacher une colonne ne change aucun
// montant : le bandeau dit ce que les colonnes cachées portent.

import { useEffect, useRef, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { Columns3, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { peut } from "@/lib/droits";
import { type Genre, type ReglagesDocuments, LIBELLES_GENRE } from "@/lib/documents";
import {
  type Colonne, type LigneMontant, COLONNES_REGLABLES, LIBELLES_COLONNE, alertesSaisie,
} from "@/lib/colonnes";

export function MenuColonnes({
  colonnes, onBasculer, genre, garder, onGarder, reglagesPrets,
}: {
  colonnes: readonly Colonne[];
  onBasculer: (c: Colonne, montrer: boolean) => void;
  genre: Genre;
  garder: boolean;
  onGarder: (b: boolean) => void;
  /** Le réglage du genre est lu : sans lui, rien à réécrire. */
  reglagesPrets: boolean;
}) {
  const [ouvert, setOuvert] = useState(false);
  const peutRegler = peut("parametres:modifier");
  // Un clic ailleurs ou Échap le ferme : ouvert, il couvre le bas d'une
  // petite fenêtre (Modifier la pièce), son bouton Sauvegarder compris.
  const zone = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!ouvert) return;
    const dehors = (e: MouseEvent) => {
      if (zone.current && !zone.current.contains(e.target as Node)) setOuvert(false);
    };
    const echap = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); setOuvert(false); } };
    document.addEventListener("mousedown", dehors);
    document.addEventListener("keydown", echap, true);
    return () => {
      document.removeEventListener("mousedown", dehors);
      document.removeEventListener("keydown", echap, true);
    };
  }, [ouvert]);
  return (
    <div className="flex justify-end">
      <div className="relative" ref={zone}>
        <Button type="button" variant="outline" size="sm"
          aria-expanded={ouvert} aria-controls="menu-colonnes"
          onClick={() => setOuvert(o => !o)}>
          <Columns3 className="h-4 w-4 mr-1.5" /> Colonnes
        </Button>
        {ouvert && (
          <div id="menu-colonnes" role="group" aria-label="Colonnes de cette pièce"
            className="absolute right-0 z-20 mt-1 w-80 rounded-md border border-border
                       bg-card p-3 shadow-lg space-y-1">
            <p className="text-xs text-muted-foreground mb-1">
              Pour cette pièce, gardé avec elle. Toujours là : désignation, quantité,
              prix, total. Cacher une colonne ne change aucun montant.
            </p>
            {COLONNES_REGLABLES.map(c => (
              <label key={c} className="flex items-center gap-2 py-0.5 text-sm cursor-pointer">
                <input type="checkbox" className="h-4 w-4 accent-primary"
                  checked={colonnes.includes(c)} onChange={e => onBasculer(c, e.target.checked)} />
                {LIBELLES_COLONNE[c]}
              </label>
            ))}
            <label className={cn("flex items-start gap-2 pt-2 mt-1 border-t border-border text-sm",
              peutRegler ? "cursor-pointer" : "opacity-70")}>
              <input type="checkbox" className="h-4 w-4 mt-0.5 accent-primary"
                disabled={!peutRegler || !reglagesPrets}
                checked={garder} onChange={e => onGarder(e.target.checked)} />
              <span>
                Garder pour les prochaines ({LIBELLES_GENRE[genre].toLowerCase()})
                <span className="block text-xs text-muted-foreground">
                  {peutRegler
                    ? "Change le réglage de Paramètres → Documents, à l'enregistrement."
                    : "Réservé à qui peut modifier les Paramètres."}
                </span>
              </span>
            </label>
          </div>
        )}
      </div>
    </div>
  );
}

/** Ce que les colonnes cachées portent, dit avant d'enregistrer. */
export function BandeauColonnes({
  colonnes, lignes, onMontrer,
}: {
  colonnes: readonly Colonne[];
  lignes: readonly LigneMontant[];
  onMontrer: (c: Colonne) => void;
}) {
  const alertes = alertesSaisie(colonnes, lignes);
  if (alertes.tvaCachee === 0 && alertes.remiseCachee === 0) return null;
  const nombre = (n: number) => (n === 1 ? "1 ligne porte" : `${n} lignes portent`);
  return (
    <div role="status"
      className="flex items-start gap-3 rounded-md border border-border border-l-4
                 border-l-orange-600 bg-card px-4 py-3 text-sm">
      <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0 text-orange-700" />
      <div className="flex-1 space-y-1">
        {alertes.tvaCachee > 0 && (
          <p>
            {nombre(alertes.tvaCachee)} une TVA que la colonne cachée ne montre pas ; le
            total en tient compte.
          </p>
        )}
        {alertes.remiseCachee > 0 && (
          <p>
            {nombre(alertes.remiseCachee)} une remise que la colonne cachée ne montre pas ;
            le papier la dit sous la désignation.
          </p>
        )}
      </div>
      <div className="flex flex-col gap-1.5 shrink-0">
        {alertes.tvaCachee > 0 && (
          <Button type="button" size="sm" variant="outline"
            onClick={() => onMontrer("tva")}>Montrer la TVA</Button>
        )}
        {alertes.remiseCachee > 0 && (
          <Button type="button" size="sm" variant="outline"
            onClick={() => onMontrer("remise")}>Montrer la remise</Button>
        )}
      </div>
    </div>
  );
}

/**
 * « Garder pour les prochaines » : réécrit le réglage du genre, après
 * que la pièce est enregistrée. Un refus ne défait pas la pièce : il
 * rend son message, que la fenêtre dit.
 */
export async function garderPourLeGenre(
  reglages: ReglagesDocuments | null,
  genre: Genre,
  colonnes: readonly Colonne[],
): Promise<string | null> {
  const reglage = reglages?.genres[genre];
  if (!reglage || !peut("parametres:modifier")) return null;
  try {
    await invoke("enregistrer_reglage_document", {
      genre,
      reglage: { ...reglage, colonnes, signatures: reglage.signatures.map(s => ({ libelle: s.libelle })) },
    });
    return null;
  } catch (e) {
    return `La pièce est enregistrée, mais les colonnes ne sont pas gardées pour les prochaines : ${e}`;
  }
}
