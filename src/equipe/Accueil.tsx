// equipe/Accueil.tsx — ce que la personne connectée peut faire ici.
//
// Les cartes prennent toute la largeur, autant par ligne que la place le
// permet (comme les tuiles de la caisse, `KpiVerre.GRILLE`). Elles
// tenaient dans 768 px sur deux colonnes : trois modules tombaient en
// deux en haut, un seul en bas (le propriétaire, 29/10/2026).
//
// EQ-3 (D49) : au-dessus, « Aujourd'hui » — ce qui attend la personne
// (ses rappels dus, les fiches en brouillon, les fiches à solder, les
// avances en cours). Le serveur ne rend que les blocs qu'elle a le droit
// de lire, et un montant qu'elle ne lit pas vaut null : l'écran affiche
// ce qu'il reçoit, il ne décide rien.
import { useEffect, useState } from "react";
import type { ModuleEquipe } from "./modules";
import { appeler as invoke, dossierCourant } from "@/lib/pont";
import { KpiPetit, GRILLE } from "@/components/ui/KpiVerre";

interface AccueilJour {
  aujourd_hui: string;
  rappels?: { nombre: number; en_retard: number };
  brouillons?: { nombre: number; net: number | null; plus_ancien: string | null };
  a_solder?: { nombre: number; reste: number | null; plus_ancien: string | null };
  avances?: { nombre: number; personnes: number; reste: number | null };
  // D47 : les prêts, à part des avances (même forme), pour qui lit la paie.
  prets?: { nombre: number; personnes: number; reste: number | null };
}

const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const MOIS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
const mois = (d: string) => `${MOIS[Number(d.slice(5, 7)) - 1] ?? ""} ${d.slice(0, 4)}`;
const pluriel = (n: number, un: string, plusieurs: string) => `${n} ${n > 1 ? plusieurs : un}`;

/** Une tuile qui mène à l'écran concerné. */
function Tuile({ testid, onClick, ...k }: {
  testid: string; onClick: () => void; titre: string; valeur: string; sous?: string; alerte?: boolean;
}) {
  return (
    <button type="button" onClick={onClick} data-testid={testid}
      className="text-left transition-colors hover:[&>div]:border-primary/40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary">
      <KpiPetit {...k} />
    </button>
  );
}

function Aujourdhui({ onOuvrir }: { onOuvrir: (cle: string, vue?: string) => void }) {
  const [jour, setJour] = useState<AccueilJour | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    let annule = false;
    invoke<AccueilJour>("lire_accueil_equipe")
      .then(j => { if (!annule) setJour(j); })
      .catch(e => { if (!annule) setErreur(String(e)); });
    return () => { annule = true; };
  }, []);

  if (erreur) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="aujourdhui-erreur">
        Impossible de lire ce qui vous attend aujourd'hui : {erreur}
      </p>
    );
  }
  if (!jour) return null;
  const { rappels, brouillons, a_solder, avances, prets } = jour;
  // Aucun bloc ouvert à ce compte : rien à montrer, pas une grille vide.
  if (!rappels && !brouillons && !a_solder && !avances && !prets) return null;

  return (
    <section className="space-y-2" aria-label="Aujourd'hui" data-testid="aujourdhui">
      <h2 className="text-sm font-semibold">Aujourd'hui</h2>
      <div style={GRILLE}>
        {rappels && (
          <Tuile testid="jour-rappels" onClick={() => onOuvrir("clients", "rappels")}
            titre="Mes rappels" valeur={String(rappels.nombre)} alerte={rappels.en_retard > 0}
            sous={rappels.nombre === 0 ? "rien en attente"
              : rappels.en_retard > 0 ? `dont ${rappels.en_retard} en retard` : "pour aujourd'hui"} />
        )}
        {brouillons && (
          <Tuile testid="jour-brouillons" onClick={() => onOuvrir("paie", "fiches")}
            titre="Fiches en brouillon" valeur={String(brouillons.nombre)}
            sous={brouillons.nombre === 0 ? "rien en attente"
              : [brouillons.net != null ? `${f(brouillons.net)} net` : null,
                 brouillons.plus_ancien ? `depuis ${mois(brouillons.plus_ancien)}` : null].filter(Boolean).join(" · ")} />
        )}
        {a_solder && (
          <Tuile testid="jour-a-solder" onClick={() => onOuvrir("paie", "fiches")}
            titre="Fiches à solder" valeur={String(a_solder.nombre)}
            sous={a_solder.nombre === 0 ? "rien en attente"
              : [a_solder.reste != null ? `${f(a_solder.reste)} à verser` : null,
                 a_solder.plus_ancien ? `depuis ${mois(a_solder.plus_ancien)}` : null].filter(Boolean).join(" · ")} />
        )}
        {avances && (
          <Tuile testid="jour-avances" onClick={() => onOuvrir("paie", "avances")}
            titre="Avances en cours" valeur={String(avances.nombre)}
            sous={avances.nombre === 0 ? "rien en attente"
              : [pluriel(avances.personnes, "personne", "personnes"),
                 avances.reste != null ? `${f(avances.reste)} à retenir` : null].filter(Boolean).join(" · ")} />
        )}
        {prets && (
          <Tuile testid="jour-prets" onClick={() => onOuvrir("paie", "prets")}
            titre="Prêts en cours" valeur={String(prets.nombre)}
            sous={prets.nombre === 0 ? "rien en attente"
              : [pluriel(prets.personnes, "personne", "personnes"),
                 prets.reste != null ? `${f(prets.reste)} à retenir` : null].filter(Boolean).join(" · ")} />
        )}
      </div>
    </section>
  );
}

export function Accueil({ nom, modules, onOuvrir }: {
  nom: string;
  modules: ModuleEquipe[];
  onOuvrir: (cle: string, vue?: string) => void;
}) {
  const dossier = dossierCourant();
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">Bonjour {nom}</h1>
        {dossier && <p className="text-sm text-muted-foreground">Dossier : {dossier.societe}</p>}
      </div>
      <Aujourdhui onOuvrir={onOuvrir} />
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
