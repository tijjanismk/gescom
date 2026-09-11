// components/EditeurPiedPage.tsx — le pied de page se dessine.
//
// Tous les autres blocs s'enchaînent de haut en bas : c'est ce qu'il
// faut pour un corps de facture, dont la hauteur dépend du nombre de
// lignes. Un pied de page, lui, a une hauteur FIXE et connue — on y
// pose les choses côte à côte : le cachet à gauche, les mentions
// légales au centre, le numéro de pièce à droite.
//
// D'où cette surface : on attrape un élément, on le déplace, on le
// redimensionne. Les coordonnées sont en millimètres et non en pixels,
// parce que la cible est du papier — et parce qu'un commerçant qui
// mesure son cachet le mesure en centimètres.

import { useRef, useState } from "react";
import type { Bloc, ElementPied } from "@/lib/modeles/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Type, Image as ImageIcon, Minus, Trash2, Copy,
} from "lucide-react";

/** Largeur utile d'une page, marges déduites. */
const LARGEUR_UTILE: Record<string, number> = {
  a4: 186,
  a5: 124,
  thermique_80: 74,
  thermique_58: 52,
};

let compteur = 0;
function nouvelId(): string {
  compteur += 1;
  return `e-${Date.now().toString(36)}-${compteur}`;
}

function elementNeuf(genre: ElementPied["genre"]): ElementPied {
  const base = {
    id: nouvelId(),
    genre,
    xMm: 4,
    yMm: 4,
    contenu: "",
    image: "pied" as const,
    alignement: "gauche" as const,
    taillePt: 8,
    gras: false,
    italique: false,
  };
  switch (genre) {
    case "image":
      return { ...base, largeurMm: 40, hauteurMm: 15 };
    case "trait":
      return { ...base, largeurMm: 60, hauteurMm: 0.5 };
    default:
      return {
        ...base,
        largeurMm: 60,
        hauteurMm: 8,
        contenu: "{{societe.nom}}",
      };
  }
}

interface Props {
  bloc: Extract<Bloc, { type: "pied_page" }>;
  format: string;
  /** Les images de la société, pour montrer ce qui sera imprimé. */
  images: { logo?: string | null; entete?: string | null; pied?: string | null };
  onChange: (patch: Partial<Extract<Bloc, { type: "pied_page" }>>) => void;
}

