// equipe/Accueil.tsx — ce que la personne connectée peut faire ici.
//
// Les cartes prennent toute la largeur, autant par ligne que la place le
// permet (comme les tuiles de la caisse, `KpiVerre.GRILLE`). Elles
// tenaient dans 768 px sur deux colonnes : trois modules tombaient en
// deux en haut, un seul en bas (le propriétaire, 29/10/2026).
import type { ModuleEquipe } from "./modules";
import { dossierCourant } from "@/lib/pont";

export function Accueil({ nom, modules, onOuvrir }: {
  nom: string;
  modules: ModuleEquipe[];
  onOuvrir: (cle: string) => void;
}) {
  const dossier = dossierCourant();
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">Bonjour {nom}</h1>
        {dossier && <p className="text-sm text-muted-foreground">Dossier : {dossier.societe}</p>}
      </div>
      {modules.length === 0 ? (
        <p className="text-sm text-muted-foreground" data-testid="rien-a-faire">
          Votre rôle ne donne accès à rien dans Équipe pour l'instant. Demander au patron.
        </p>
      ) : (
        <div className="grid gap-4" style={{ gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))" }}>
          {modules.map(m => {
            const Icone = m.icone;
            return (
              <button key={m.cle} onClick={() => onOuvrir(m.cle)} data-testid={`module-${m.cle}`}
                className="text-left border border-border rounded-xl p-4 hover:bg-muted/50 transition-colors space-y-2">
                <div className="flex items-center gap-2 font-medium">
                  <Icone className="h-4 w-4" /> {m.libelle}
                </div>
                <p className="text-sm text-muted-foreground">{m.description}</p>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
