// pages/PageServeur.tsx — le premier écran, avant même la connexion.
//
// La CAISSE d'un autre poste (D24, révisé le 25/10/2026 ; le poste qui
// garde la base passe par PageRolePoste et n'a rien à saisir).
// Tant qu'on ne sait pas OÙ joindre le serveur, il n'y a rien à
// afficher — pas même un écran de connexion, puisqu'il n'y a personne à
// qui demander le mot de passe.
//
// L'adresse se tape, ou se trouve : « Rechercher le serveur » (27/10/2026)
// interroge le réseau local et un clic la remplit.
//
// D55 (01/11/2026) : le même écran s'ouvre aussi depuis l'écran de
// connexion, par « Réglage du serveur » — sur une caisse d'un autre poste
// et dans Gescom Équipe —, quand le serveur a changé de port ou d'IP.
// Il ne touche qu'à l'adresse : rien des données, le serveur exige
// toujours la connexion pour le reste. Dans Équipe (`equipe`), l'adresse
// va dans son `equipe.json` et passe devant celle de la caisse du poste ;
// « Reprendre celle de la caisse » l'efface.
//
// On ne se contente pas d'enregistrer l'adresse : on l'ESSAIE d'abord.
// Une adresse acceptée sans essai donne une application qui paraît
// installée et qui échoue au premier geste, dans la boutique, devant un
// client. Mieux vaut refuser tout de suite et dire quoi vérifier.

import { useEffect, useState } from "react";
import { appeler as invoke, definirServeur, estDansTauri, etatReseau, sante } from "@/lib/pont";
import { Loader2, Network, CheckCircle2, AlertTriangle, ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RechercheServeur } from "@/components/RechercheServeur";

interface Sante {
  version_serveur: string;
  version_protocole: number;
  base_saine: boolean;
  postes_connectes: number;
}

interface Props {
  /** Appelé une fois l'adresse essayée et enregistrée. */
  onConfigure: () => void;
  /** Finalement, ce poste garde la base (D24 : un geste volontaire). */
  onGarderBase?: () => Promise<void>;
  /** Ouvert depuis l'écran de connexion (D55) : y revenir sans rien changer. */
  onAnnuler?: () => void;
  /** Gescom Équipe (D55) : son adresse à elle, pas de nom de poste à régler. */
  equipe?: boolean;
}

/** D'où vient l'adresse d'Équipe (`lire_config_reseau`, D55). */
const SOURCES: Record<string, string> = {
  equipe: "réglée dans Gescom Équipe",
  caisse: "celle de la caisse de ce poste",
  defaut: "ce poste, faute d'autre réglage",
};

