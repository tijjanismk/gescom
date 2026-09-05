import { GlassIcon, type GlassVariante } from "@/components/ui/GlassIcon";

// =====================================================================
//  Cartes KPI — vocabulaire commun aux ecrans qui affichent du verre.
//
//  Dashboard, Caisse, FicheClient, FicheFournisseur tiraient chacun sa
//  propre tuile ; trois variantes de la meme carte finissaient toujours
//  par diverger. Un seul endroit ici.
// =====================================================================

// Coins carres : seules les tuiles d'icones s'arrondissent.
export const CARTE = "relative z-[1] bg-card/70 border border-border rounded-none";
export const TXT2 = "#8a8482";

// auto-fit / 190px : 4 colonnes en large, 2x2 en etroit. Un
// `grid-cols-4` fixe tombait en 3+1 aux largeurs intermediaires.
export const GRILLE: React.CSSProperties = {
  display: "grid",
  gap: "0.75rem",
  gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))",
};

// =====================================================================
//  KpiCard — tuile 52, chiffre en tete d'affiche
// =====================================================================

export function KpiCard({
  titre, valeur, sous, icone, variante = "neutral", inactif,
  tendance, tendanceIcones, onClick,
}: {
  titre: string; valeur: string; sous?: string;
  icone: React.ElementType; variante?: GlassVariante;
  inactif?: boolean; tendance?: number;
  /** [hausse, baisse] — evite d'importer lucide ici. */
  tendanceIcones?: [React.ElementType, React.ElementType];
  onClick?: () => void;
}) {
  const [Haut, Bas] = tendanceIcones ?? [];
  return (
    <div
      onClick={onClick}
      className={`${CARTE} p-4 space-y-3
                  ${onClick ? "cursor-pointer transition-all hover:border-primary/30" : ""}`}>
      <div className="flex items-start justify-between">
        <GlassIcon icone={icone} variante={variante} taille="md" inactif={inactif} />
        {tendance !== undefined && Haut && Bas && (
          <div className={`flex items-center gap-1 text-xs font-medium ${
            tendance >= 0 ? "text-green-600" : "text-red-500"
          }`}>
            {tendance >= 0
              ? <Haut className="h-3.5 w-3.5" />
              : <Bas className="h-3.5 w-3.5" />}
            {Math.abs(tendance)}%
          </div>
        )}
      </div>
      <div>
        <p className="text-2xl tracking-tight" style={{ fontWeight: 700 }}>{valeur}</p>
        <p className="text-xs mt-0.5" style={{ color: TXT2 }}>{titre}</p>
        {sous && <p className="text-xs" style={{ color: TXT2 }}>{sous}</p>}
      </div>
    </div>
  );
}

// =====================================================================
//  KpiPetit — tuile 36 en tete de bloc
// =====================================================================

export function KpiPetit({
  titre, valeur, sous, icone, variante = "clear", inactif, alerte,
}: {
  titre: string; valeur: string; sous?: string;
  icone: React.ElementType; variante?: GlassVariante;
  inactif?: boolean; alerte?: boolean;
}) {
  return (
    <div className={`${CARTE} p-4`}>
      <div className="flex items-center gap-2.5 mb-2">
        <GlassIcon icone={icone} variante={variante} taille="sm" inactif={inactif} />
        <p className="text-xs font-medium" style={{ color: TXT2 }}>{titre}</p>
      </div>
      <p className={`text-2xl ${alerte ? "text-red-500" : ""}`} style={{ fontWeight: 700 }}>
        {valeur}
      </p>
      {sous && <p className="text-xs" style={{ color: TXT2 }}>{sous}</p>}
    </div>
  );
}

// =====================================================================
//  KpiLigne — tuile 36 a gauche, libelle et valeur a droite.
//  Format des fiches client / fournisseur : beaucoup d'indicateurs,
//  peu de place verticale.
// =====================================================================

export function KpiLigne({
  label, valeur, icone, variante = "clear", inactif,
}: {
  label: string; valeur: string;
  icone: React.ElementType; variante?: GlassVariante; inactif?: boolean;
}) {
  return (
    <div className={`${CARTE} p-3 flex items-center gap-3`}>
      <GlassIcon icone={icone} variante={variante} taille="sm" inactif={inactif} />
      <div className="min-w-0">
        <p className="text-xs" style={{ color: TXT2 }}>{label}</p>
        <p className="text-sm truncate" style={{ fontWeight: 700 }}>{valeur}</p>
      </div>
    </div>
  );
}
