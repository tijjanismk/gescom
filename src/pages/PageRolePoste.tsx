// pages/PageRolePoste.tsx — avant la connexion, le rôle de ce poste.
//
// D22/D24, révisés le 25/10/2026. La fenêtre ne parle qu'au serveur ;
// ce qui change d'un poste à l'autre, c'est OÙ il est :
//
//   - ce poste GARDE LA BASE (une seule caisse, ou le poste principal) :
//     le serveur est ici, la fenêtre le lance s'il ne répond pas —
//     rien à saisir ;
//   - ce poste est la CAISSE d'un autre poste : l'adresse se saisit et
//     s'essaie (PageServeur).
//
// Le rôle se demande UNE fois, au premier démarrage, et ne se devine
// jamais : deviner « garde la base » faisait d'une caisse fraîchement
// installée un poste principal, avec son serveur et sa base vide.

import { useEffect, useState, type ReactNode } from "react";
import { AlertTriangle, Database, Loader2, MonitorSmartphone, RotateCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { appeler as invoke, definirServeur, etatReseau, ADRESSE_LOCALE } from "@/lib/pont";

/** Ce poste garde la base : écrit dans poste.json, puis aligne le pont. */
export async function garderLaBase() {
  const e = etatReseau();
  await invoke("definir_config_reseau", {
    mode: "monoposte",
    serveur: null,
    posteNom: e.posteNom || null,
  });
  definirServeur("monoposte", ADRESSE_LOCALE);
}

function Cadre({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-muted/30">
      <div className="w-full max-w-lg space-y-5">{children}</div>
    </div>
  );
}

interface PropsRole {
  /** Le choix « garde la base » est écrit : lancer le serveur local. */
  onGarderBase: () => Promise<void>;
  /** Le poste est une caisse : passer au branchement. */
  onCaisse: () => void;
}

export function PageRolePoste({ onGarderBase, onCaisse }: PropsRole) {
  const [enCours, setEnCours] = useState(false);
  const [erreur, setErreur] = useState("");

  async function garder() {
    setEnCours(true);
    setErreur("");
    try {
      await onGarderBase();
    } catch (e) {
      setErreur(String(e));
      setEnCours(false);
    }
  }

  return (
    <Cadre>
      <div className="text-center space-y-1">
        <h1 className="text-xl font-semibold">Comment travaille ce poste ?</h1>
        <p className="text-sm text-muted-foreground">
          Une seule fois, à l'installation. On pourra changer plus tard
          dans Paramètres → Réseau.
        </p>
      </div>

      <button type="button" onClick={garder} disabled={enCours}
        className="w-full text-left rounded-lg border border-border bg-background p-5
                   hover:border-primary disabled:opacity-60 space-y-1">
        <span className="flex items-center gap-2 font-medium">
          <Database className="h-4 w-4 text-primary" /> Ce poste garde la base
        </span>
        <span className="block text-sm text-muted-foreground">
          La boutique n'a qu'une caisse, ou ce poste est le poste
          principal que les autres caisses rejoignent. Les données de la
          boutique sont sur cette machine.
        </span>
      </button>

      <button type="button" onClick={onCaisse} disabled={enCours}
        className="w-full text-left rounded-lg border border-border bg-background p-5
                   hover:border-primary disabled:opacity-60 space-y-1">
        <span className="flex items-center gap-2 font-medium">
          <MonitorSmartphone className="h-4 w-4 text-primary" /> Ce poste est une caisse d'un autre poste
        </span>
        <span className="block text-sm text-muted-foreground">
          Les données sont sur le poste principal ; ce poste n'en garde
          aucune. Il faudra l'adresse affichée par Gescom Serveur là-bas.
        </span>
      </button>

      {enCours && (
        <p className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Enregistrement…
        </p>
      )}
      {erreur && (
        <div className="flex items-start gap-2 text-sm rounded-md bg-red-500/10 text-red-600 p-3">
          <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0" />
          <p>{erreur}</p>
        </div>
      )}
    </Cadre>
  );
}

interface PropsLocal {
  /** Le serveur de cette machine répond. */
  onPret: () => void;
  /** Finalement, ce poste est une caisse d'un autre poste. */
  onCaisse: () => void;
}

/**
 * Le serveur de cette machine : on le joint, ou on le lance
 * (`demarrer_serveur_local`, côté Rust). S'il ne vient pas, on le dit
 * et on ne fait pas semblant : pas d'écran de connexion qui échouerait
 * au premier mot de passe.
 */
export function PageServeurLocal({ onPret, onCaisse }: PropsLocal) {
  const [erreur, setErreur] = useState("");
  const [essai, setEssai] = useState(0);

  useEffect(() => {
    let annule = false;
    setErreur("");
    invoke("demarrer_serveur_local")
      .then(() => { if (!annule) onPret(); })
      .catch(e => { if (!annule) setErreur(String(e)); });
    return () => { annule = true; };
  }, [essai]);

  if (!erreur) {
    return (
      <Cadre>
        <p className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Démarrage du serveur de ce poste…
        </p>
      </Cadre>
    );
  }

  return (
    <Cadre>
      <div className="text-center space-y-1">
        <AlertTriangle className="h-9 w-9 mx-auto text-red-600" />
        <h1 className="text-xl font-semibold">Le serveur de ce poste ne répond pas</h1>
        <p className="text-sm text-muted-foreground">
          Ce poste garde la base de la boutique : sans son serveur, rien
          ne peut s'enregistrer.
        </p>
      </div>
      <div className="flex items-start gap-2 text-sm rounded-md bg-red-500/10 text-red-600 p-3"
        data-testid="erreur-serveur-local">
        <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0" />
        <p>{erreur}</p>
      </div>
      <div className="flex gap-2">
        <Button className="flex-1" onClick={() => setEssai(n => n + 1)}>
          <RotateCw className="h-4 w-4 mr-2" /> Réessayer
        </Button>
        <Button variant="outline" className="flex-1" onClick={onCaisse}>
          Ce poste est une caisse d'un autre poste
        </Button>
      </div>
    </Cadre>
  );
}