export function PageServeur({ onConfigure, onGarderBase, onAnnuler, equipe }: Props) {
  const actuel = etatReseau();
  const [serveur, setServeur] = useState("");
  const [posteNom, setPosteNom] = useState("");
  const [test, setTest] = useState<Sante | null>(null);
  const [enCours, setEnCours] = useState(false);
  const [erreur, setErreur] = useState("");

  useEffect(() => {
    const e = etatReseau();
    // L'adresse précédente, s'il y en avait une : on revient souvent
    // ici après une panne, pas après un déménagement.
    if (e.serveur) setServeur(e.serveur);
    if (e.posteNom) setPosteNom(e.posteNom);
  }, []);

  // L'adresse essayée : celle du champ, ou celle qu'un clic sur un serveur
  // trouvé vient d'y mettre (l'état n'est pas encore à jour à cet instant).
  async function essayer(adresse: string = serveur) {
    setEnCours(true);
    setTest(null);
    setErreur("");
    try {
      // Dans un navigateur (mise au point, pas l'application installée),
      // pas de commande Rust : on lit `/sante` directement.
      const s = estDansTauri()
        ? await invoke<Sante>("tester_serveur", { adresse: adresse.trim() })
        : await sante(adresse.trim()).catch(() => {
            throw `Serveur injoignable sur ${adresse.trim()}.`;
          }) as Sante;
      setTest(s);
      if (!s.base_saine) {
        setErreur(
          "Le serveur répond, mais sa base est abîmée. "
          + "La restaurer avant de brancher les caisses.",
        );
      }
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  async function enregistrer() {
    setEnCours(true);
    setErreur("");
    try {
      // Dans un navigateur, l'adresse n'est retenue que par lui (le
      // miroir de `pont.ts`) : il n'a pas de fichier de réglage.
      if (estDansTauri()) {
        await invoke("definir_config_reseau", {
          mode: "poste",
          serveur: serveur.trim(),
          posteNom: equipe ? null : posteNom.trim() || null,
        });
      }
      // Le navigateur n'est qu'un miroir de `poste.json` : on l'aligne
      // tout de suite, sinon le prochain appel partirait encore à
      // l'ancienne adresse.
      definirServeur("poste", serveur.trim());
      onConfigure();
    } catch (e) {
      setErreur(String(e));
      setEnCours(false);
    }
  }

  // Équipe : effacer son adresse, pour reprendre celle de la caisse.
  async function reprendreCaisse() {
    setEnCours(true);
    setErreur("");
    try {
      await invoke("definir_config_reseau", { mode: "poste", serveur: null });
      onConfigure();
    } catch (e) {
      setErreur(String(e));
      setEnCours(false);
    }
  }

  const adresseOk = serveur.trim().length > 0;
  const titre = onAnnuler ? "Réglage du serveur" : "Brancher ce poste";

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-muted/30">
      <div className="w-full max-w-md space-y-5">
        <div className="text-center space-y-1">
          <Network className="h-9 w-9 mx-auto text-primary" />
          <h1 className="text-xl font-semibold">{titre}</h1>
          <p className="text-sm text-muted-foreground">
            {equipe ? "Gescom Équipe" : "Gescom"} travaille sur le serveur de la boutique.
            Le rechercher, ou saisir l'adresse affichée par&nbsp;<b>Gescom Serveur</b> sur
            le poste principal.
          </p>
        </div>

        {onAnnuler && actuel.serveur && (
          <div className="rounded-md border border-border bg-background px-3 py-2 text-sm" data-testid="adresse-actuelle">
            <p>
              Adresse actuelle : <span className="font-mono">{actuel.serveur}</span>
            </p>
            {equipe && actuel.source && SOURCES[actuel.source] && (
              <p className="text-xs text-muted-foreground">{SOURCES[actuel.source]}</p>
            )}
            {equipe && actuel.source === "equipe" && actuel.serveurCaisse && (
              <button type="button" disabled={enCours} onClick={reprendreCaisse}
                className="mt-1 text-xs underline text-muted-foreground hover:text-foreground">
                Reprendre celle de la caisse ({actuel.serveurCaisse})
              </button>
            )}
          </div>
        )}

        <div className="bg-background border border-border rounded-lg p-5 space-y-4">
          <div>
            <Label htmlFor="adresse-serveur">Adresse du serveur</Label>
            <Input
              id="adresse-serveur"
              className="mt-1 font-mono"
              placeholder="192.168.1.10"
              value={serveur}
              onChange={(e) => { setServeur(e.target.value); setTest(null); }}
              onKeyDown={(e) => { if (e.key === "Enter" && adresseOk) essayer(); }}
            />
            <p className="text-[11px] text-muted-foreground mt-1">
              La fenêtre Gescom Serveur l'affiche, sous « Sur les caisses, taper ».
            </p>
          </div>

          {/* Ou la chercher sur le réseau : un clic remplit et essaie. */}
          <RechercheServeur
            desactive={enCours}
            onChoisir={(a) => { setServeur(a); essayer(a); }}
          />

          {!equipe && <div>
            <Label>Nom de ce poste</Label>
            <Input
              className="mt-1"
              placeholder="Caisse 1"
              value={posteNom}
              onChange={(e) => setPosteNom(e.target.value)}
            />
            <p className="text-[11px] text-muted-foreground mt-1">
              C'est ce nom que le patron verra dans la console du serveur.
            </p>
          </div>}

          {test && (
            <div className="flex items-start gap-2 text-sm rounded-md
                            bg-green-500/10 text-green-700 dark:text-green-400 p-3">
              <CheckCircle2 className="h-4 w-4 mt-0.5 shrink-0" />
              <div>
                <p className="font-medium">Serveur joignable</p>
                <p className="text-xs">
                  Version {test.version_serveur} · protocole v
                  {test.version_protocole} · {test.postes_connectes} poste(s)
                  connecté(s)
                </p>
              </div>
            </div>
          )}

          {erreur && (
            <div className="flex items-start gap-2 text-sm rounded-md
                            bg-red-500/10 text-red-600 p-3" role="alert">
              <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0" />
              <p>{erreur}</p>
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" className="flex-1"
              disabled={!adresseOk || enCours} onClick={() => essayer()}>
              {enCours && !test
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Essayer"}
            </Button>
            {/* Enregistrer n'est possible qu'APRÈS un essai réussi. Une
                adresse acceptée sans essai donne une application qui
                paraît installée et qui échoue au premier geste. */}
            <Button className="flex-1"
              disabled={!test || !test.base_saine || enCours}
              onClick={enregistrer}>
              Enregistrer
            </Button>
          </div>
        </div>

        <p className="text-xs text-muted-foreground text-center">
          Le serveur ne répond pas&nbsp;? Vérifier qu'il est démarré sur le
          poste principal (fenêtre Gescom Serveur), que les deux machines
          sont sur le même réseau, et que le pare-feu laisse passer le port.
        </p>

        {onAnnuler && (
          <p className="text-xs text-center">
            <button type="button" className="inline-flex items-center gap-1 underline text-muted-foreground hover:text-foreground"
              disabled={enCours} onClick={onAnnuler}>
              <ArrowLeft className="h-3.5 w-3.5" /> Retour à la connexion, sans rien changer
            </button>
          </p>
        )}

        {onGarderBase && (
          <p className="text-xs text-center">
            <button type="button" className="underline text-muted-foreground hover:text-foreground"
              disabled={enCours}
              onClick={async () => {
                setEnCours(true);
                setErreur("");
                try { await onGarderBase(); } catch (e) { setErreur(String(e)); setEnCours(false); }
              }}>
              Non : ce poste garde la base de la boutique
            </button>
          </p>
        )}
      </div>
    </div>
  );
}
