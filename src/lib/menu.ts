// lib/menu.ts — D37 : ce que le menu du dossier cache.
//
// Un affichage seulement : cacher une entree ne change ni les donnees,
// ni les regles, ni les traces (l'Historique enregistre toujours). Le
// reglage vit cote serveur, par dossier (`lire_menu` / `definir_menu`) ;
// ici on le garde pour le menu et la palette, et on previent quand il
// change — l'onglet Parametres → Menu et le menu ne se voient pas.

import { appeler as invoke } from "@/lib/pont";

export interface EntreeMenu { href: string; nom: string; }
export interface ReglageMenu {
  caches: string[];
  masquables: EntreeMenu[];
  toujours: EntreeMenu[];
  boutique_simple: string[];
}

const EVENEMENT = "gescom:menu";
let caches = new Set<string>();

/** L'entree `href` est-elle cachee dans ce dossier ? */
export function estCache(href: string): boolean {
  return caches.has(href);
}

/** Relit le reglage du dossier, puis previent le menu. */
export async function chargerMenu(): Promise<ReglageMenu> {
  const r = await invoke<ReglageMenu>("lire_menu");
  poser(r.caches);
  return r;
}

/** Enregistre ce que le menu cache (le serveur juge), puis previent. */
export async function definirMenu(liste: string[]): Promise<ReglageMenu> {
  const r = await invoke<ReglageMenu>("definir_menu", { caches: liste });
  poser(r.caches);
  return r;
}

function poser(liste: string[]) {
  caches = new Set(liste);
  window.dispatchEvent(new Event(EVENEMENT));
}

/** S'abonner aux changements (le menu se redessine). */
export function surChangementMenu(f: () => void): () => void {
  window.addEventListener(EVENEMENT, f);
  return () => window.removeEventListener(EVENEMENT, f);
}
