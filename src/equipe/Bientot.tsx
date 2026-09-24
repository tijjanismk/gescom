// equipe/Bientot.tsx — un écran annoncé, pas encore construit : le
// dire plutôt qu'afficher un écran vide.
import type { ModuleEquipe } from "./modules";

export function Bientot({ module }: { module: ModuleEquipe }) {
  return (
    <div className="max-w-xl space-y-2" data-testid={`bientot-${module.cle}`}>
      <h1 className="text-xl font-semibold">{module.libelle}</h1>
      <p className="text-sm text-muted-foreground">{module.description}</p>
      <p className="text-sm">Arrive avec l'étape {module.aVenir} du plan.</p>
    </div>
  );
}
