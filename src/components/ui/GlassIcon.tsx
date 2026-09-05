import { useEffect } from "react";

// =====================================================================
//  GlassIcon — tuile de verre iOS
//
//  Le verre ne se lit que sur un fond non uni : poser <GlassHalos />
//  derriere le conteneur si l'ecran est sur blanc plat.
// =====================================================================

export type GlassVariante = "tinted" | "neutral" | "ink" | "clear";
export type GlassTaille = "sm" | "md" | "lg";

const COTE: Record<GlassTaille, number> = { sm: 36, md: 52, lg: 72 };
const ICONE: Record<GlassTaille, number> = { sm: 18, md: 26, lg: 32 };

// Lucide dessine sur un viewBox 24 et met la taille a l'echelle : un
// strokeWidth de 1.7 rendrait 2.27px sur une tuile de 72. On compense
// pour garder un trait de 1.7px reel aux trois tailles.
function traitReel(taillePx: number) {
  return Number(((1.7 * 24) / taillePx).toFixed(3));
}

const CSS = `
.gi{
  --gi-specular: inset 0 1px 0 rgba(255,255,255,.95);
  --gi-ombre: 0 7px 16px rgba(24,22,20,.15);
  --gi-border: rgba(255,255,255,.8);
  position:relative;
  display:inline-flex;
  align-items:center;
  justify-content:center;
  overflow:hidden;
  flex:0 0 auto;
  padding:0;
  margin:0;
  width:var(--gi-cote);
  height:var(--gi-cote);
  border-radius:var(--gi-rayon);
  border:1px solid var(--gi-border);
  background-color:transparent;
  background-image:var(--gi-fond);
  box-shadow:var(--gi-specular), var(--gi-ombre);
  color:var(--gi-encre);
  -webkit-backdrop-filter:var(--gi-flou);
  backdrop-filter:var(--gi-flou);
  transition:transform 160ms ease, box-shadow 160ms ease,
             background-image 160ms ease, opacity 160ms ease;
}
button.gi{ -webkit-appearance:none; appearance:none; cursor:pointer; }

/* Teinte : par-dessus le verre, sous l'icone. */
.gi__teinte{ position:absolute; inset:0; pointer-events:none;
             background-image:var(--gi-teinte, none);
             transition:background-image 160ms ease; }
.gi__icone{ position:relative; z-index:1; display:block; }

/* --- Tailles : rayon = 0,31 x cote --- */
.gi--sm{ --gi-cote:36px; --gi-rayon:11px; }
.gi--md{ --gi-cote:52px; --gi-rayon:16px; }
.gi--lg{ --gi-cote:72px; --gi-rayon:22px; }

/* --- Variantes --- */
.gi--tinted{
  --gi-flou: blur(15px) saturate(180%);
  --gi-fond: linear-gradient(165deg, rgba(255,255,255,.62), rgba(255,255,255,.18));
  --gi-fond-hover: linear-gradient(165deg, rgba(255,255,255,.71), rgba(255,255,255,.21));
  --gi-teinte: linear-gradient(160deg, rgba(236,48,19,.28), rgba(236,48,19,.02) 65%);
  --gi-teinte-hover: linear-gradient(160deg, rgba(236,48,19,.32), rgba(236,48,19,.023) 65%);
  --gi-encre: #b3230f;
}
.gi--neutral{
  --gi-flou: blur(18px) saturate(120%);
  --gi-fond: linear-gradient(165deg, rgba(255,255,255,.72), rgba(255,255,255,.30));
  --gi-fond-hover: linear-gradient(165deg, rgba(255,255,255,.83), rgba(255,255,255,.35));
  --gi-encre: #201e1d;
}
.gi--ink{
  --gi-flou: blur(15px) saturate(180%);
  --gi-fond: linear-gradient(165deg, rgba(32,30,29,.72), rgba(32,30,29,.42));
  --gi-fond-hover: linear-gradient(165deg, rgba(32,30,29,.83), rgba(32,30,29,.48));
  --gi-border: rgba(255,255,255,.28);
  --gi-encre: #ffffff;
}
.gi--clear{
  --gi-flou: blur(10px) saturate(200%);
  --gi-fond: linear-gradient(165deg, rgba(255,255,255,.30), rgba(255,255,255,.05));
  --gi-fond-hover: linear-gradient(165deg, rgba(255,255,255,.35), rgba(255,255,255,.06));
  --gi-encre: #b3230f;
}

/* --- Etats --- */
.gi--interactive:hover{
  transform:translateY(-2px);
  background-image:var(--gi-fond-hover);
  box-shadow:var(--gi-specular), 0 12px 26px rgba(24,22,20,.20);
}
.gi--interactive:hover .gi__teinte{
  background-image:var(--gi-teinte-hover, var(--gi-teinte, none));
}
.gi--interactive:active{
  transform:scale(.97);
  box-shadow:var(--gi-specular), inset 0 2px 6px rgba(24,22,20,.22);
}
.gi:focus-visible{ outline:2px solid #ec3013; outline-offset:2px; }

/* Apres les regles :hover — meme specificite, l'ordre tranche. */
.gi--off, .gi--off:hover, .gi--off:active{
  opacity:.45;
  filter:saturate(60%);
  box-shadow:none;
  transform:none;
  cursor:not-allowed;
  background-image:var(--gi-fond);
}

@media (prefers-reduced-motion: reduce){
  .gi{ transition:none; }
  .gi--interactive:hover, .gi--interactive:active{ transform:none; }
}

/* Repli si le navigateur ne sait pas flouter le fond : on monte les
   opacites, sinon la tuile devient un rectangle fantome. */
@supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))){
  .gi--tinted{ --gi-fond: linear-gradient(165deg, rgba(255,255,255,.88), rgba(255,255,255,.55)); }
  .gi--neutral{ --gi-fond: linear-gradient(165deg, rgba(255,255,255,.95), rgba(255,255,255,.70)); }
  .gi--clear{ --gi-fond: linear-gradient(165deg, rgba(255,255,255,.60), rgba(255,255,255,.30)); }
  .gi--ink{ --gi-fond: linear-gradient(165deg, rgba(32,30,29,.94), rgba(32,30,29,.80)); }
}

/* --- Halos de fond --- */
.gi-halos{ position:absolute; inset:-10%; pointer-events:none; z-index:0;
           filter:blur(45px); overflow:hidden; }
.gi-halos i{ position:absolute; display:block; border-radius:9999px; }
`;

