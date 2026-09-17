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
  const [logo, entete, pied, libres] = await Promise.all([
    lire("lire_logo_base64"),
    lire("lire_entete_base64"),
    lire("lire_pied_base64"),
    // Les images posées sur les documents, toutes, par identifiant.
    // Une base sans la table (serveur d'avant I1) rend une erreur :
    // on rend alors « aucune », le document s'imprime sans cachet.
    invoke<Record<string, string>>("lire_images_base64").catch(() => ({})),
  ]);
  return { logo, entete, pied, libres: libres ?? {} };
}

/** Une image posable sur un document, telle que le serveur la liste. */
export interface ImageLibre {
  id: string;
  nom: string;
  taille: number;
  cree_le: string;
}

export async function listerImages(): Promise<ImageLibre[]> {
  try {
    return await invoke<ImageLibre[]>("lister_images");
  } catch {
    return [];
  }
}

/**
 * Importe une image à poser sur un document. Rend son identifiant, ou
 * `null` si personne n'a choisi de fichier.
 */
export async function importerImageLibre(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { readFile } = await import("@tauri-apps/plugin-fs");
  const fichier = await open({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }],
  });
  if (!fichier || typeof fichier !== "string") return null;
  const octets = await readFile(fichier);
  let texte = "";
  for (let i = 0; i < octets.length; i += 0x8000) {
    texte += String.fromCharCode(...octets.subarray(i, i + 0x8000));
  }
  const r = await invoke<{ id: string }>("importer_image", {
    nom: fichier.split(/[\\/]/).pop() || "image.png",
    contenu: btoa(texte),
  });
  return r.id;
}

/** Le serveur refuse si un modèle la pose encore : l'erreur dit lesquels. */
export async function supprimerImageLibre(id: string): Promise<void> {
  await invoke("supprimer_image", { id });
}

/**
 * Pose une des trois images de la société, depuis l'atelier.
 *
 * Elles se réglaient dans Paramètres → Société ; elles appartiennent au
 * DOCUMENT, et c'est en le dessinant qu'on s'aperçoit qu'il manque un
 * cachet. Le fichier part en base64 (D8) : un chemin de caisse ne
 * désigne rien chez le serveur.
 *
 * Rend `false` si personne n'a choisi de fichier.
 */
export async function importerImageSociete(
  genre: "logo" | "entete" | "pied",
): Promise<boolean> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { readFile } = await import("@tauri-apps/plugin-fs");
  const fichier = await open({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }],
  });
  if (!fichier || typeof fichier !== "string") return false;

  const octets = await readFile(fichier);
  let texte = "";
  for (let i = 0; i < octets.length; i += 0x8000) {
    texte += String.fromCharCode(...octets.subarray(i, i + 0x8000));
  }
  await invoke(`sauvegarder_${genre}`, {
    nom: fichier.split(/[\/]/).pop() || "image.png",
    contenu: btoa(texte),
  });
  return true;
}

export async function modeleActif(genre: GenreDocument): Promise<Modele | null> {
  try {
    return await invoke<Modele | null>("lire_modele_actif", { genre });
  } catch {
    return null;
  }
}

/**
 * Les modèles d'un genre, pour que l'écran les propose.
 *
 * Rend une liste vide plutôt qu'une erreur : un aperçu doit s'ouvrir
 * même si les modèles ne se lisent pas — le générateur d'origine, lui,
 * est toujours là.
 */
export async function modelesDuGenre(genre: GenreDocument): Promise<Modele[]> {
  try {
    return await invoke<Modele[]>("lire_modeles", { genre });
  } catch {
    return [];
  }
}

/**
 * Un modèle + des données → le document, tel qu'il sortira.
 *
 * UNE seule génération pour l'aperçu et pour l'impression. Rendre deux
 * fois — une pour montrer, une pour imprimer — c'est exactement ainsi
 * qu'on finit par imprimer autre chose que ce qui était à l'écran.
 * `images` se passe quand l'appelant les a déjà : l'aperçu les charge
 * pour lui-même, inutile de les redemander au serveur.
 */
export async function htmlParModele(
  modele: Modele,
  donnees: unknown,
  images?: ImagesDocument,
): Promise<string> {
  return rendreModele(modele, donnees, {
    images: images ?? (await chargerImages()),
  });
}

/** Un modèle utilisable : il existe, et il a des blocs à poser. */
export function modeleUtilisable(m: Modele | null | undefined): m is Modele {
  return !!m && Array.isArray(m.contenu?.blocs) && m.contenu.blocs.length > 0;
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
  if (!modeleUtilisable(modele)) return false;

  const html = await htmlParModele(modele, donnees);
  await invoke("imprimer_facture", { html, nomFichier: nomFichier ?? null });
  return true;
}