export function EditeurPiedPage({ bloc, format, images, onChange }: Props) {
  const surface = useRef<HTMLDivElement>(null);
  const [actif, setActif] = useState<string | null>(null);
  // `null` quand on ne déplace rien. On garde l'écart entre le point
  // saisi et le coin de l'élément : sans lui, l'élément saute sous le
  // curseur au premier mouvement.
  const glisse = useRef<
    { id: string; ecartX: number; ecartY: number; redim: boolean } | null
  >(null);

  const largeurMm = LARGEUR_UTILE[format] ?? 186;

  /** Pixels de l'écran → millimètres du papier. */
  function enMm(e: { clientX: number; clientY: number }) {
    const r = surface.current?.getBoundingClientRect();
    if (!r) return { x: 0, y: 0 };
    return {
      x: ((e.clientX - r.left) / r.width) * largeurMm,
      y: ((e.clientY - r.top) / r.height) * bloc.hauteurMm,
    };
  }

  function modifier(id: string, patch: Partial<ElementPied>) {
    onChange({
      elements: bloc.elements.map((e) => (e.id === id ? { ...e, ...patch } : e)),
    });
  }

  function ajouter(genre: ElementPied["genre"]) {
    const e = elementNeuf(genre);
    onChange({ elements: [...bloc.elements, e] });
    setActif(e.id);
  }

  function supprimer(id: string) {
    onChange({ elements: bloc.elements.filter((e) => e.id !== id) });
    if (actif === id) setActif(null);
  }

  function dupliquer(id: string) {
    const src = bloc.elements.find((e) => e.id === id);
    if (!src) return;
    // Décalé de 3 mm : posée exactement dessus, la copie serait
    // invisible et on croirait que le bouton n'a rien fait.
    const copie = { ...src, id: nouvelId(), xMm: src.xMm + 3, yMm: src.yMm + 3 };
    onChange({ elements: [...bloc.elements, copie] });
    setActif(copie.id);
  }

  function onPointerDown(
    ev: React.PointerEvent,
    el: ElementPied,
    redim: boolean,
  ) {
    ev.preventDefault();
    ev.stopPropagation();
    setActif(el.id);
    const p = enMm(ev);
    glisse.current = {
      id: el.id,
      ecartX: p.x - el.xMm,
      ecartY: p.y - el.yMm,
      redim,
    };
    (ev.target as HTMLElement).setPointerCapture(ev.pointerId);
  }

  function onPointerMove(ev: React.PointerEvent) {
    const g = glisse.current;
    if (!g) return;
    const el = bloc.elements.find((x) => x.id === g.id);
    if (!el) return;
    const p = enMm(ev);

    if (g.redim) {
      // 5 mm au minimum : en dessous, l'élément devient impossible à
      // rattraper à la souris.
      modifier(el.id, {
        largeurMm: Math.max(5, Math.round((p.x - el.xMm) * 10) / 10),
        hauteurMm: Math.max(
          el.genre === "trait" ? 0.5 : 4,
          Math.round((p.y - el.yMm) * 10) / 10,
        ),
      });
      return;
    }

    // Bridé à la bande : un élément posé dehors ne s'imprimerait pas,
    // et le commerçant chercherait pourquoi.
    modifier(el.id, {
      xMm: Math.max(
        0,
        Math.min(largeurMm - el.largeurMm, Math.round((p.x - g.ecartX) * 10) / 10),
      ),
      yMm: Math.max(
        0,
        Math.min(bloc.hauteurMm - el.hauteurMm, Math.round((p.y - g.ecartY) * 10) / 10),
      ),
    });
  }

  function onPointerUp() {
    glisse.current = null;
  }

  const selection = bloc.elements.find((e) => e.id === actif) ?? null;

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 flex-wrap">
        <Button size="sm" variant="outline" onClick={() => ajouter("texte")}>
          <Type className="h-3.5 w-3.5 mr-1" /> Texte
        </Button>
        <Button size="sm" variant="outline" onClick={() => ajouter("image")}>
          <ImageIcon className="h-3.5 w-3.5 mr-1" /> Image
        </Button>
        <Button size="sm" variant="outline" onClick={() => ajouter("trait")}>
          <Minus className="h-3.5 w-3.5 mr-1" /> Trait
        </Button>
        <div className="flex items-center gap-1 ml-auto">
          <Label className="text-xs">Hauteur</Label>
          <Input
            type="number" min={8} max={80} step={1}
            className="h-8 w-20"
            value={bloc.hauteurMm}
            onChange={(e) =>
              onChange({ hauteurMm: Math.max(8, Number(e.target.value) || 8) })
            }
          />
          <span className="text-xs text-muted-foreground">mm</span>
        </div>
      </div>

      {/* La surface. Son rapport largeur/hauteur est celui du papier :
          ce qu'on voit ici est ce qui sortira de l'imprimante. */}
      <div
        ref={surface}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={onPointerUp}
        onPointerDown={() => setActif(null)}
        className="relative w-full bg-white border border-dashed
                   border-muted-foreground/40 rounded overflow-hidden
                   select-none"
        style={{ aspectRatio: `${largeurMm} / ${bloc.hauteurMm}` }}
      >
        {bloc.elements.map((el) => {
          const choisi = el.id === actif;
          const style: React.CSSProperties = {
            position: "absolute",
            left: `${(el.xMm / largeurMm) * 100}%`,
            top: `${(el.yMm / bloc.hauteurMm) * 100}%`,
            width: `${(el.largeurMm / largeurMm) * 100}%`,
            height: `${(el.hauteurMm / bloc.hauteurMm) * 100}%`,
            fontSize: `${el.taillePt}pt`,
            textAlign: el.alignement === "centre" ? "center"
              : el.alignement === "droite" ? "right" : "left",
            fontWeight: el.gras ? 700 : 400,
            fontStyle: el.italique ? "italic" : "normal",
          };
          const src =
            el.image === "pied" ? images.pied
              : el.image === "logo" ? images.logo : images.entete;

          return (
            <div
              key={el.id}
              style={style}
              onPointerDown={(ev) => onPointerDown(ev, el, false)}
              className={`cursor-move overflow-hidden text-black leading-tight
                ${choisi ? "outline outline-2 outline-primary" : "outline outline-1 outline-muted-foreground/20"}`}
            >
              {el.genre === "trait" && (
                <div className="w-full border-t border-black" />
              )}
              {el.genre === "image" && (
                src
                  ? <img src={src} alt="" className="w-full h-full object-contain" />
                  : <div className="w-full h-full flex items-center justify-center
                                    text-[10px] text-muted-foreground bg-muted/40">
                      {el.image} — aucune image
                    </div>
              )}
              {el.genre === "texte" && (
                <div className="whitespace-pre-line">{el.contenu}</div>
              )}

              {choisi && (
                <div
                  onPointerDown={(ev) => onPointerDown(ev, el, true)}
                  className="absolute -right-0.5 -bottom-0.5 h-3 w-3
                             bg-primary cursor-se-resize rounded-sm"
                  title="Redimensionner"
                />
              )}
            </div>
          );
        })}

        {bloc.elements.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center
                          text-xs text-muted-foreground">
            Ajouter un texte, une image ou un trait, puis le déplacer ici.
          </div>
        )}
      </div>

      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" checked={bloc.trait}
               onChange={(e) => onChange({ trait: e.target.checked })} />
        Filet de séparation au-dessus du pied
      </label>

      {/* Réglages de l'élément choisi */}
      {selection && (
        <div className="border border-border rounded-lg p-3 space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold uppercase tracking-wide
                             text-muted-foreground">
              {selection.genre}
            </span>
            <div className="flex gap-1">
              <Button size="sm" variant="ghost"
                onClick={() => dupliquer(selection.id)} title="Dupliquer">
                <Copy className="h-3.5 w-3.5" />
              </Button>
              <Button size="sm" variant="ghost"
                onClick={() => supprimer(selection.id)} title="Supprimer">
                <Trash2 className="h-3.5 w-3.5 text-red-500" />
              </Button>
            </div>
          </div>

          {selection.genre === "texte" && (
            <>
              <div>
                <Label className="text-xs">
                  Texte — les {"{{champs}}"} sont remplacés à l'impression
                </Label>
                <textarea
                  className="w-full mt-1 text-sm rounded-md border border-input
                             bg-background px-2 py-1.5 min-h-[64px]"
                  value={selection.contenu}
                  onChange={(e) => modifier(selection.id, { contenu: e.target.value })}
                />
              </div>
              <div className="flex items-center gap-2 flex-wrap">
                <Label className="text-xs">Taille</Label>
                <Input type="number" min={5} max={20} step={0.5}
                  className="h-8 w-16" value={selection.taillePt}
                  onChange={(e) =>
                    modifier(selection.id, { taillePt: Number(e.target.value) || 8 })} />
                <label className="flex items-center gap-1 text-xs">
                  <input type="checkbox" checked={selection.gras}
                    onChange={(e) => modifier(selection.id, { gras: e.target.checked })} />
                  Gras
                </label>
                <label className="flex items-center gap-1 text-xs">
                  <input type="checkbox" checked={selection.italique}
                    onChange={(e) =>
                      modifier(selection.id, { italique: e.target.checked })} />
                  Italique
                </label>
              </div>
            </>
          )}

          {selection.genre === "image" && (
            <div>
              <Label className="text-xs">Quelle image</Label>
              <select
                className="w-full mt-1 h-8 text-sm rounded-md border
                           border-input bg-background px-2"
                value={selection.image}
                onChange={(e) =>
                  modifier(selection.id, {
                    image: e.target.value as ElementPied["image"],
                  })}
              >
                <option value="pied">Pied de page</option>
                <option value="logo">Logo</option>
                <option value="entete">En-tête</option>
              </select>
              <p className="text-[11px] text-muted-foreground mt-1">
                Les images se chargent dans Paramètres → Société. Absente,
                la zone reste vide à l'impression — elle ne laisse pas de
                cadre.
              </p>
            </div>
          )}

          {selection.genre !== "image" && (
            <div>
              <Label className="text-xs">Alignement</Label>
              <div className="flex gap-1 mt-1">
                {(["gauche", "centre", "droite"] as const).map((a) => (
                  <Button key={a} size="sm"
                    variant={selection.alignement === a ? "default" : "outline"}
                    onClick={() => modifier(selection.id, { alignement: a })}>
                    {a}
                  </Button>
                ))}
              </div>
            </div>
          )}

          {/* Les millimètres au clavier : la souris place vite, le
              chiffre place juste. Aligner deux éléments au pixel près
              est pénible ; écrire deux fois la même valeur ne l'est
              pas. */}
          <div className="grid grid-cols-4 gap-2">
            {([
              ["xMm", "X"], ["yMm", "Y"],
              ["largeurMm", "Larg."], ["hauteurMm", "Haut."],
            ] as const).map(([cle, libelle]) => (
              <div key={cle}>
                <Label className="text-[11px]">{libelle} (mm)</Label>
                <Input type="number" step={0.5} className="h-8"
                  value={selection[cle]}
                  onChange={(e) =>
                    modifier(selection.id, { [cle]: Number(e.target.value) || 0 })} />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
