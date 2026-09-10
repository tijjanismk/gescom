// lib/modeles/service.ts — charger, semer, imprimer.

import { appeler as invoke } from "@/lib/pont";
import { modelesParDefaut, ACTIFS_PAR_DEFAUT } from "./defauts";
import { rendreModele, type ImagesDocument } from "./rendu";
import type { GenreDocument, Modele } from "./types";

/**
 * Installe les modèles d'usine manquants.
 *
 * Appelé au démarrage. N'écrase JAMAIS un modèle existant, même
 * d'usine : le commerçant a pu le modifier, et une mise à jour de
 * Gescom qui reprendrait la main sur sa mise en page lui ferait
 * imprimer une facture qu'il ne reconnaît pas, sans prévenir.
 * Le retour à l'usine reste un geste explicite, dans l'écran.
 */
export async function assurerModelesParDefaut(): Promise<void> {
  const existants = await invoke<Modele[]>("lire_modeles", {});
  const connus = new Set(existants.map((m) => m.id));

  for (const m of modelesParDefaut()) {
    if (connus.has(m.id)) continue;
    await invoke("enregistrer_modele", { modele: m });
  }

  // Chaque genre a besoin d'un actif, sinon l'impression retombe sans
  // rien dire sur le générateur historique.
  const apres = await invoke<Modele[]>("lire_modeles", {});
  const genresActifs = new Set(apres.filter((m) => m.actif).map((m) => m.genre));
  for (const id of ACTIFS_PAR_DEFAUT) {
    const m = apres.find((x) => x.id === id);
    if (m && !genresActifs.has(m.genre)) {
      await invoke("definir_modele_actif", { id });
      genresActifs.add(m.genre);
    }
  }
}

/**
 * Le logo, l'en-tête et le pied, en base64.
 *
 * Chargés à part et non stockés dans le modèle : ce sont des images de
 * la SOCIÉTÉ, pas de la mise en page. Les mettre dans le modèle
 * gonflerait le fichier d'export de plusieurs centaines de kilo-octets
 * et exporterait le logo d'une boutique vers une autre.
 */
export async function chargerImages(): Promise<ImagesDocument> {
  const lire = async (commande: string) => {
    try {
      return await invoke<string | null>(commande);
    } catch {
      return null;
    }
  };
  const [logo, entete, pied] = await Promise.all([
    lire("lire_logo_base64"),
    lire("lire_entete_base64"),
    lire("lire_pied_base64"),
  ]);
  return { logo, entete, pied };
}

export async function modeleActif(genre: GenreDocument): Promise<Modele | null> {
  try {
    return await invoke<Modele | null>("lire_modele_actif", { genre });
  } catch {
    return null;
  }
}

/**
 * Imprime par le modèle actif du genre.
 *
 * Rend `false` s'il n'y a pas de modèle — l'appelant retombe alors sur
 * le générateur historique. Ce repli n'est pas une élégance : les
 * modèles sont neufs, et une facture qui ne sort pas coûte une vente.
 */
export async function imprimerParModele(
  genre: GenreDocument,
  donnees: unknown,
  nomFichier?: string,
): Promise<boolean> {
  const modele = await modeleActif(genre);
  if (!modele || !modele.contenu?.blocs?.length) return false;

  const images = await chargerImages();
  const html = rendreModele(modele, donnees, { images });
  await invoke("imprimer_facture", { html, nomFichier: nomFichier ?? null });
  return true;
}
