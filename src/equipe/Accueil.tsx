// equipe/Accueil.tsx — ce que la personne connectée peut faire ici.
import type { ModuleEquipe } from "./modules";
import { dossierCourant } from "@/lib/pont";

export function Accueil({ nom, modules, onOuvrir }: {
  nom: string;
  modules: ModuleEquipe[];
  onOuvrir: (cle: string) => void;
}) {
  const dossier = dossierCourant();
  return (
    <div className="max-w-3xl space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">Bonjour {nom}</h1>
        {dossier && <p className="text-sm text-muted-foreground">Dossier : {dossier.societe}</p>}
      </div>
      {modules.length === 0 ? (
        <p className="text-sm text-muted-foreground" data-testid="rien-a-faire">
          Votre rôle ne donne accès à rien dans Équipe pour l'instant. Demander au patron.
        </p>
      ) : (
        <div className="grid sm:grid-cols-2 gap-4">
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
