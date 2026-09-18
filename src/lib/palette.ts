// lib/palette.ts — la palette de commandes : ce qu'on peut faire d'ici.
//
// Un seul raccourci (Ctrl+K) ouvre une liste : aller quelque part, ou
// faire un geste. La liste a deux sources :
//
// - les actions GLOBALES, posées par le Layout : la navigation, les
//   onglets de Paramètres, le compte ;
// - les actions DE LA PAGE, que chaque écran déclare tant qu'il est
//   monté (`useActionsPalette`) : « Nouveau client » n'a de sens que
//   sur Clients, « Ouvrir la caisse » que sur Caisse.
//
// Chaque action dit DE QUOI elle a besoin (`droit`) : la palette ne
// propose pas ce que le serveur refuserait. Comme pour le menu, c'est
// du confort — le refus qui compte est celui du noyau.

import { useEffect } from "react";
import { peut } from "@/lib/droits";

export interface ActionPalette {
  /** Unique dans la liste — sert de clé et d'identité pour le clavier. */
  id: string;
  libelle: string;
  /** Une ligne sous le libellé : où ça mène, ce que ça fait. */
  detail?: string;
  /** Le titre de groupe : « Aller à », « Sur cette page », « Compte ». */
  groupe: string;
  /** Permission requise ; absente = pour tous. */
  droit?: string;
  /** Mots supplémentaires pour la recherche (synonymes, anglais…). */
  motsCles?: string;
  executer: () => void;
}

// ---------------------------------------------------------------------
//  Le registre des actions de page
// ---------------------------------------------------------------------

const parPage = new Map<string, ActionPalette[]>();
const abonnes = new Set<() => void>();

function prevenir() {
  abonnes.forEach((f) => f());
}

/** Les actions de page actuellement déclarées, filtrées par le droit. */
export function actionsDePage(): ActionPalette[] {
  const toutes: ActionPalette[] = [];
  parPage.forEach((liste) => toutes.push(...liste));
  return toutes.filter((a) => !a.droit || peut(a.droit));
}

/** Être prévenu quand une page déclare ou retire ses actions. */
export function surChangementActions(f: () => void): () => void {
  abonnes.add(f);
  return () => { abonnes.delete(f); };
}

/**
 * Déclarer les actions d'un écran tant qu'il est monté.
 *
 * `cle` nomme l'écran (une page qui se remonte remplace ses actions,
 * pas ne les cumule). Les fermetures capturent l'état du rendu où elles
 * sont posées : passer en `deps` ce dont elles dépendent, comme pour un
 * effet.
 */
export function useActionsPalette(cle: string, actions: ActionPalette[], deps: unknown[] = []) {
  useEffect(() => {
    parPage.set(cle, actions);
    prevenir();
    return () => {
      parPage.delete(cle);
      prevenir();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}

// ---------------------------------------------------------------------
//  La recherche
// ---------------------------------------------------------------------

/** Sans accents ni casse : « reglement » trouve « Règlement ». */
export function normaliser(s: string): string {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

/**
 * Chaque mot tapé doit se retrouver quelque part dans l'action. Le
 * score préfère le libellé qui COMMENCE par le premier mot, puis celui
 * qui le contient, puis le reste — assez pour que « cli » mette
 * Clients avant « Nouveau client ».
 */
export function filtrer(actions: ActionPalette[], requete: string): ActionPalette[] {
  const mots = normaliser(requete).split(/\s+/).filter(Boolean);
  if (mots.length === 0) return actions;
  const notes: [number, ActionPalette][] = [];
  for (const a of actions) {
    const libelle = normaliser(a.libelle);
    const tout = `${libelle} ${normaliser(a.detail ?? "")} ${normaliser(a.motsCles ?? "")} ${normaliser(a.groupe)}`;
    if (!mots.every((m) => tout.includes(m))) continue;
    const note = libelle.startsWith(mots[0]) ? 0 : libelle.includes(mots[0]) ? 1 : 2;
    notes.push([note, a]);
  }
  return notes.sort((x, y) => x[0] - y[0]).map(([, a]) => a);
}
