// pages/Modeles.tsx — l'atelier des documents.
//
// Trois colonnes : ce qu'on peut poser, ce qui est posé, et à quoi ça
// ressemble. L'aperçu est à droite en permanence et se redessine à
// chaque frappe — une mise en page qu'on ne voit qu'à l'impression se
// règle à l'aveugle, et on découvre la colonne trop étroite sur le
// papier remis au client.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlignCenter, AlignLeft, AlignRight, ArrowDown, ArrowLeft, ArrowUp,
  Bold, Copy, Download, Eye, EyeOff, FileText, GripVertical,
  Italic, Loader2,
  Minus, Plus, Printer, RotateCcw, Save, Star, Trash2, Underline, Upload,
} from "lucide-react";
import { message, confirm, open, save } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { appeler as invoke } from "@/lib/pont";
import { EditeurPiedPage } from "@/components/EditeurPiedPage";

import {
  CHAMPS_PAR_GENRE, CHAMPS_PIECE, CHAMPS_SOCIETE, CHAMPS_TOTAUX,
  COLONNES_PAR_GENRE, FORMATS, GENRES, SOURCE_PAR_GENRE,
} from "@/lib/modeles/types";
import type {
  Alignement, Bloc, CadreFlottant, Champ, ChampDisponible, Colonne, FormatPapier,
  FormatValeur, GenreDocument, Modele, TypeBloc,
} from "@/lib/modeles/types";
import { rendreModele } from "@/lib/modeles/rendu";
import { contexteExemple } from "@/lib/modeles/contexte";
import { modeleDUsine } from "@/lib/modeles/defauts";
import {
  assurerModelesParDefaut, chargerImages, importerImageSociete,
  importerImageLibre, listerImages, supprimerImageLibre, type ImageLibre,
} from "@/lib/modeles/service";
import type { ImagesDocument } from "@/lib/modeles/rendu";

// =====================================================================
//  La palette
// =====================================================================

/** Les blocs du flux, dans l'ordre du document — pas les flottants,
 *  qui sont posés au millimètre et n'ont pas de « avant / après ». */
function blocsDuFlux(doc: Document): Element[] {
  return Array.from(doc.querySelectorAll("[data-bloc]:not(.flottant)"));
}

/**
 * Où tombera le bloc qu'on lâche, d'après le curseur : l'identifiant du
 * bloc DEVANT lequel il passe, ou `null` pour la fin.
 *
 * Au-dessus du milieu d'un bloc, il passe devant ; en dessous, derrière.
 * Hors de tout bloc — la marge du bas — il va à la fin. Un identifiant
 * et non un rang : un bloc caché ou flottant n'est pas dans le document,
 * et le rang dans l'aperçu ne serait plus celui de la structure.
 */
function placeSousLeCurseur(doc: Document, e: DragEvent): string | null {
  const blocs = blocsDuFlux(doc);
  for (let i = 0; i < blocs.length; i += 1) {
    const r = blocs[i].getBoundingClientRect();
    if (e.clientY < r.top + r.height / 2) return blocs[i].getAttribute("data-bloc");
    if (e.clientY <= r.bottom) return blocs[i + 1]?.getAttribute("data-bloc") ?? null;
  }
  return null;
}

/** Le trait qui dit où le bloc va tomber. */
function montrerPoint(doc: Document, e: DragEvent) {
  effacerPoint(doc);
  const blocs = blocsDuFlux(doc);
  const avant = placeSousLeCurseur(doc, e);
  const trait = doc.createElement("div");
  trait.className = "point-depot";
  trait.setAttribute("style",
    "height:0;border-top:.6mm solid currentColor;margin:1mm 0;"
    + "opacity:.75;pointer-events:none;");
  const apres = avant ? doc.querySelector(`[data-bloc="${avant}"]`) : null;
  if (apres) apres.parentNode?.insertBefore(trait, apres);
  else (blocs[blocs.length - 1]?.parentNode ?? doc.body).appendChild(trait);
}

/** Le cadre d'un bloc flottant, lu sur l'élément lui-même (en mm). */
function cadreDe(el: HTMLElement): CadreFlottant {
  const mm = (v: string) => parseFloat(v) || 0;
  return {
    xMm: mm(el.style.left), yMm: mm(el.style.top),
    largeurMm: mm(el.style.width), hauteurMm: mm(el.style.height),
  };
}

/** Combien de pixels vaut un millimètre dans ce document. */
function pxParMm(doc: Document): number {
  const sonde = doc.createElement("div");
  sonde.setAttribute("style", "position:absolute;width:100mm;height:0;visibility:hidden");
  doc.body.appendChild(sonde);
  const px = sonde.getBoundingClientRect().width / 100;
  sonde.remove();
  return px || 3.78;
}

/** Au demi-millimètre : assez fin pour l'œil, lisible dans les réglages. */
const demi = (v: number) => Math.round(v * 2) / 2;

/**
 * Le cadre qu'un bloc DU FLUX occupe à l'écran, en mm depuis le coin
 * haut-gauche de la feuille : c'est le cadre qu'il garde quand on le
 * détache pour le poser à la main — il ne bouge pas d'un millimètre au
 * moment où il devient flottant.
 */
function cadreDansLaFeuille(doc: Document, el: HTMLElement): CadreFlottant | null {
  const feuille = doc.querySelector<HTMLElement>(".feuille");
  if (!feuille) return null;
  const px = pxParMm(doc);
  const f = feuille.getBoundingClientRect();
  const r = el.getBoundingClientRect();
  return {
    xMm: demi(Math.max(0, (r.left - f.left) / px)),
    yMm: demi(Math.max(0, (r.top - f.top) / px)),
    largeurMm: demi(Math.max(3, r.width / px)),
    hauteurMm: demi(Math.max(3, r.height / px)),
  };
}

/** Le pointeur est-il sur le coin bas-droit (la poignée) de l'élément ? */
function surLaPoignee(doc: Document, el: HTMLElement, e: PointerEvent): boolean {
  const px = pxParMm(doc);
  const r = el.getBoundingClientRect();
  return e.clientX > r.right - 4 * px && e.clientY > r.bottom - 4 * px;
}

/**
 * Attraper un bloc dans l'aperçu : le déplacer, ou le redimensionner par
 * son coin bas-droit. Le geste se voit en direct sur l'élément ; le
 * modèle n'est écrit qu'au relâcher, en une fois.
 *
 * Un bloc du flux qu'on prend par le coin se DÉTACHE : il devient
 * flottant, au cadre qu'il occupait, et le geste continue sans
 * relâcher. C'est ainsi que « chaque rectangle se redimensionne dans
 * l'aperçu », sans passer par une case à cocher.
 */
function attraperFlottant(
  doc: Document, el: HTMLElement, e: PointerEvent,
  poser: (cadre: CadreFlottant) => void,
  depuisLeFlux?: CadreFlottant,
) {
  const px = pxParMm(doc);
  const depart = depuisLeFlux ?? cadreDe(el);
  // Le coin bas-droit — la poignée dessinée par le rendu — redimensionne.
  const redim = !!depuisLeFlux || surLaPoignee(doc, el, e);
  if (depuisLeFlux) {
    // Sorti du flux sur-le-champ, à sa place : ce qui suit remonte,
    // comme après l'enregistrement.
    el.style.position = "absolute";
    el.style.margin = "0";
    el.style.left = `${depart.xMm}mm`;
    el.style.top = `${depart.yMm}mm`;
    el.style.width = `${depart.largeurMm}mm`;
    el.style.height = `${depart.hauteurMm}mm`;
    el.style.overflow = "hidden";
  }
  const x0 = e.clientX;
  const y0 = e.clientY;
  let cadre = depart;
  const bouger = (ev: PointerEvent) => {
    const dx = (ev.clientX - x0) / px;
    const dy = (ev.clientY - y0) / px;
    cadre = redim
      ? { ...depart,
          largeurMm: demi(Math.max(3, depart.largeurMm + dx)),
          hauteurMm: demi(Math.max(3, depart.hauteurMm + dy)) }
      : { ...depart,
          xMm: demi(Math.max(0, depart.xMm + dx)),
          yMm: demi(Math.max(0, depart.yMm + dy)) };
    el.style.left = `${cadre.xMm}mm`;
    el.style.top = `${cadre.yMm}mm`;
    el.style.width = `${cadre.largeurMm}mm`;
    el.style.height = `${cadre.hauteurMm}mm`;
  };
  const lacher = () => {
    doc.removeEventListener("pointermove", bouger);
    doc.removeEventListener("pointerup", lacher);
    doc.removeEventListener("pointercancel", lacher);
    // Un bloc pris par le coin depuis le flux est détaché même sans
    // bouger : c'est le geste qui le dit.
    if (depuisLeFlux || JSON.stringify(cadre) !== JSON.stringify(depart)) poser(cadre);
  };
  doc.addEventListener("pointermove", bouger);
  doc.addEventListener("pointerup", lacher);
  doc.addEventListener("pointercancel", lacher);
}

function effacerPoint(doc: Document) {
  doc.querySelectorAll(".point-depot").forEach((el) => el.remove());
}