let cssPose = false;
function useStylesGlass() {
  useEffect(() => {
    if (cssPose || document.getElementById("gi-styles")) { cssPose = true; return; }
    const el = document.createElement("style");
    el.id = "gi-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
    cssPose = true;
  }, []);
}

// =====================================================================

export interface GlassIconProps {
  icone: React.ElementType;
  variante?: GlassVariante;
  taille?: GlassTaille;
  /** Bascule sur `ink` — etat actif / selectionne. */
  actif?: boolean;
  inactif?: boolean;
  onClick?: () => void;
  /** Obligatoire si onClick : la tuile devient un bouton. */
  label?: string;
  title?: string;
  className?: string;
}

export function GlassIcon({
  icone: Icone,
  variante = "neutral",
  taille = "md",
  actif = false,
  inactif = false,
  onClick,
  label,
  title,
  className = "",
}: GlassIconProps) {
  useStylesGlass();

  const v = actif ? "ink" : variante;
  const px = ICONE[taille];
  const cliquable = Boolean(onClick) && !inactif;

  const classes = [
    "gi", `gi--${v}`, `gi--${taille}`,
    cliquable ? "gi--interactive" : "",
    inactif ? "gi--off" : "",
    className,
  ].filter(Boolean).join(" ");

  // fill / stroke en ATTRIBUTS : une classe CSS ne survit pas a
  // l'export du SVG, l'icone retombe alors sur fill noir plein.
  const svg = (
    <Icone
      className="gi__icone"
      width={px}
      height={px}
      fill="none"
      stroke="currentColor"
      strokeWidth={traitReel(px)}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    />
  );

  const teinte = <span className="gi__teinte" aria-hidden="true" />;

  if (onClick) {
    return (
      <button
        type="button"
        className={classes}
        onClick={inactif ? undefined : onClick}
        disabled={inactif}
        aria-label={label}
        title={title ?? label}
        style={{ width: COTE[taille], height: COTE[taille] }}
      >
        {teinte}
        {svg}
      </button>
    );
  }

  return (
    <span
      className={classes}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      title={title}
      style={{ width: COTE[taille], height: COTE[taille] }}
    >
      {teinte}
      {svg}
    </span>
  );
}

// =====================================================================
//  GlassHalos — le fond dont le verre a besoin
//
//  A poser en premier enfant d'un conteneur `position:relative`, avec
//  le contenu au-dessus (z-index >= 1).
// =====================================================================

export function GlassHalos({ className = "" }: { className?: string }) {
  useStylesGlass();
  return (
    <div className={`gi-halos ${className}`} aria-hidden="true">
      <i style={{ top: "-6%", left: "4%", width: 380, height: 380,
                  background: "rgba(236,48,19,.22)" }} />
      <i style={{ top: "22%", right: "6%", width: 320, height: 320,
                  background: "rgba(120,140,190,.18)" }} />
      <i style={{ bottom: "-8%", left: "38%", width: 420, height: 300,
                  background: "rgba(255,190,120,.16)" }} />
    </div>
  );
}
