// components/OngletDocuments.tsx — Paramètres → Documents (v3, A-1).
//
// Le commerçant ne dessine pas sa facture (D17) : il pose son papier à
// en-tête, coche ce qui apparaît, nomme au plus trois signatures par
// genre (D18). Tout le reste est fixe, dans le générateur.
//
// Les messages s'affichent dans l'onglet, pas dans une boîte de
// dialogue : on voit ce qui a été enregistré à côté de ce qu'on a
// changé.

import { useEffect, useMemo, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import {
  Loader2, Save, Upload, X, ImageIcon, RotateCcw, Plus, Trash2, Stamp,
  CheckCircle2, AlertTriangle, PanelTop, PanelBottom,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import {
  type Genre, type ReglageGenre, type ReglagesDocuments, type Choix,
  type Coordonnee, type FormatDocument,
  LIBELLES_GENRE, PORTEE_GENRE, LIBELLES_FORMAT, LIBELLES_COORDONNEE,
  SIGNATURES_MAX, lireReglagesDocuments, choisirImage, contenuBase64,
} from "@/lib/documents";
import { documentExemple } from "@/lib/exemples-documents";
import type { FormatImpression } from "@/lib/genererPDF";

type Avis = { type: "ok" | "erreur"; texte: string } | null;

const COORDONNEES: Coordonnee[] = [
  "adresse", "telephone", "telephone2", "email", "site_web", "nif", "rccm",
];

function AvisLigne({ avis }: { avis: Avis }) {
  if (!avis) return null;
  const ok = avis.type === "ok";
  return (
    <p role={ok ? "status" : "alert"}
      className={cn("text-xs flex items-center gap-1.5",
        ok ? "text-emerald-700" : "text-destructive")}>
      {ok ? <CheckCircle2 className="h-3.5 w-3.5" /> : <AlertTriangle className="h-3.5 w-3.5" />}
      {avis.texte}
    </p>
  );
}

/** Trois positions : laisser le document décider, forcer, retirer. */
function ChoixTriple({
  valeur, onChange, etiquette, aide,
}: { valeur: Choix; onChange: (c: Choix) => void; etiquette: string; aide: string }) {
  const options: { v: Choix; l: string }[] = [
    { v: "auto", l: "Auto" }, { v: "oui", l: "Toujours" }, { v: "non", l: "Jamais" },
  ];
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <div>
        <p className="text-sm">{etiquette}</p>
        <p className="text-xs text-muted-foreground">{aide}</p>
      </div>
      <div role="radiogroup" aria-label={etiquette}
        className="inline-flex rounded-md border border-border overflow-hidden shrink-0">
        {options.map(o => (
          <button key={o.v} type="button" role="radio" aria-checked={valeur === o.v}
            onClick={() => onChange(o.v)}
            className={cn("px-2.5 py-1 text-xs border-l border-border first:border-l-0",
              valeur === o.v ? "bg-primary text-primary-foreground" : "hover:bg-muted")}>
            {o.l}
          </button>
        ))}
      </div>
    </div>
  );
}

function Case({
  coche, onChange, etiquette, aide,
}: { coche: boolean; onChange: (b: boolean) => void; etiquette: string; aide?: string }) {
  return (
    <label className="flex items-start gap-2.5 py-1.5 cursor-pointer">
      <input type="checkbox" checked={coche} onChange={e => onChange(e.target.checked)}
        className="mt-0.5 h-4 w-4 accent-primary" />
      <span>
        <span className="text-sm block">{etiquette}</span>
        {aide && <span className="text-xs text-muted-foreground block">{aide}</span>}
      </span>
    </label>
  );
}

// =====================================================================
//  Papier à en-tête : deux images de la société
// =====================================================================

function ImageSociete({
  titre, aide, icone: Icone, image, onPoser, onRetirer, occupe,
}: {
  titre: string; aide: string; icone: typeof PanelTop;
  image: string | null; occupe: boolean;
  onPoser: () => void; onRetirer: () => void;
}) {
  return (
    <div className="rounded-lg border border-border p-3 flex flex-col gap-2">
      <p className="text-sm font-medium flex items-center gap-1.5">
        <Icone className="h-4 w-4 text-muted-foreground" /> {titre}
      </p>
      <div className="h-20 rounded-md border border-dashed border-border bg-muted/30
                      flex items-center justify-center overflow-hidden">
        {image
          ? <img src={image} alt={titre} className="max-h-full max-w-full object-contain" />
          : <ImageIcon className="h-6 w-6 text-muted-foreground" />}
      </div>
      <p className="text-xs text-muted-foreground">{aide}</p>
      <div className="flex gap-2">
        <Button size="sm" variant="outline" onClick={onPoser} disabled={occupe}>
          {occupe ? <Loader2 className="h-4 w-4 animate-spin" />
            : <><Upload className="h-4 w-4 mr-1.5" />{image ? "Changer" : "Téléverser"}</>}
        </Button>
        {image && (
          <Button size="sm" variant="ghost" onClick={onRetirer} disabled={occupe}>
            <X className="h-4 w-4 mr-1" /> Retirer
          </Button>
        )}
      </div>
    </div>
  );
}

// =====================================================================
//  L'exemple, réduit
// =====================================================================

const LARGEUR_PX: Record<string, number> = { a4: 794, a5: 559, thermique_80: 302, thermique_58: 220 };
const HAUTEUR_PX: Record<string, number> = { a4: 1123, a5: 794, thermique_80: 700, thermique_58: 700 };

function ApercuExemple({ html, format }: { html: string; format: string }) {
  const largeur = LARGEUR_PX[format] ?? 794;
  const hauteur = HAUTEUR_PX[format] ?? 1123;
  // Tout tient dans 400 px de large, sans agrandir un ticket.
  const echelle = Math.min(1, 400 / largeur);
  return (
    <figure className="xl:sticky xl:top-4 space-y-1.5" aria-label="Aperçu d'exemple">
      <figcaption className="text-xs text-muted-foreground">
        Aperçu d'exemple — données fictives, votre société. Rien n'est enregistré tant
        que vous n'avez pas cliqué sur Enregistrer.
      </figcaption>
      <div className="rounded-md border border-border bg-muted/40 p-2 overflow-hidden"
        style={{ height: hauteur * echelle + 16 }}>
        <iframe title="Exemple de document" srcDoc={html} sandbox="allow-same-origin"
          style={{
            width: largeur, height: hauteur, border: "none", background: "#fff",
            transform: `scale(${echelle})`, transformOrigin: "top left",
            boxShadow: "0 1px 6px rgba(0,0,0,.12)",
          }} />
      </div>
    </figure>
  );
}

// =====================================================================
//  L'onglet
// =====================================================================

export function OngletDocuments() {
  const [reglages, setReglages] = useState<ReglagesDocuments | null>(null);
  const [genre, setGenre] = useState<Genre>("facture");
  const [brouillon, setBrouillon] = useState<ReglageGenre | null>(null);
  const [entete, setEntete] = useState<string | null>(null);
  const [pied, setPied] = useState<string | null>(null);
  const [piedFacture, setPiedFacture] = useState<string>("");
  const [societe, setSociete] = useState<Record<string, unknown>>({ nom: "Ma société" });
  const [logo, setLogo] = useState<string | null>(null);
  const [erreurChargement, setErreurChargement] = useState<string | null>(null);
  const [occupe, setOccupe] = useState<string | null>(null);
  const [avisPapier, setAvisPapier] = useState<Avis>(null);
  const [avisGenre, setAvisGenre] = useState<Avis>(null);

  useEffect(() => {
    Promise.all([
      lireReglagesDocuments(),
      invoke<string | null>("lire_entete_base64").catch(() => null),
      invoke<string | null>("lire_pied_base64").catch(() => null),
      invoke<Record<string, unknown>>("lire_parametres_societe").catch(() => ({})),
      invoke<string | null>("lire_logo_base64").catch(() => null),
    ])
      .then(([r, e, p, s, l]) => {
        setReglages(r); setEntete(e); setPied(p); setLogo(l);
        setSociete({ nom: "Ma société", ...(s as Record<string, unknown>) });
        setPiedFacture(String((s as { pied_facture?: string }).pied_facture ?? ""));
        setBrouillon(r.genres.facture);
      })
      .catch(e => setErreurChargement(String(e)));
  }, []);

  function choisirGenre(g: Genre) {
    if (!reglages) return;
    setGenre(g);
    setBrouillon(reglages.genres[g]);
    setAvisGenre(null);
  }

  const modifie = !!reglages && !!brouillon
    && JSON.stringify(reglages.genres[genre]) !== JSON.stringify(brouillon);

  function changer<K extends keyof ReglageGenre>(cle: K, valeur: ReglageGenre[K]) {
    setBrouillon(b => (b ? { ...b, [cle]: valeur } : b));
    setAvisGenre(null);
  }

  function appliquer(g: Genre, r: ReglageGenre) {
    setReglages(prev => (prev ? { ...prev, genres: { ...prev.genres, [g]: r } } : prev));
    if (g === genre) setBrouillon(r);
  }

  // ---- Papier à en-tête ----------------------------------------------

  async function poserImageSociete(quoi: "entete" | "pied") {
    setAvisPapier(null);
    try {
      const f = await choisirImage("image/png,image/jpeg,image/webp,image/svg+xml");
      if (!f) return;
      setOccupe(quoi);
      await invoke(quoi === "entete" ? "sauvegarder_entete" : "sauvegarder_pied",
        { nom: f.nom, contenu: contenuBase64(f.dataUrl) });
      const relue = await invoke<string | null>(quoi === "entete" ? "lire_entete_base64" : "lire_pied_base64");
      (quoi === "entete" ? setEntete : setPied)(relue);
      setAvisPapier({ type: "ok", texte: quoi === "entete" ? "En-tête enregistré." : "Pied de page enregistré." });
    } catch (e) {
      setAvisPapier({ type: "erreur", texte: String(e) });
    } finally {
      setOccupe(null);
    }
  }

  async function retirerImageSociete(quoi: "entete" | "pied") {
    setAvisPapier(null);
    setOccupe(quoi);
    try {
      await invoke(quoi === "entete" ? "supprimer_entete" : "supprimer_pied");
      (quoi === "entete" ? setEntete : setPied)(null);
      setAvisPapier({ type: "ok", texte: quoi === "entete" ? "En-tête retiré." : "Pied de page retiré." });
    } catch (e) {
      setAvisPapier({ type: "erreur", texte: String(e) });
    } finally {
      setOccupe(null);
    }
  }

  async function basculerCoordonnee(c: Coordonnee, coche: boolean) {
    if (!reglages) return;
    const choix = coche
      ? [...reglages.coordonnees, c]
      : reglages.coordonnees.filter(x => x !== c);
    setAvisPapier(null);
    // La case bouge tout de suite ; un refus du serveur la remet.
    const avant = reglages.coordonnees;
    setReglages({ ...reglages, coordonnees: choix });
    try {
      const enregistre = await invoke<Coordonnee[]>("enregistrer_coordonnees_documents", { coordonnees: choix });
      setReglages(r => (r ? { ...r, coordonnees: enregistre } : r));
    } catch (e) {
      setReglages(r => (r ? { ...r, coordonnees: avant } : r));
      setAvisPapier({ type: "erreur", texte: String(e) });
    }
  }

  // ---- Le genre --------------------------------------------------------

  async function enregistrer() {
    if (!brouillon) return;
    setOccupe("genre");
    setAvisGenre(null);
    try {
      // Les images ne partent pas avec le réglage : elles ont leur
      // commande, et le serveur garde celles des emplacements restants.
      const reglage = {
        ...brouillon,
        signatures: brouillon.signatures.map(s => ({ libelle: s.libelle })),
      };
      const r = await invoke<ReglageGenre>("enregistrer_reglage_document", { genre, reglage });
      appliquer(genre, r);
      setAvisGenre({ type: "ok", texte: `${LIBELLES_GENRE[genre]} : réglages enregistrés.` });
    } catch (e) {
      setAvisGenre({ type: "erreur", texte: String(e) });
    } finally {
      setOccupe(null);
    }
  }

  async function retablir() {
    setOccupe("genre");
    setAvisGenre(null);
    try {
      const r = await invoke<ReglageGenre>("retablir_reglage_document", { genre });
      appliquer(genre, r);
      setAvisGenre({ type: "ok", texte: `${LIBELLES_GENRE[genre]} : réglages d'usine rétablis.` });
    } catch (e) {
      setAvisGenre({ type: "erreur", texte: String(e) });
    } finally {
      setOccupe(null);
    }
  }

  async function poserCachet(rang: number, retirer = false) {
    setAvisGenre(null);
    try {
      let image: string | null = null;
      if (!retirer) {
        const f = await choisirImage();
        if (!f) return;
        image = f.dataUrl;
      }
      setOccupe(`cachet-${rang}`);
      const r = await invoke<ReglageGenre>("poser_image_signature", { genre, rang, image });
      // Le réglage enregistré revient avec ses images ; le brouillon
      // garde ses libellés en cours de saisie.
      setReglages(prev => (prev ? { ...prev, genres: { ...prev.genres, [genre]: r } } : prev));
      setBrouillon(b => (b ? {
        ...b,
        signatures: b.signatures.map((s, i) => ({ ...s, image: r.signatures[i]?.image ?? null })),
      } : b));
      setAvisGenre({ type: "ok", texte: retirer ? "Image retirée." : "Image posée sur la signature." });
    } catch (e) {
      setAvisGenre({ type: "erreur", texte: String(e) });
    } finally {
      setOccupe(null);
    }
  }

  // L'exemple se redessine à chaque case cochée : on voit l'effet
  // AVANT d'enregistrer. Même générateur que l'impression.
  const apercu = useMemo(() => {
    if (!reglages || !brouillon) return "";
    return documentExemple(genre, brouillon.format as FormatImpression, {
      logo, entete, pied, reglage: brouillon, coordonnees: reglages.coordonnees,
    }, societe).replace(/<script>[\s\S]*?<\/script>/g, "");
  }, [genre, brouillon, reglages, logo, entete, pied, societe]);

  if (erreurChargement) {
    return (
      <div className="rounded-lg border border-destructive/40 bg-destructive/5 p-4 text-sm">
        Réglages des documents indisponibles : {erreurChargement}
      </div>
    );
  }
  if (!reglages || !brouillon) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const estTicket = genre === "ticket";
  // Un bulletin de paie se remet sur une page, pas sur un rouleau.
  const formats: FormatDocument[] = estTicket
    ? ["thermique_80", "thermique_58"]
    : genre === "bulletin" ? ["a4", "a5"]
    : ["a4", "a5", "thermique_80", "thermique_58"];
  // Un cachet ne se pose que sur une signature ENREGISTRÉE : le serveur
  // refuserait un emplacement qu'il ne connaît pas encore.
  const enregistrees = reglages.genres[genre].signatures.length;

  return (
    <div className="space-y-8 max-w-6xl">
      {/* ---- Papier à en-tête ---- */}
      <section className="space-y-3 max-w-3xl" aria-labelledby="titre-papier">
        <div>
          <h2 id="titre-papier" className="text-base font-semibold">Papier à en-tête</h2>
          <p className="text-sm text-muted-foreground">
            Les images de votre papier, faites chez l'imprimeur. Elles s'impriment
            sur tous les documents A4 et A5 — pas sur les tickets.
          </p>
        </div>
        <div className="grid sm:grid-cols-2 gap-3">
          <ImageSociete titre="En-tête" icone={PanelTop} image={entete}
            aide="Largeur de la page, environ 3 cm de haut. Remplace le nom et les coordonnées."
            occupe={occupe === "entete"}
            onPoser={() => poserImageSociete("entete")}
            onRetirer={() => retirerImageSociete("entete")} />
          <ImageSociete titre="Pied de page" icone={PanelBottom} image={pied}
            aide="En bas de chaque page. Remplace la mention de bas de page."
            occupe={occupe === "pied"}
            onPoser={() => poserImageSociete("pied")}
            onRetirer={() => retirerImageSociete("pied")} />
        </div>
        <div className="rounded-lg border border-border p-3">
          <p className="text-sm font-medium">Sans image d'en-tête : sous le nom de la société</p>
          <p className="text-xs text-muted-foreground mb-2">
            Le nom s'affiche toujours. Cochez les coordonnées à montrer.
            {entete && " (Ignoré tant qu'un en-tête image est posé.)"}
          </p>
          <div className="flex flex-wrap gap-x-5 gap-y-1">
            {COORDONNEES.map(c => (
              <label key={c} className="flex items-center gap-1.5 text-sm cursor-pointer">
                <input type="checkbox" className="h-4 w-4 accent-primary"
                  checked={reglages.coordonnees.includes(c)}
                  onChange={e => basculerCoordonnee(c, e.target.checked)} />
                {LIBELLES_COORDONNEE[c]}
              </label>
            ))}
          </div>
        </div>
        <AvisLigne avis={avisPapier} />
      </section>

      {/* ---- Par genre ---- */}
      <section className="space-y-4" aria-labelledby="titre-genres">
        <div>
          <h2 id="titre-genres" className="text-base font-semibold">Chaque document</h2>
          <p className="text-sm text-muted-foreground">
            La mise en page est fixe ; ce qui se règle, c'est le format, quelques
            colonnes et les signatures.
          </p>
        </div>

        <div role="tablist" aria-label="Genre de document" className="flex flex-wrap gap-1.5">
          {reglages.ordre.map(g => (
            <button key={g} role="tab" aria-selected={genre === g} type="button"
              onClick={() => choisirGenre(g)}
              className={cn("px-3 py-1.5 rounded-md border text-sm transition-colors",
                genre === g
                  ? "border-primary bg-primary/5 text-primary font-medium"
                  : "border-border text-muted-foreground hover:bg-muted")}>
              {LIBELLES_GENRE[g]}
            </button>
          ))}
        </div>

        <div className="grid xl:grid-cols-[minmax(0,1fr)_420px] gap-6 items-start">
        <div className="rounded-lg border border-border p-4 space-y-5">
          <p className="text-xs text-muted-foreground">{PORTEE_GENRE[genre]}</p>

          <div>
            <Label className="mb-1.5 block">Format par défaut</Label>
            <div role="radiogroup" aria-label="Format par défaut" className="flex flex-wrap gap-1.5">
              {formats.map(f => (
                <button key={f} type="button" role="radio" aria-checked={brouillon.format === f}
                  onClick={() => changer("format", f)}
                  className={cn("px-3 py-1.5 rounded-md border text-sm",
                    brouillon.format === f
                      ? "border-primary bg-primary/5 text-primary font-medium"
                      : "border-border hover:bg-muted")}>
                  {LIBELLES_FORMAT[f]}
                </button>
              ))}
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              Proposé à l'impression ; on peut toujours en choisir un autre dans l'aperçu.
            </p>
          </div>

          <div className="divide-y divide-border">
            <ChoixTriple etiquette="Colonne remise" valeur={brouillon.colonne_remise}
              aide="Auto : seulement si une ligne porte une remise."
              onChange={c => changer("colonne_remise", c)} />
            <ChoixTriple etiquette="Colonne TVA" valeur={brouillon.colonne_tva}
              aide="Auto : seulement si une ligne porte de la TVA."
              onChange={c => changer("colonne_tva", c)} />
            <ChoixTriple etiquette="Récapitulatif TVA" valeur={brouillon.recap_tva}
              aide="Le détail HT / TVA / TTC par taux, sous les totaux."
              onChange={c => changer("recap_tva", c)} />
          </div>

          <div>
            <Case coche={brouillon.montant_lettres} onChange={b => changer("montant_lettres", b)}
              etiquette="Montant en lettres" aide="« Arrêté la présente facture à la somme de… »" />
            <Case coche={brouillon.reference_article} onChange={b => changer("reference_article", b)}
              etiquette="Référence de l'article" aide="Le code de l'article devant sa désignation." />
          </div>

          <div>
            <Label htmlFor="mention-doc" className="mb-1.5 block">Mention de bas de page</Label>
            <textarea id="mention-doc" rows={2}
              value={brouillon.mention ?? ""}
              onChange={e => changer("mention", e.target.value || null)}
              placeholder={piedFacture || "Merci de votre confiance"}
              className="w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm
                         focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" />
            <p className="text-xs text-muted-foreground mt-1">
              Vide : le « pied de facture » de Paramètres → Société.
              {pied && " Ignorée tant qu'une image de pied de page est posée."}
            </p>
          </div>

          {!estTicket && (
            <div>
              <Label className="mb-1.5 block">
                Signatures <span className="text-muted-foreground font-normal">({brouillon.signatures.length}/{SIGNATURES_MAX})</span>
              </Label>
              {brouillon.signatures.length === 0 && (
                <p className="text-xs text-muted-foreground mb-2">Aucune signature sur ce document.</p>
              )}
              <ul className="space-y-2">
                {brouillon.signatures.map((s, i) => (
                  <li key={i} className="flex items-center gap-2">
                    <Input aria-label={`Signature ${i + 1}`} value={s.libelle} maxLength={40}
                      placeholder="Ex. Le magasinier"
                      onChange={e => changer("signatures",
                        brouillon.signatures.map((x, j) => (j === i ? { ...x, libelle: e.target.value } : x)))} />
                    <div className="h-9 w-14 shrink-0 rounded border border-dashed border-border
                                    flex items-center justify-center overflow-hidden bg-muted/30"
                      title={s.image ? "Cachet / signature posé" : "Pas d'image"}>
                      {s.image
                        ? <img src={s.image} alt={`Cachet de ${s.libelle}`} className="max-h-full max-w-full object-contain" />
                        : <Stamp className="h-4 w-4 text-muted-foreground" />}
                    </div>
                    <div className="w-[84px] shrink-0 flex justify-center">
                    {i < enregistrees ? (
                      s.image ? (
                        <Button size="sm" variant="ghost" onClick={() => poserCachet(i, true)}
                          disabled={occupe !== null} title="Retirer l'image">
                          <X className="h-4 w-4" />
                        </Button>
                      ) : (
                        <Button size="sm" variant="outline" onClick={() => poserCachet(i)}
                          disabled={occupe !== null} title="Poser un cachet ou une signature scannée">
                          {occupe === `cachet-${i}` ? <Loader2 className="h-4 w-4 animate-spin" /> : "Image…"}
                        </Button>
                      )
                    ) : (
                      <span className="text-[11px] leading-tight text-muted-foreground text-center">
                        enregistrer d'abord
                      </span>
                    )}
                    </div>
                    <Button size="sm" variant="ghost" aria-label={`Retirer la signature ${i + 1}`}
                      onClick={() => changer("signatures", brouillon.signatures.filter((_, j) => j !== i))}>
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </li>
                ))}
              </ul>
              {brouillon.signatures.length < SIGNATURES_MAX && (
                <Button size="sm" variant="outline" className="mt-2"
                  onClick={() => changer("signatures", [...brouillon.signatures, { libelle: "", image: null }])}>
                  <Plus className="h-4 w-4 mr-1" /> Ajouter une signature
                </Button>
              )}
              <p className="text-xs text-muted-foreground mt-1">
                L'image (cachet, signature scannée) s'imprime au-dessus du trait. PNG ou JPEG, 512 ko au plus.
              </p>
            </div>
          )}

          <div className="flex flex-wrap items-center gap-2 pt-2 border-t border-border">
            <Button onClick={enregistrer} disabled={!modifie || occupe !== null}>
              {occupe === "genre" ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Save className="h-4 w-4 mr-2" />Enregistrer</>}
            </Button>
            <Button variant="outline" onClick={() => { setBrouillon(reglages.genres[genre]); setAvisGenre(null); }}
              disabled={!modifie || occupe !== null}>
              Annuler les changements
            </Button>
            <Button variant="ghost" onClick={retablir} disabled={occupe !== null} className="ml-auto">
              <RotateCcw className="h-4 w-4 mr-2" /> Réglages d'usine
            </Button>
          </div>
          <AvisLigne avis={avisGenre} />
        </div>
        <ApercuExemple html={apercu} format={brouillon.format} />
        </div>
      </section>
    </div>
  );
}