/** Entoure dans l'aperçu le bloc en cours de réglage. */
function marquerChoisi(doc: Document | null, id: string | null) {
  if (!doc) return;
  doc.querySelectorAll("[data-bloc].bloc-choisi")
    .forEach((el) => el.classList.remove("bloc-choisi"));
  if (!id) return;
  doc.querySelector(`[data-bloc="${id}"]`)?.classList.add("bloc-choisi");
}

const PALETTE: { type: TypeBloc; nom: string; aide: string }[] = [
  { type: "entete", nom: "En-tête", aide: "Logo et coordonnées de la société" },
  { type: "image", nom: "Image", aide: "Logo, en-tête ou cachet, à la taille voulue" },
  { type: "titre", nom: "Titre", aide: "Le nom du document et son numéro" },
  { type: "champs", nom: "Champs", aide: "Libellé + valeur, sur 1 à 3 colonnes" },
  { type: "tableau", nom: "Tableau", aide: "Les lignes du document" },
  { type: "totaux", nom: "Totaux", aide: "Bloc de totaux aligné à droite" },
  { type: "texte", nom: "Texte", aide: "Mention libre, avec {{champs}}" },
  { type: "signatures", nom: "Signatures", aide: "Deux traits et deux noms" },
  { type: "pied_page", nom: "Pied de page", aide: "Bande dessinée, en bas de chaque page" },
  { type: "trait", nom: "Trait", aide: "Séparateur horizontal" },
  { type: "espace", nom: "Espace", aide: "Blanc vertical" },
  { type: "saut_page", nom: "Saut de page", aide: "Force une nouvelle page" },
];

let compteurBloc = 0;
function nouvelId(prefixe: string): string {
  compteurBloc += 1;
  return `${prefixe}-${Date.now().toString(36)}-${compteurBloc}`;
}

function blocNeuf(type: TypeBloc, genre: GenreDocument): Bloc {
  const base = { id: nouvelId("b"), visible: true };
  switch (type) {
    case "entete":
      return { ...base, type, image: "logo", hauteurMm: 18, afficherSociete: true, alignement: "gauche" };
    case "titre":
      return { ...base, type, texte: "{{piece.type_libelle}} N° {{piece.numero}}", alignement: "centre", taillePt: 15, trait: true };
    case "champs":
      return {
        ...base, type, titre: "", colonnes: 2,
        items: [
          { id: nouvelId("c"), libelle: "Date", chemin: "piece.date_piece", format: "date", masquerSiVide: false },
          { id: nouvelId("c"), libelle: "Client", chemin: "tiers.nom", format: "texte", masquerSiVide: false },
        ],
      };
    case "tableau":
      return {
        ...base, type, source: SOURCE_PAR_GENRE[genre], zebre: true,
        siVide: "Aucune ligne.",
        colonnes: [
          { id: nouvelId("col"), libelle: "Désignation", chemin: "article_nom", largeur: 60, alignement: "gauche", format: "texte" },
          { id: nouvelId("col"), libelle: "Qté", chemin: "quantite", largeur: 15, alignement: "droite", format: "nombre" },
          { id: nouvelId("col"), libelle: "Montant", chemin: "montant_ttc", largeur: 25, alignement: "droite", format: "montant" },
        ],
      };
    case "totaux":
      return {
        ...base, type, accentuerDernier: true,
        items: [
          { id: nouvelId("c"), libelle: "TOTAL", chemin: "totaux.total_ttc", format: "montant", masquerSiVide: false },
        ],
      };
    case "texte":
      return { ...base, type, contenu: "Merci de votre confiance.", alignement: "centre", taillePt: 9, italique: true, cadre: false };
    case "signatures":
      return { ...base, type, gauche: "Le client", droite: "Pour l'entreprise" };
    case "pied_page":
      // Vide et 20 mm : on ne devine pas ce que le commercant veut y
      // mettre, mais on lui donne tout de suite une bande a la bonne
      // taille pour un cachet.
      return { ...base, type, hauteurMm: 20, trait: true, elements: [] };
    case "image":
      // 40 mm de large, hauteur libre : le logo garde ses proportions
      // et le commercant ajuste ensuite en regardant l'apercu.
      return { ...base, type, image: "logo", largeurMm: 40, hauteurMm: 0, alignement: "gauche" };
    case "espace":
      return { ...base, type, hauteurMm: 5 };
    default:
      return { ...base, type } as Bloc;
  }
}

function resumeBloc(b: Bloc): string {
  switch (b.type) {
    case "entete": return b.afficherSociete ? "Logo + société" : "Logo seul";
    case "titre": return b.texte.slice(0, 40);
    case "champs": return `${b.items.length} champ(s), ${b.colonnes} col.`;
    case "tableau": return `${b.source} — ${b.colonnes.length} colonne(s)`;
    case "totaux": return `${b.items.length} total/totaux`;
    case "texte": return b.contenu.slice(0, 40).replace(/\n/g, " ");
    case "signatures": return [b.gauche, b.droite].filter(Boolean).join(" · ") || "vides";
    case "pied_page": return `${b.hauteurMm} mm, ${b.elements.length} élément(s)`;
    case "image":
      return `${NOM_IMAGE[b.image]} — ${b.largeurMm > 0 ? `${b.largeurMm} mm` : "largeur libre"}`;
    case "espace": return `${b.hauteurMm} mm`;
    default: return "";
  }
}

const NOM_TYPE: Record<TypeBloc, string> =
  Object.fromEntries(PALETTE.map((p) => [p.type, p.nom])) as Record<TypeBloc, string>;

// =====================================================================
//  Écran
// =====================================================================

