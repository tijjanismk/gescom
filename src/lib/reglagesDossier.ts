// lib/reglagesDossier.ts
//
// D43 — les réglages du dossier posés par l'assistant (et modifiables
// dans Paramètres → Société) : TVA par défaut des nouveaux articles,
// fond de caisse habituel.

import { useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";

export interface ReglagesDossier {
  tva_pct: number;
  fond_caisse: number;
  taux_proposes: number[];
}

/**
 * À l'ouverture d'une fenêtre de caisse, propose le fond habituel —
 * seulement si le champ est vide : on ne réécrit pas ce que le
 * caissier a déjà tapé. Il reste modifiable : c'est l'argent RÉELLEMENT
 * dans le tiroir qui compte.
 */
export function useFondHabituel(ouvert: boolean, poser: React.Dispatch<React.SetStateAction<string>>) {
  useEffect(() => {
    if (!ouvert) return;
    invoke<ReglagesDossier>("lire_reglages_dossier")
      .then(r => { if (r.fond_caisse > 0) poser(v => v || String(r.fond_caisse)); })
      .catch(() => { /* sans réglage, le champ reste vide comme avant */ });
  }, [ouvert]);
}