export function Modeles({ onFermer }: { onFermer?: () => void } = {}) {
  const [genre, setGenre] = useState<GenreDocument>("facture");
  const [liste, setListe] = useState<Modele[]>([]);
  const [modele, setModele] = useState<Modele | null>(null);
  const [modifie, setModifie] = useState(false);
  const [blocActif, setBlocActif] = useState<string | null>(null);
  const apercuRef = useRef<HTMLIFrameElement>(null);
  const depotApercuRef = useRef<((charge: string, avant: string | null) => void) | null>(null);
  const poserFlottantRef = useRef<((id: string, cadre: CadreFlottant) => void) | null>(null);

  /** Poser une image de la société sans quitter l'atelier. */
  const importerImage = useCallback(async (genre: "logo" | "entete" | "pied") => {
    try {
      if (await importerImageSociete(genre)) setImages(await chargerImages());
    } catch (e) {
      console.error("Import d'image :", e);
    }
  }, []);

  // Les images POSEES sur les documents : leur liste (noms) et leurs
  // octets (dans `images.libres`). Importer une image la selectionne
  // tout de suite dans le bloc en cours — c'est pour lui qu'on l'importe.
  const [imagesLibres, setImagesLibres] = useState<ImageLibre[]>([]);
  useEffect(() => { listerImages().then(setImagesLibres); }, []);
  const importerLibre = useCallback(async () => {
    try {
      const id = await importerImageLibre();
      if (!id) return;
      const [liste, imgs] = await Promise.all([listerImages(), chargerImages()]);
      setImagesLibres(liste);
      setImages(imgs);
      if (blocActif) majBloc(blocActif, { imageId: id });
    } catch (e) {
      await message(String(e), { title: "Import d'image", kind: "error" });
    }
  }, [blocActif]);
  const supprimerLibre = useCallback(async (id: string) => {
    try {
      await supprimerImageLibre(id);
      const [liste, imgs] = await Promise.all([listerImages(), chargerImages()]);
      setImagesLibres(liste);
      setImages(imgs);
      if (blocActif) majBloc(blocActif, { imageId: undefined });
    } catch (e) {
      // Le serveur refuse tant qu'un modele la pose, et nomme lesquels.
      await message(String(e), { title: "Image utilisée", kind: "warning" });
    }
  }, [blocActif]);
  const [images, setImages] = useState<ImagesDocument>({});
  const [chargement, setChargement] = useState(true);
  // Le semis des modèles d'usine peut échouer sans que la page soit
  // inutilisable : on le dit, on n'efface pas l'écran.
  const [avertissement, setAvertissement] = useState<string | null>(null);
  const [enregistrement, setEnregistrement] = useState(false);
  const survolRef = useRef<number | null>(null);
  const [survol, setSurvol] = useState<number | null>(null);

  // -------------------------------------------------------------------
  //  Chargement
  // -------------------------------------------------------------------

  const recharger = useCallback(async (garderId?: string) => {
    const tous = await invoke<Modele[]>("lire_modeles", {});
    setListe(tous);
    const duGenre = tous.filter((m) => m.genre === genre);
    const choisi =
      duGenre.find((m) => m.id === garderId) ??
      duGenre.find((m) => m.actif) ??
      duGenre[0] ??
      null;
    setModele(choisi ? structuredClone(choisi) : null);
    setModifie(false);
    setBlocActif(null);
  }, [genre]);

  useEffect(() => {
    let vivant = true;
    (async () => {
      setChargement(true);
      setAvertissement(null);
      try {
        // Le semis ÉCRIT : il installe les modèles d'usine manquants,
        // ce qui demande le droit de gérer les modèles. Il n'a aucune
        // raison d'empêcher la CONSULTATION. Le mettre dans le même
        // `try` que la lecture faisait perdre l'écran entier — et avec
        // lui le bouton qui aurait permis de réparer.
        try {
          await assurerModelesParDefaut();
        } catch (e) {
          if (!vivant) return;
          setAvertissement(
            `Les modèles d'usine n'ont pas pu être installés : ${String(e)}`,
          );
        }
        const imgs = await chargerImages();
        if (!vivant) return;
        setImages(imgs);
        await recharger();
      } catch (e) {
        await message(
          `Impossible de lire les modèles enregistrés.\n\n${String(e)}`,
          { title: "Chargement des modèles", kind: "error" },
        );
      } finally {
        if (vivant) setChargement(false);
      }
    })();
    return () => { vivant = false; };
  }, [recharger]);

  const duGenre = useMemo(
    () => liste.filter((m) => m.genre === genre),
    [liste, genre],
  );

  // -------------------------------------------------------------------
  //  Aperçu
  // -------------------------------------------------------------------

  const apercu = useMemo(() => {
    if (!modele) return "";
    try {
      return rendreModele(modele, contexteExemple(genre), { apercu: true, designable: true, images });
    } catch (e) {
      // Un modèle en cours d'édition peut être momentanément incohérent.
      // L'aperçu doit le dire, pas disparaître : un cadre blanc laisse
      // croire que le document est vide.
      return `<p style="font:13px sans-serif;color:#a00;padding:20px">
                Aperçu impossible : ${String(e)}</p>`;
    }
  }, [modele, genre, images]);

  // -------------------------------------------------------------------
  //  Mutations
  // -------------------------------------------------------------------

  function majBlocs(f: (b: Bloc[]) => Bloc[]) {
    setModele((m) => {
      if (!m) return m;
      return { ...m, contenu: { ...m.contenu, blocs: f(m.contenu.blocs) } };
    });
    setModifie(true);
  }

  function majBloc(id: string, patch: Record<string, unknown>) {
    majBlocs((blocs) =>
      blocs.map((b) => (b.id === id ? ({ ...b, ...patch } as Bloc) : b)),
    );
  }

  function insererBloc(type: TypeBloc, position: number) {
    const b = blocNeuf(type, genre);
    majBlocs((blocs) => {
      const copie = [...blocs];
      copie.splice(position, 0, b);
      return copie;
    });
    setBlocActif(b.id);
  }

  function deplacerBloc(de: number, vers: number) {
    if (de === vers || de + 1 === vers) return;
    majBlocs((blocs) => {
      const copie = [...blocs];
      const [b] = copie.splice(de, 1);
      copie.splice(de < vers ? vers - 1 : vers, 0, b);
      return copie;
    });
  }

  function supprimerBloc(id: string) {
    majBlocs((blocs) => blocs.filter((b) => b.id !== id));
    setBlocActif((a) => (a === id ? null : a));
  }

  // -------------------------------------------------------------------
  //  Glisser-déposer
  // -------------------------------------------------------------------

  function surDepot(e: React.DragEvent, position: number) {
    e.preventDefault();
    const charge = e.dataTransfer.getData("text/plain");
    setSurvol(null);
    survolRef.current = null;
    if (charge.startsWith("nouveau:")) {
      insererBloc(charge.slice(8) as TypeBloc, position);
    } else if (charge.startsWith("deplacer:")) {
      deplacerBloc(Number(charge.slice(9)), position);
    }
  }

  function surSurvol(e: React.DragEvent, position: number) {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (survolRef.current !== position) {
      survolRef.current = position;
      setSurvol(position);
    }
  }

  // -------------------------------------------------------------------
  //  Actions
  // -------------------------------------------------------------------

  async function enregistrer() {
    if (!modele) return;
    setEnregistrement(true);
    try {
      await invoke("enregistrer_modele", { modele });
      await recharger(modele.id);
    } catch (e) {
      await message(String(e), { title: "Enregistrement", kind: "error" });
    } finally {
      setEnregistrement(false);
    }
  }

  async function activer() {
    if (!modele) return;
    try {
      if (modifie) await invoke("enregistrer_modele", { modele });
      await invoke("definir_modele_actif", { id: modele.id });
      await recharger(modele.id);
    } catch (e) {
      await message(String(e), { title: "Activation", kind: "error" });
    }
  }

  async function dupliquer() {
    if (!modele) return;
    const copie: Modele = {
      ...structuredClone(modele),
      id: nouvelId("mod"),
      nom: `${modele.nom} (copie)`,
      est_defaut: false,
      actif: false,
    };
    try {
      await invoke("enregistrer_modele", { modele: copie });
      await recharger(copie.id);
    } catch (e) {
      await message(String(e), { title: "Duplication", kind: "error" });
    }
  }

  async function supprimer() {
    if (!modele) return;
    const ok = await confirm(
      `Supprimer « ${modele.nom} » ? Cette action ne se défait pas.`,
      { title: "Supprimer le modèle", kind: "warning" },
    );
    if (!ok) return;
    try {
      await invoke("supprimer_modele", { id: modele.id });
      await recharger();
    } catch (e) {
      await message(String(e), { title: "Suppression", kind: "error" });
    }
  }

  async function reinitialiser() {
    if (!modele) return;
    const usine = modeleDUsine(modele.id);
    if (!usine) {
      await message(
        "Ce modèle n'a pas de version d'usine : il a été créé ici. "
        + "Le supprimer ou le corriger à la main.",
        { title: "Pas de version d'usine", kind: "info" },
      );
      return;
    }
    const ok = await confirm(
      `Remettre « ${modele.nom} » dans son état d'origine ? `
      + "Toutes les modifications seront perdues.",
      { title: "Réinitialiser", kind: "warning" },
    );
    if (!ok) return;
    setModele({ ...usine, actif: modele.actif });
    setModifie(true);
    setBlocActif(null);
  }

  async function exporter() {
    try {
      const chemin = await save({
        title: "Exporter les modèles",
        defaultPath: "modeles-gescom.json",
        filters: [{ name: "Modèles Gescom", extensions: ["json"] }],
      });
      if (!chemin) return;
      // Le contenu vient du serveur (ou de la base locale en monoposte) ;
      // le fichier s'ecrit ICI, sur le poste qui a choisi l'emplacement.
      // Indente : un export doit rester lisible et comparable.
      const lot = await invoke<unknown>("exporter_modeles", { ids: null });
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      await writeTextFile(chemin, JSON.stringify(lot, null, 2));
      await message(
        `${liste.length} modèle(s) exporté(s).\n\nCopier ce fichier sur les `
        + "autres postes, puis l'importer depuis cet écran.",
        { title: "Export terminé" },
      );
    } catch (e) {
      await message(String(e), { title: "Export", kind: "error" });
    }
  }

  async function importer() {
    try {
      const chemin = await open({
        title: "Importer des modèles",
        multiple: false,
        filters: [{ name: "Modèles Gescom", extensions: ["json"] }],
      });
      if (!chemin || typeof chemin !== "string") return;
      const { readTextFile } = await import("@tauri-apps/plugin-fs");
      let lot: unknown;
      try {
        lot = JSON.parse(await readTextFile(chemin));
      } catch {
        throw "Ce fichier n'est pas un export de modèles Gescom lisible.";
      }
      const bilan = await invoke<{ ajoutes: number; remplaces: number; ignores: string[] }>(
        "importer_modeles", { lot },
      );
      await recharger();
      const details = bilan.ignores.length
        ? `\n\nIgnorés :\n${bilan.ignores.join("\n")}`
        : "";
      await message(
        `${bilan.ajoutes} ajouté(s), ${bilan.remplaces} remplacé(s).${details}`
        + "\n\nLe choix du modèle actif n'a pas été touché : il appartient "
        + "à ce poste.",
        { title: "Import terminé" },
      );
    } catch (e) {
      await message(String(e), { title: "Import", kind: "error" });
    }
  }

  async function imprimerExemple() {
    if (!modele) return;
    try {
      const html = rendreModele(modele, contexteExemple(genre), { images });
      await invoke("imprimer_facture", { html, nomFichier: "apercu_modele.html" });
    } catch (e) {
      await message(String(e), { title: "Impression", kind: "error" });
    }
  }

  async function creerVierge() {
    const neuf: Modele = {
      id: nouvelId("mod"),
      genre,
      nom: "Nouveau modèle",
      format: "a4",
      est_defaut: false,
      actif: false,
      contenu: {
        version: 1,
        page: { margeMm: 12, taillePt: 10, police: "Arial, Helvetica, sans-serif", couleurAccent: "#1a1a1a" },
        blocs: [blocNeuf("entete", genre), blocNeuf("titre", genre)],
      },
    };
    try {
      await invoke("enregistrer_modele", { modele: neuf });
      await recharger(neuf.id);
    } catch (e) {
      await message(String(e), { title: "Création", kind: "error" });
    }
  }

  // -------------------------------------------------------------------
  //  Rendu
  // -------------------------------------------------------------------

  const selection = modele?.contenu.blocs.find((b) => b.id === blocActif) ?? null;

  /**
   * Cliquer DANS le document choisit le bloc.
   *
   * L'aperçu est une iframe `allow-same-origin` SANS `allow-scripts` :
   * la page reste inerte — aucun script du document ne s'exécute — mais
   * React peut lire son DOM et y poser un écouteur. C'est ce qui permet
   * de régler un bloc en le désignant du doigt, au lieu de le chercher
   * dans une liste.
   */
  const brancherApercu = useCallback(() => {
    const doc = apercuRef.current?.contentDocument;
    if (!doc) return;
    doc.addEventListener("click", (e) => {
      const cible = (e.target as Element | null)?.closest?.("[data-bloc]");
      const id = cible?.getAttribute("data-bloc");
      if (id) setBlocActif(id);
    });
    // Un bloc flottant se prend à la main : déplacer, ou redimensionner
    // par le coin. Le geste ne touche le modèle qu'au relâcher.
    doc.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      const cible = e.target as Element | null;
      const flottant = cible?.closest?.(".flottant") as HTMLElement | null;
      if (flottant) {
        const id = flottant.getAttribute("data-bloc");
        if (!id) return;
        e.preventDefault();
        setBlocActif(id);
        attraperFlottant(doc, flottant, e, (cadre) => poserFlottantRef.current?.(id, cadre));
        return;
      }
      // Un bloc du flux, pris par son coin bas-droit : il se détache et
      // se redimensionne dans le même geste. Pris ailleurs, un clic le
      // choisit, rien de plus — le flux reste le flux.
      const bloc = cible?.closest?.("[data-bloc]") as HTMLElement | null;
      const id = bloc?.getAttribute("data-bloc");
      if (!bloc || !id || bloc.classList.contains("pied") || !surLaPoignee(doc, bloc, e)) return;
      const cadre = cadreDansLaFeuille(doc, bloc);
      if (!cadre) return;
      e.preventDefault();
      setBlocActif(id);
      attraperFlottant(doc, bloc, e, (c) => poserFlottantRef.current?.(id, c), cadre);
    });
    // Déposer DANS le document, et pas seulement dans la liste : c'est
    // là qu'on voit où le bloc va tomber. `dragover` doit accepter le
    // dépôt, sinon le navigateur refuse le `drop` sans rien dire.
    doc.addEventListener("dragover", (e) => {
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
      montrerPoint(doc, e);
    });
    doc.addEventListener("dragleave", () => effacerPoint(doc));
    doc.addEventListener("drop", (e) => {
      e.preventDefault();
      effacerPoint(doc);
      const charge = e.dataTransfer?.getData("text/plain") ?? "";
      // Les poignées vivent dans un ref : l'écouteur est posé une fois
      // par chargement de l'iframe et survivrait au rendu suivant avec
      // un `modele` périmé.
      depotApercuRef.current?.(charge, placeSousLeCurseur(doc, e));
    });
    marquerChoisi(doc, blocActif);
  }, [blocActif]);

  // La poignée de dépôt, réécrite à chaque rendu pour voir le modèle
  // dans son état courant.
  depotApercuRef.current = (charge: string, avant: string | null) => {
    const blocs = modele?.contenu.blocs ?? [];
    const rang = avant ? blocs.findIndex((b) => b.id === avant) : -1;
    const position = rang >= 0 ? rang : blocs.length;
    if (charge.startsWith("nouveau:")) {
      insererBloc(charge.slice(8) as TypeBloc, position);
    } else if (charge.startsWith("deplacer:")) {
      deplacerBloc(Number(charge.slice(9)), position);
    }
  };
  poserFlottantRef.current = (id, cadre) => majBloc(id, { flottant: cadre });

  // Le bloc choisi s'entoure dans l'aperçu, qu'on l'ait pris dans la
  // structure ou dans le document : les deux désignent la même chose.
  useEffect(() => {
    marquerChoisi(apercuRef.current?.contentDocument ?? null, blocActif);
  }, [blocActif, apercu]);
  const champsDispo: ChampDisponible[] = useMemo(
    () => [...CHAMPS_PIECE, ...CHAMPS_TOTAUX, ...CHAMPS_SOCIETE, ...CHAMPS_PAR_GENRE[genre]],
    [genre],
  );
  // Les colonnes d'un tableau dependent du genre : un releve liste des
  // factures, un journal des mouvements. Proposer « Designation,
  // Quantite, Prix unitaire » la-bas envoyait vers des colonnes vides.
  const colonnesDispo: ChampDisponible[] = useMemo(
    () => COLONNES_PAR_GENRE[genre],
    [genre],
  );

  if (chargement) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        <Loader2 className="mr-2 h-5 w-5 animate-spin" /> Chargement des modèles…
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      {avertissement && (
        <div className="rounded border border-amber-300 bg-amber-50 px-3 py-2 text-sm text-amber-900">
          {avertissement}
        </div>
      )}

      {/* ---- Barre du haut ---- */}
      <div className="flex flex-wrap items-end gap-2">
        {/* L'atelier prend tout l'écran : il faut un chemin de retour
            visible, sinon on se croit sorti des paramètres. */}
        {onFermer && (
          <Button variant="ghost" size="sm" onClick={onFermer}
            className="mb-0.5 h-9 px-2 text-muted-foreground">
            <ArrowLeft className="mr-1 h-4 w-4" /> Paramètres
          </Button>
        )}
        <div className="min-w-[190px]">
          <Label className="text-xs text-muted-foreground">Type de document</Label>
          <Select value={genre} onValueChange={(v) => setGenre(v as GenreDocument)}>
            <SelectTrigger><SelectValue /></SelectTrigger>
            <SelectContent>
              {GENRES.map((g) => (
                <SelectItem key={g.id} value={g.id}>{g.nom}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="min-w-[230px]">
          <Label className="text-xs text-muted-foreground">Modèle</Label>
          <Select
            value={modele?.id ?? ""}
            onValueChange={(v) => {
              const m = liste.find((x) => x.id === v);
              if (m) { setModele(structuredClone(m)); setModifie(false); setBlocActif(null); }
            }}
          >
            <SelectTrigger><SelectValue placeholder="Aucun modèle" /></SelectTrigger>
            <SelectContent>
              {duGenre.map((m) => (
                <SelectItem key={m.id} value={m.id}>
                  {m.actif ? "★ " : ""}{m.nom}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex-1" />

        <Button variant="outline" size="sm" onClick={creerVierge}>
          <Plus className="mr-1 h-4 w-4" /> Nouveau
        </Button>
        <Button variant="outline" size="sm" onClick={dupliquer} disabled={!modele}>
          <Copy className="mr-1 h-4 w-4" /> Dupliquer
        </Button>
        <Button variant="outline" size="sm" onClick={reinitialiser} disabled={!modele}>
          <RotateCcw className="mr-1 h-4 w-4" /> Réinitialiser
        </Button>
        <Button variant="outline" size="sm" onClick={importer}>
          <Upload className="mr-1 h-4 w-4" /> Importer
        </Button>
        <Button variant="outline" size="sm" onClick={exporter}>
          <Download className="mr-1 h-4 w-4" /> Exporter
        </Button>
        <Button variant="outline" size="sm" onClick={imprimerExemple} disabled={!modele}>
          <Printer className="mr-1 h-4 w-4" /> Imprimer un exemple
        </Button>
        <Button
          variant="outline" size="sm" onClick={supprimer}
          disabled={!modele || modele.est_defaut}
          title={modele?.est_defaut ? "Un modèle d'usine ne se supprime pas" : ""}
        >
          <Trash2 className="mr-1 h-4 w-4" /> Supprimer
        </Button>
        <Button size="sm" onClick={activer} disabled={!modele || modele.actif}>
          <Star className="mr-1 h-4 w-4" /> {modele?.actif ? "Actif" : "Utiliser"}
        </Button>
        <Button size="sm" onClick={enregistrer} disabled={!modele || !modifie || enregistrement}>
          {enregistrement
            ? <Loader2 className="mr-1 h-4 w-4 animate-spin" />
            : <Save className="mr-1 h-4 w-4" />}
          Enregistrer
        </Button>
      </div>

      {!modele ? (
        <div className="flex flex-1 items-center justify-center text-muted-foreground">
          Aucun modèle pour ce type de document. Créer un modèle.
        </div>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-[280px_320px_1fr]">
          {/* ---- Colonne 1 : palette + structure ---- */}
          <div className="flex min-h-0 flex-col gap-3">
            <div className="rounded-lg border p-2">
              <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Blocs — glisser dans la structure
              </div>
              <div className="grid grid-cols-2 gap-1">
                {PALETTE.map((p) => (
                  <div
                    key={p.type}
                    draggable
                    onDragStart={(e) => e.dataTransfer.setData("text/plain", `nouveau:${p.type}`)}
                    onDoubleClick={() => insererBloc(p.type, modele.contenu.blocs.length)}
                    title={`${p.aide}\n(double-clic pour ajouter à la fin)`}
                    className="cursor-grab rounded border bg-muted/40 px-2 py-1.5 text-xs
                               hover:bg-muted active:cursor-grabbing"
                  >
                    {p.nom}
                  </div>
                ))}
              </div>
            </div>

            <div className="flex min-h-0 flex-1 flex-col rounded-lg border">
              <div className="border-b px-2 py-1.5 text-xs font-semibold uppercase
                              tracking-wide text-muted-foreground">
                Structure du document
              </div>
              <div
                className="min-h-0 flex-1 overflow-y-auto p-1"
                onDragOver={(e) => surSurvol(e, modele.contenu.blocs.length)}
                onDrop={(e) => surDepot(e, modele.contenu.blocs.length)}
              >
                {modele.contenu.blocs.map((b, i) => (
                  <div key={b.id}>
                    <div
                      className={cn(
                        "h-0.5 rounded",
                        survol === i ? "bg-primary" : "bg-transparent",
                      )}
                      onDragOver={(e) => surSurvol(e, i)}
                      onDrop={(e) => surDepot(e, i)}
                    />
                    <div
                      draggable
                      onDragStart={(e) => e.dataTransfer.setData("text/plain", `deplacer:${i}`)}
                      onDragOver={(e) => surSurvol(e, i)}
                      onDrop={(e) => surDepot(e, i)}
                      onClick={() => setBlocActif(b.id)}
                      className={cn(
                        "group flex cursor-pointer items-center gap-1.5 rounded border px-2 py-1.5",
                        blocActif === b.id ? "border-primary bg-primary/5" : "border-transparent hover:bg-muted/50",
                        !b.visible && "opacity-45",
                      )}
                    >
                      <GripVertical className="h-3.5 w-3.5 shrink-0 cursor-grab text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <div className="text-xs font-medium">
                          {NOM_TYPE[b.type]}
                          {b.flottant && (
                            <span className="ml-1.5 rounded bg-muted px-1 text-[10px] font-normal text-muted-foreground"
                              title="Posé au millimètre, par-dessus le reste">
                              flottant
                            </span>
                          )}
                        </div>
                        <div className="truncate text-[11px] text-muted-foreground">
                          {resumeBloc(b)}
                        </div>
                      </div>
                      <button
                        className="opacity-0 group-hover:opacity-100"
                        title={b.visible ? "Ne pas imprimer ce bloc" : "Imprimer ce bloc"}
                        onClick={(e) => { e.stopPropagation(); majBloc(b.id, { visible: !b.visible }); }}
                      >
                        {b.visible
                          ? <Eye className="h-3.5 w-3.5 text-muted-foreground" />
                          : <EyeOff className="h-3.5 w-3.5 text-muted-foreground" />}
                      </button>
                      <button
                        className="opacity-0 group-hover:opacity-100"
                        title="Supprimer ce bloc"
                        onClick={(e) => { e.stopPropagation(); supprimerBloc(b.id); }}
                      >
                        <Trash2 className="h-3.5 w-3.5 text-destructive" />
                      </button>
                    </div>
                  </div>
                ))}
                <div
                  className={cn(
                    "mt-1 h-8 rounded border border-dashed text-center text-[11px] leading-8 text-muted-foreground",
                    survol === modele.contenu.blocs.length && "border-primary text-primary",
                  )}
                >
                  déposer ici pour ajouter à la fin
                </div>
              </div>
            </div>
          </div>

          {/* ---- Colonne 2 : propriétés ---- */}
          <div className="min-h-0 overflow-y-auto rounded-lg border p-3">
            <ReglagesModele
              modele={modele}
              onChange={(patch) => { setModele({ ...modele, ...patch }); setModifie(true); }}
              onPage={(patch) => {
                setModele({
                  ...modele,
                  contenu: { ...modele.contenu, page: { ...modele.contenu.page, ...patch } },
                });
                setModifie(true);
              }}
            />
            <div className="my-3 border-t" />
            {selection ? (
              <>
              <ProprietesBloc
                bloc={selection}
                champsDispo={champsDispo}
                colonnesDispo={colonnesDispo}
                onImporterImage={importerImage}
                imagesLibres={imagesLibres}
                onImporterLibre={importerLibre}
                onSupprimerLibre={supprimerLibre}
                blocs={modele.contenu.blocs}
                onMajBloc={majBloc}
                onChange={(patch) => majBloc(selection.id, patch)}
                format={modele?.format ?? "a4"}
                images={images}
              />
              <SectionFlottant
                bloc={selection}
                onChange={(patch) => majBloc(selection.id, patch)}
                cadreActuel={() => {
                  const doc = apercuRef.current?.contentDocument;
                  const el = doc?.querySelector<HTMLElement>(`[data-bloc="${selection.id}"]`);
                  return doc && el ? cadreDansLaFeuille(doc, el) : null;
                }}
              />
              </>
            ) : (
              <p className="text-xs text-muted-foreground">
                Choisir un bloc dans la structure pour en régler le contenu.
              </p>
            )}
          </div>

          {/* ---- Colonne 3 : aperçu ---- */}
          <div className="flex min-h-0 flex-col rounded-lg border bg-muted/30">
            <div className="flex items-center gap-2 border-b px-3 py-1.5">
              <FileText className="h-3.5 w-3.5 text-muted-foreground" />
              <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Aperçu — données d'exemple
              </span>
              {modele.actif && <Badge variant="secondary" className="text-[10px]">Modèle actif</Badge>}
              {modifie && <Badge variant="outline" className="text-[10px]">Non enregistré</Badge>}
            </div>
            <iframe
              ref={apercuRef}
              title="Aperçu du modèle"
              srcDoc={apercu}
              onLoad={brancherApercu}
              // PAS de `allow-scripts` : aucun script du document ne
              // s'exécute, c'est la règle D50. `allow-same-origin` ne
              // donne rien à la page — il donne à l'ATELIER le droit de
              // lire ce DOM, donc de savoir sur quel bloc on a cliqué.
              sandbox="allow-same-origin"
              className="min-h-0 flex-1 border-0 bg-white"
            />
          </div>
        </div>
      )}
    </div>
  );
}

// =====================================================================
//  Réglages du modèle
// =====================================================================

function ReglagesModele({
  modele, onChange, onPage,
}: {
  modele: Modele;
  onChange: (p: Partial<Modele>) => void;
  onPage: (p: Partial<Modele["contenu"]["page"]>) => void;
}) {
  return (
    <div className="space-y-2">
      <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        Le modèle
      </div>
      <div>
        <Label className="text-xs">Nom</Label>
        <Input
          className="h-8" value={modele.nom}
          onChange={(e) => onChange({ nom: e.target.value })}
        />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div>
          <Label className="text-xs">Format</Label>
          <Select
            value={modele.format}
            onValueChange={(v) => onChange({ format: v as FormatPapier })}
          >
            <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
            <SelectContent>
              {FORMATS.map((f) => (
                <SelectItem key={f.id} value={f.id}>{f.nom}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div>
          <Label className="text-xs">Marge (mm)</Label>
          <Input
            type="number" className="h-8" value={modele.contenu.page.margeMm}
            onChange={(e) => onPage({ margeMm: Number(e.target.value) || 0 })}
          />
        </div>
        <div>
          <Label className="text-xs">Taille du texte (pt)</Label>
          <Input
            type="number" step="0.5" className="h-8" value={modele.contenu.page.taillePt}
            onChange={(e) => onPage({ taillePt: Number(e.target.value) || 10 })}
          />
        </div>
        <div>
          <Label className="text-xs">Couleur d'accent</Label>
          <Input
            type="color" className="h-8 p-1" value={modele.contenu.page.couleurAccent}
            onChange={(e) => onPage({ couleurAccent: e.target.value })}
          />
        </div>
      </div>
    </div>
  );
}

// =====================================================================
//  Propriétés d'un bloc
// =====================================================================

const ALIGNEMENTS: { id: Alignement; icone: typeof AlignLeft }[] = [
  { id: "gauche", icone: AlignLeft },
  { id: "centre", icone: AlignCenter },
  { id: "droite", icone: AlignRight },
];

/**
 * FLOTTANT : sortir le bloc du flux pour le poser au millimètre, par-
 * dessus le reste. Les valeurs se règlent ici ou à la main dans
 * l'aperçu — c'est le même cadre.
 */
function SectionFlottant({ bloc, onChange, cadreActuel }: {
  bloc: Bloc;
  onChange: (patch: Record<string, unknown>) => void;
  /** Le cadre que le bloc occupe dans l'aperçu, s'il s'y trouve. */
  cadreActuel: () => CadreFlottant | null;
}) {
  const f = bloc.flottant;
  function basculer(actif: boolean) {
    if (!actif) { onChange({ flottant: undefined }); return; }
    // Le bloc reste là où il est : son cadre à l'écran devient son
    // cadre flottant. Sans aperçu (bloc caché), une vignette en haut à
    // gauche qu'on tire ensuite.
    const img = bloc.type === "image" ? bloc : null;
    onChange({ flottant: cadreActuel() ?? {
      xMm: 0, yMm: 0,
      largeurMm: img && img.largeurMm > 0 ? img.largeurMm : 40,
      hauteurMm: img && img.hauteurMm > 0 ? img.hauteurMm : 20,
    } satisfies CadreFlottant });
  }
  const champ = (cle: keyof CadreFlottant, libelle: string) => (
    <div>
      <Label className="text-[11px]">{libelle}</Label>
      <Input
        type="number" step="0.5" min={0} className="h-8"
        value={f?.[cle] ?? 0}
        onChange={(e) => f && onChange({ flottant: { ...f, [cle]: Math.max(0, Number(e.target.value) || 0) } })}
      />
    </div>
  );
  return (
    <div className="mt-3 space-y-2 border-t pt-3">
      <label className="flex items-center gap-2 text-xs">
        <input type="checkbox" checked={!!f} onChange={(e) => basculer(e.target.checked)} />
        Bloc flottant — posé au millimètre, par-dessus le reste
      </label>
      {f && (
        <>
          <div className="grid grid-cols-2 gap-2">
            {champ("xMm", "Gauche (mm)")}
            {champ("yMm", "Haut (mm)")}
            {champ("largeurMm", "Largeur (mm)")}
            {champ("hauteurMm", "Hauteur (mm)")}
          </div>
          <p className="text-[11px] text-muted-foreground">
            Dans l'aperçu : tirer le bloc pour le déplacer, son coin
            bas-droit pour le redimensionner. Deux blocs flottants peuvent
            se recouvrir ; celui qui vient plus bas dans la structure passe
            devant.
          </p>
        </>
      )}
      {!f && (
        <p className="text-[11px] text-muted-foreground">
          Ou, dans l'aperçu : tirer le coin bas-droit du bloc — il se
          détache à sa place et se redimensionne dans le même geste.
        </p>
      )}
    </div>
  );
}

function ChoixAlignement({
  valeur, onChange,
}: { valeur: Alignement; onChange: (a: Alignement) => void }) {
  return (
    <div className="flex gap-1">
      {ALIGNEMENTS.map(({ id, icone: Icone }) => (
        <Button
          key={id} type="button" size="sm"
          variant={valeur === id ? "default" : "outline"}
          className="h-8 w-8 p-0" onClick={() => onChange(id)}
        >
          <Icone className="h-3.5 w-3.5" />
        </Button>
      ))}
    </div>
  );
}

/**
 * Deux blocs qui posent la MÊME image : elle sortira deux fois.
 *
 * Le cas arrive tout seul — les modèles d'usine ont un bloc En-tête qui
 * porte déjà le logo, et poser un bloc Image « Logo » par-dessus le
 * double sans rien dire. On le dit, et on propose de retirer l'autre.
 */
function DoublonImage({
  image, moi, blocs, onMajBloc,
}: {
  image: "logo" | "entete" | "pied";
  moi: string;
  blocs: Bloc[];
  onMajBloc: (id: string, patch: Record<string, unknown>) => void;
}) {
  const autres = blocs.filter(
    (b) =>
      b.id !== moi &&
      b.visible &&
      ((b.type === "entete" && b.image === image) ||
        (b.type === "image" && b.image === image)),
  );
  if (autres.length === 0) return null;
  const entetes = autres.filter((b) => b.type === "entete");
  return (
    <div className="mt-1.5 rounded border border-amber-300 bg-amber-50 p-2
                    text-[11px] text-amber-900">
      {entetes.length > 0
        ? "Le bloc En-tête pose déjà cette image : elle sortira deux fois sur le document."
        : "Un autre bloc Image pose déjà cette image."}
      {entetes.length > 0 && (
        <Button variant="outline" size="sm"
          className="ml-2 h-6 bg-white text-[11px]"
          onClick={() => entetes.forEach((b) => onMajBloc(b.id, { image: "aucune" }))}>
          Retirer l'image de l'en-tête
        </Button>
      )}
    </div>
  );
}

/** Les trois images de la société, telles qu'on les nomme à l'écran. */
const NOM_IMAGE: Record<"logo" | "entete" | "pied", string> = {
  logo: "Logo",
  entete: "Bandeau d'en-tête",
  pied: "Bandeau de pied",
};

const IMAGES_POSABLES: { id: "logo" | "entete" | "pied"; nom: string }[] = [
  { id: "logo", nom: NOM_IMAGE.logo },
  { id: "entete", nom: NOM_IMAGE.entete },
  { id: "pied", nom: NOM_IMAGE.pied },
];

/**
 * Gras, italique, souligné — les trois, ensemble.
 *
 * Trois cases à cocher séparées prenaient trois lignes du panneau pour
 * un réglage que tout le monde connaît sous cette forme depuis trente
 * ans.
 */
function ChoixStyle({
  gras, italique, souligne, grasParDefaut = false, onChange,
}: {
  gras?: boolean;
  italique?: boolean;
  souligne?: boolean;
  /** Le titre est gras tant qu'on ne l'a pas décoché. */
  grasParDefaut?: boolean;
  onChange: (patch: Record<string, unknown>) => void;
}) {
  const etats: { cle: string; actif: boolean; Icone: typeof Bold; titre: string }[] = [
    { cle: "gras", actif: gras ?? grasParDefaut, Icone: Bold, titre: "Gras" },
    { cle: "italique", actif: !!italique, Icone: Italic, titre: "Italique" },
    { cle: "souligne", actif: !!souligne, Icone: Underline, titre: "Souligné" },
  ];
  return (
    <div className="flex gap-1">
      {etats.map(({ cle, actif, Icone, titre }) => (
        <Button
          key={cle} type="button" size="sm" title={titre}
          variant={actif ? "default" : "outline"}
          className="h-8 w-8 p-0" onClick={() => onChange({ [cle]: !actif })}
        >
          <Icone className="h-3.5 w-3.5" />
        </Button>
      ))}
    </div>
  );
}

const FORMATS_VALEUR: { id: FormatValeur; nom: string }[] = [
  { id: "texte", nom: "Texte" },
  { id: "montant", nom: "Montant" },
  { id: "nombre", nom: "Nombre" },
  { id: "pourcentage", nom: "Pourcentage (0,18 → 18 %)" },
  { id: "date", nom: "Date" },
  { id: "date_heure", nom: "Date et heure" },
];

function ChoixChemin({
  valeur, champs, onChange,
}: {
  valeur: string;
  champs: ChampDisponible[];
  /** Le format n'est donné QUE par la liste : une saisie à la main
   *  garde celui du champ, sinon régler le chemin d'une colonne
   *  « Montant » la remettait en texte brut à chaque frappe. */
  onChange: (chemin: string, format?: FormatValeur) => void;
}) {
  return (
    <div className="flex gap-1">
      <Input
        className="h-8 font-mono text-[11px]" value={valeur}
        onChange={(e) => onChange(e.target.value)}
        placeholder="piece.numero"
      />
      <Select
        value=""
        onValueChange={(v) => {
          const c = champs.find((x) => x.chemin === v);
          if (c) onChange(c.chemin, c.format);
        }}
      >
        <SelectTrigger className="h-8 w-8 p-0 [&>svg]:mx-auto"><span /></SelectTrigger>
        <SelectContent>
          {champs.map((c) => (
            <SelectItem key={c.chemin} value={c.chemin}>
              {c.libelle} <span className="text-muted-foreground">— {c.chemin}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function ProprietesBloc({
  bloc, champsDispo, colonnesDispo, onChange, format, images, onImporterImage,
  blocs, onMajBloc, imagesLibres, onImporterLibre, onSupprimerLibre,
}: {
  bloc: Bloc;
  /** Tous les blocs : de quoi repérer deux blocs qui posent la même image. */
  blocs: Bloc[];
  onMajBloc: (id: string, patch: Record<string, unknown>) => void;
  champsDispo: ChampDisponible[];
  /** Poser une image de la société depuis l'atelier. */
  onImporterImage: (genre: "logo" | "entete" | "pied") => void;
  /** Les images posées sur les documents, et leurs gestes. */
  imagesLibres: ImageLibre[];
  onImporterLibre: () => void;
  onSupprimerLibre: (id: string) => void;
  /** Les colonnes proposees pour un tableau, selon le genre. */
  colonnesDispo: ChampDisponible[];
  onChange: (patch: Record<string, unknown>) => void;
  /** Le format décide de la largeur utile du pied, en millimètres. */
  format: string;
  /** Pour montrer dans l'éditeur ce qui sera réellement imprimé. */
  images: ImagesDocument;
}) {
  const titre = (
    <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
      {NOM_TYPE[bloc.type]}
    </div>
  );

  switch (bloc.type) {
    case "entete":
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Image</Label>
            <Select value={bloc.image} onValueChange={(v) => onChange({ image: v })}>
              <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="logo">Logo</SelectItem>
                <SelectItem value="entete">Bandeau d'en-tête</SelectItem>
                <SelectItem value="aucune">Aucune</SelectItem>
              </SelectContent>
            </Select>
            <p className="mt-1 text-[11px] text-muted-foreground">
              Le logo se pose par le bloc Image, ci-contre. Les images ne
              voyagent pas dans l'export : ce sont celles de la boutique,
              pas du modèle.
            </p>
          </div>
          <div>
            <Label className="text-xs">Hauteur de l'image (mm)</Label>
            <Input
              type="number" className="h-8" value={bloc.hauteurMm}
              onChange={(e) => onChange({ hauteurMm: Number(e.target.value) || 0 })}
            />
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox" checked={bloc.afficherSociete}
              onChange={(e) => onChange({ afficherSociete: e.target.checked })}
            />
            Afficher les coordonnées de la société
          </label>
          <div>
            <Label className="text-xs">Alignement</Label>
            <ChoixAlignement valeur={bloc.alignement} onChange={(a) => onChange({ alignement: a })} />
          </div>
        </div>
      );

    case "titre":
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Texte</Label>
            <Input
              className="h-8" value={bloc.texte}
              onChange={(e) => onChange({ texte: e.target.value })}
            />
            <p className="mt-1 text-[11px] text-muted-foreground">
              {"{{piece.numero}}"} est remplacé à l'impression.
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label className="text-xs">Taille (pt)</Label>
              <Input
                type="number" step="0.5" className="h-8" value={bloc.taillePt}
                onChange={(e) => onChange({ taillePt: Number(e.target.value) || 12 })}
              />
            </div>
            <div>
              <Label className="text-xs">Alignement</Label>
              <ChoixAlignement valeur={bloc.alignement} onChange={(a) => onChange({ alignement: a })} />
            </div>
          </div>
          <div>
            <Label className="text-xs">Style</Label>
            <ChoixStyle
              gras={bloc.gras} italique={bloc.italique} souligne={bloc.souligne}
              grasParDefaut onChange={onChange}
            />
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox" checked={bloc.trait}
              onChange={(e) => onChange({ trait: e.target.checked })}
            />
            Trait sous le titre
          </label>
        </div>
      );

    case "texte":
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Contenu</Label>
            <textarea
              className="min-h-[110px] w-full rounded-md border bg-transparent p-2 text-xs"
              value={bloc.contenu}
              onChange={(e) => onChange({ contenu: e.target.value })}
            />
            <p className="mt-1 text-[11px] text-muted-foreground">
              Champs disponibles : {"{{tiers.nom}}"}, {"{{totaux.total_en_lettres}}"}…
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label className="text-xs">Taille (pt)</Label>
              <Input
                type="number" step="0.5" className="h-8" value={bloc.taillePt}
                onChange={(e) => onChange({ taillePt: Number(e.target.value) || 9 })}
              />
            </div>
            <div>
              <Label className="text-xs">Alignement</Label>
              <ChoixAlignement valeur={bloc.alignement} onChange={(a) => onChange({ alignement: a })} />
            </div>
          </div>
          <div>
            <Label className="text-xs">Style</Label>
            <ChoixStyle
              gras={bloc.gras} italique={bloc.italique} souligne={bloc.souligne}
              onChange={onChange}
            />
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input type="checkbox" checked={bloc.cadre}
                   onChange={(e) => onChange({ cadre: e.target.checked })} />
            Encadré
          </label>
        </div>
      );

    case "image":
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Quelle image</Label>
            {/* Deux familles. Les trois images de la SOCIÉTÉ sont
                partagées par tout le monde — les remplacer les change
                partout. Les images POSÉES ont chacune leur identité :
                un cachet, une signature, un QR, autant qu'on veut. */}
            <select
              className="h-8 w-full rounded-md border bg-transparent px-2 text-xs"
              value={bloc.imageId ? `libre:${bloc.imageId}` : `societe:${bloc.image}`}
              onChange={(e) => {
                const v = e.target.value;
                if (v.startsWith("libre:")) onChange({ imageId: v.slice(6) });
                else onChange({ imageId: undefined, image: v.slice(8) });
              }}
            >
              <optgroup label="Images de la société">
                {IMAGES_POSABLES.map((i) => (
                  <option key={i.id} value={`societe:${i.id}`}>{i.nom}</option>
                ))}
              </optgroup>
              <optgroup label="Images posées sur les documents">
                {imagesLibres.length === 0 && (
                  <option value="libre:" disabled>— aucune, importer ci-dessous —</option>
                )}
                {imagesLibres.map((i) => (
                  <option key={i.id} value={`libre:${i.id}`}>{i.nom}</option>
                ))}
              </optgroup>
            </select>

            <div className="mt-1.5 flex flex-wrap items-center gap-2">
              <Button variant="outline" size="sm" className="h-7 text-xs"
                onClick={onImporterLibre}
                title="Une image de plus, avec sa propre identité">
                <Upload className="mr-1 h-3 w-3" /> Importer une image…
              </Button>
              {!bloc.imageId && (
                <Button variant="outline" size="sm" className="h-7 text-xs"
                  onClick={() => onImporterImage(bloc.image)}
                  title="Remplace cette image de la société partout où elle est posée">
                  {images[bloc.image] ? "Remplacer celle de la société…" : "Poser celle de la société…"}
                </Button>
              )}
              {bloc.imageId && (
                <Button variant="ghost" size="sm" className="h-7 text-xs text-destructive"
                  onClick={() => onSupprimerLibre(bloc.imageId as string)}
                  title="Refusé tant qu'un modèle la pose">
                  <Trash2 className="mr-1 h-3 w-3" /> Supprimer
                </Button>
              )}
              {(bloc.imageId ? images.libres?.[bloc.imageId] : images[bloc.image]) && (
                <img
                  src={(bloc.imageId ? images.libres?.[bloc.imageId] : images[bloc.image]) as string}
                  alt="" className="h-7 rounded border bg-white object-contain" />
              )}
            </div>
            {!bloc.imageId && (
              <DoublonImage
                image={bloc.image} moi={bloc.id} blocs={blocs} onMajBloc={onMajBloc}
              />
            )}
            <p className="mt-1 text-[11px] text-muted-foreground">
              {bloc.imageId
                ? "Une image posée ne se supprime pas tant qu'un modèle l'utilise : le serveur nomme lesquels."
                : "Une image de la société est partagée : la remplacer la change partout où elle est posée."}
              {" "}Absente, le bloc reste vide à l'impression.
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label className="text-xs">Largeur (mm)</Label>
              <Input
                type="number" step="1" min="0" className="h-8" value={bloc.largeurMm}
                onChange={(e) => onChange({ largeurMm: Number(e.target.value) || 0 })}
              />
            </div>
            <div>
              <Label className="text-xs">Hauteur (mm)</Label>
              <Input
                type="number" step="1" min="0" className="h-8" value={bloc.hauteurMm}
                onChange={(e) => onChange({ hauteurMm: Number(e.target.value) || 0 })}
              />
            </div>
          </div>
          <p className="text-[11px] text-muted-foreground">
            0 = libre. L'image garde ses proportions et rentre dans la
            case donnée : elle ne déborde jamais sur le reste.
          </p>
          <div>
            <Label className="text-xs">Alignement</Label>
            <ChoixAlignement valeur={bloc.alignement} onChange={(a) => onChange({ alignement: a })} />
          </div>
        </div>
      );

    case "pied_page":
      return (
        <div className="space-y-2">
          {titre}
          <p className="text-[11px] text-muted-foreground">
            Le pied se DESSINE : les autres blocs s'enchaînent de haut en
            bas, celui-ci a une hauteur fixe et on y pose les choses côte
            à côte. Il s'imprime en bas de chaque page.
          </p>
          <EditeurPiedPage
            bloc={bloc}
            format={format}
            images={images}
            onChange={onChange}
          />
        </div>
      );

    case "signatures":
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Signature de gauche</Label>
            <Input className="h-8" value={bloc.gauche}
                   onChange={(e) => onChange({ gauche: e.target.value })} />
          </div>
          <div>
            <Label className="text-xs">Signature de droite</Label>
            <Input className="h-8" value={bloc.droite}
                   onChange={(e) => onChange({ droite: e.target.value })} />
          </div>
          <p className="text-[11px] text-muted-foreground">
            Les deux vides : le bloc ne s'imprime pas.
          </p>
        </div>
      );

    case "espace":
      return (
        <div className="space-y-2">
          {titre}
          <Label className="text-xs">Hauteur (mm)</Label>
          <Input
            type="number" className="h-8" value={bloc.hauteurMm}
            onChange={(e) => onChange({ hauteurMm: Number(e.target.value) || 0 })}
          />
        </div>
      );

    case "champs":
    case "totaux": {
      const items = bloc.items;
      const maj = (liste: Champ[]) => onChange({ items: liste });
      return (
        <div className="space-y-2">
          {titre}
          {bloc.type === "champs" && (
            <>
              <div>
                <Label className="text-xs">Titre du bloc</Label>
                <Input className="h-8" value={bloc.titre}
                       onChange={(e) => onChange({ titre: e.target.value })} />
              </div>
              <div>
                <Label className="text-xs">Colonnes</Label>
                <Select value={String(bloc.colonnes)}
                        onValueChange={(v) => onChange({ colonnes: Number(v) })}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">1</SelectItem>
                    <SelectItem value="2">2</SelectItem>
                    <SelectItem value="3">3</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </>
          )}
          {bloc.type === "totaux" && (
            <label className="flex items-center gap-2 text-xs">
              <input type="checkbox" checked={bloc.accentuerDernier}
                     onChange={(e) => onChange({ accentuerDernier: e.target.checked })} />
              Mettre le dernier total en évidence
            </label>
          )}

          <ListeEditable
            titre={bloc.type === "totaux" ? "Totaux" : "Champs"}
            elements={items}
            onAjouter={() => maj([...items, {
              id: nouvelId("c"), libelle: "Nouveau", chemin: "", format: "texte", masquerSiVide: true,
            }])}
            onSupprimer={(i) => maj(items.filter((_, k) => k !== i))}
            onDeplacer={(i, d) => {
              const copie = [...items];
              const j = i + d;
              if (j < 0 || j >= copie.length) return;
              [copie[i], copie[j]] = [copie[j], copie[i]];
              maj(copie);
            }}
            rendu={(c, i) => (
              <div className="space-y-1">
                <Input
                  className="h-7 text-xs" value={c.libelle} placeholder="Libellé"
                  onChange={(e) => {
                    const copie = [...items];
                    copie[i] = { ...c, libelle: e.target.value };
                    maj(copie);
                  }}
                />
                <ChoixChemin
                  valeur={c.chemin} champs={champsDispo}
                  onChange={(chemin, format) => {
                    const copie = [...items];
                    copie[i] = { ...c, chemin, format: format ?? c.format };
                    maj(copie);
                  }}
                />
                <div className="flex items-center gap-2">
                  <Select
                    value={c.format}
                    onValueChange={(v) => {
                      const copie = [...items];
                      copie[i] = { ...c, format: v as FormatValeur };
                      maj(copie);
                    }}
                  >
                    <SelectTrigger className="h-7 flex-1 text-xs"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      {FORMATS_VALEUR.map((f) => (
                        <SelectItem key={f.id} value={f.id}>{f.nom}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <label className="flex items-center gap-1 text-[11px]" title="Ne rien imprimer si la valeur est vide">
                    <input
                      type="checkbox" checked={c.masquerSiVide}
                      onChange={(e) => {
                        const copie = [...items];
                        copie[i] = { ...c, masquerSiVide: e.target.checked };
                        maj(copie);
                      }}
                    />
                    si vide
                  </label>
                </div>
              </div>
            )}
          />
        </div>
      );
    }

    case "tableau": {
      const cols = bloc.colonnes;
      const maj = (liste: Colonne[]) => onChange({ colonnes: liste });
      const somme = cols.reduce((s, c) => s + (Number(c.largeur) || 0), 0);
      return (
        <div className="space-y-2">
          {titre}
          <div>
            <Label className="text-xs">Liste parcourue</Label>
            <Input
              className="h-8 font-mono text-[11px]" value={bloc.source}
              onChange={(e) => onChange({ source: e.target.value })}
            />
          </div>
          <div>
            <Label className="text-xs">Texte si la liste est vide</Label>
            <Input className="h-8" value={bloc.siVide}
                   onChange={(e) => onChange({ siVide: e.target.value })} />
          </div>
          <label className="flex items-center gap-2 text-xs">
            <input type="checkbox" checked={bloc.zebre}
                   onChange={(e) => onChange({ zebre: e.target.checked })} />
            Lignes alternées
          </label>

          {/* ---- Habillage : filets, couleurs, coins ---- */}
          <div className="rounded-md border p-2 space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wide
                          text-muted-foreground">Habillage</p>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label className="text-xs">Filets</Label>
                <select
                  className="h-8 w-full rounded-md border bg-transparent px-2 text-xs"
                  value={bloc.bordures ?? "lignes"}
                  onChange={(e) => onChange({ bordures: e.target.value })}
                >
                  <option value="lignes">Sous chaque ligne</option>
                  <option value="grille">Grille complète</option>
                  <option value="aucune">Aucun</option>
                </select>
              </div>
              <div>
                <Label className="text-xs">Couleur des filets</Label>
                <Input type="color" className="h-8 p-1"
                  value={bloc.couleurBordure ?? "#dddddd"}
                  onChange={(e) => onChange({ couleurBordure: e.target.value })} />
              </div>
              <div>
                <Label className="text-xs">Fond de l'en-tête</Label>
                <Input type="color" className="h-8 p-1"
                  value={bloc.couleurEntete ?? "#ffffff"}
                  onChange={(e) => onChange({ couleurEntete: e.target.value })} />
              </div>
              <div>
                <Label className="text-xs">Texte de l'en-tête</Label>
                <Input type="color" className="h-8 p-1"
                  value={bloc.couleurTexteEntete ?? "#000000"}
                  onChange={(e) => onChange({ couleurTexteEntete: e.target.value })} />
              </div>
              {bloc.zebre && (
                <div>
                  <Label className="text-xs">Ligne alternée</Label>
                  <Input type="color" className="h-8 p-1"
                    value={bloc.couleurZebre ?? "#f5f5f5"}
                    onChange={(e) => onChange({ couleurZebre: e.target.value })} />
                </div>
              )}
            </div>

            <div>
              <Label className="text-xs">Coins arrondis (mm)</Label>
              <div className="grid grid-cols-4 gap-1">
                {([
                  ["hg", "Haut g."], ["hd", "Haut d."],
                  ["bg", "Bas g."], ["bd", "Bas d."],
                ] as const).map(([cle, nom]) => (
                  <div key={cle}>
                    <Input type="number" min="0" step="0.5" className="h-7 text-xs"
                      value={bloc.arrondiMm?.[cle] ?? 0}
                      onChange={(e) => onChange({
                        arrondiMm: {
                          hg: 0, hd: 0, bd: 0, bg: 0,
                          ...(bloc.arrondiMm ?? {}),
                          [cle]: Number(e.target.value) || 0,
                        },
                      })} />
                    <p className="text-center text-[10px] text-muted-foreground">{nom}</p>
                  </div>
                ))}
              </div>
              <p className="mt-1 text-[11px] text-muted-foreground">
                Chaque coin le sien. Un rayon pose un cadre autour du
                tableau : sans lui, le navigateur ignore l'arrondi.
              </p>
            </div>
          </div>

          <div className={cn(
            "rounded border px-2 py-1 text-[11px]",
            somme === 100 ? "text-muted-foreground" : "border-amber-400 text-amber-700",
          )}>
            Somme des largeurs : {somme} %
            {somme !== 100 && " — le tableau sera étiré ou tronqué."}
          </div>

          <ListeEditable
            titre="Colonnes"
            elements={cols}
            onAjouter={() => maj([...cols, {
              id: nouvelId("col"), libelle: "Colonne", chemin: "",
              largeur: 20, alignement: "gauche", format: "texte",
            }])}
            onSupprimer={(i) => maj(cols.filter((_, k) => k !== i))}
            onDeplacer={(i, d) => {
              const copie = [...cols];
              const j = i + d;
              if (j < 0 || j >= copie.length) return;
              [copie[i], copie[j]] = [copie[j], copie[i]];
              maj(copie);
            }}
            rendu={(c, i) => (
              <div className="space-y-1">
                <Input
                  className="h-7 text-xs" value={c.libelle} placeholder="En-tête"
                  onChange={(e) => {
                    const copie = [...cols];
                    copie[i] = { ...c, libelle: e.target.value };
                    maj(copie);
                  }}
                />
                <ChoixChemin
                  valeur={c.chemin} champs={colonnesDispo}
                  onChange={(chemin, format) => {
                    const copie = [...cols];
                    copie[i] = { ...c, chemin, format: format ?? c.format };
                    maj(copie);
                  }}
                />
                <div className="flex items-center gap-1">
                  <Input
                    type="number" className="h-7 w-16 text-xs" value={c.largeur}
                    onChange={(e) => {
                      const copie = [...cols];
                      copie[i] = { ...c, largeur: Number(e.target.value) || 0 };
                      maj(copie);
                    }}
                  />
                  <span className="text-[11px] text-muted-foreground">%</span>
                  <Select
                    value={c.format}
                    onValueChange={(v) => {
                      const copie = [...cols];
                      copie[i] = { ...c, format: v as FormatValeur };
                      maj(copie);
                    }}
                  >
                    <SelectTrigger className="h-7 flex-1 text-xs"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      {FORMATS_VALEUR.map((f) => (
                        <SelectItem key={f.id} value={f.id}>{f.nom}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <ChoixAlignement
                    valeur={c.alignement}
                    onChange={(a) => {
                      const copie = [...cols];
                      copie[i] = { ...c, alignement: a };
                      maj(copie);
                    }}
                  />
                </div>
              </div>
            )}
          />
        </div>
      );
    }

    default:
      return (
        <div>
          {titre}
          <p className="text-xs text-muted-foreground">Ce bloc n'a aucun réglage.</p>
        </div>
      );
  }
}

// =====================================================================
//  Liste éditable — champs, colonnes
// =====================================================================

function ListeEditable<T extends { id: string }>({
  titre, elements, rendu, onAjouter, onSupprimer, onDeplacer,
}: {
  titre: string;
  elements: T[];
  rendu: (e: T, i: number) => React.ReactNode;
  onAjouter: () => void;
  onSupprimer: (i: number) => void;
  onDeplacer: (i: number, direction: -1 | 1) => void;
}) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <Label className="text-xs">{titre}</Label>
        <Button type="button" size="sm" variant="ghost" className="h-6 px-2"
                onClick={onAjouter}>
          <Plus className="h-3.5 w-3.5" />
        </Button>
      </div>
      <div className="space-y-2">
        {elements.map((e, i) => (
          <div key={e.id} className="rounded border p-1.5">
            <div className="mb-1 flex items-center justify-end gap-0.5">
              <button className="p-0.5" title="Monter" onClick={() => onDeplacer(i, -1)}>
                <ArrowUp className="h-3 w-3 text-muted-foreground" />
              </button>
              <button className="p-0.5" title="Descendre" onClick={() => onDeplacer(i, 1)}>
                <ArrowDown className="h-3 w-3 text-muted-foreground" />
              </button>
              <button className="p-0.5" title="Retirer" onClick={() => onSupprimer(i)}>
                <Minus className="h-3 w-3 text-destructive" />
              </button>
            </div>
            {rendu(e, i)}
          </div>
        ))}
        {elements.length === 0 && (
          <p className="text-[11px] text-muted-foreground">Aucun élément.</p>
        )}
      </div>
    </div>
  );
}
