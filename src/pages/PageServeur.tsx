// pages/PageServeur.tsx — le premier écran, avant même la connexion.
//
// Le client v2 ne sait parler qu'au serveur : le monoposte reste la v1.
// Tant qu'on ne sait pas OÙ joindre le serveur, il n'y a rien à
// afficher — pas même un écran de connexion, puisqu'il n'y a personne à
// qui demander le mot de passe.
//
// On ne se contente pas d'enregistrer l'adresse : on l'ESSAIE d'abord.
// Une adresse acceptée sans essai donne une application qui paraît
// installée et qui échoue au premier geste, dans la boutique, devant un
// client. Mieux vaut refuser tout de suite et dire quoi vérifier.

import { useEffect, useState } from "react";
import { appeler as invoke, definirServeur, etatReseau } from "@/lib/pont";
import { Loader2, Network, CheckCircle2, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface Sante {
  version_serveur: string;
  version_protocole: number;
  base_saine: boolean;
  postes_connectes: number;
}

interface Props {
  /** Appelé une fois l'adresse essayée et enregistrée. */
  onConfigure: () => void;
}

export function PageServeur({ onConfigure }: Props) {
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

  async function essayer() {
    setEnCours(true);
    setTest(null);
    setErreur("");
    try {
      const s = await invoke<Sante>("tester_serveur", { adresse: serveur.trim() });
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
      await invoke("definir_config_reseau", {
        mode: "poste",
        serveur: serveur.trim(),
        posteNom: posteNom.trim() || null,
      });
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

  const adresseOk = serveur.trim().length > 0;

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-muted/30">
      <div className="w-full max-w-md space-y-5">
        <div className="text-center space-y-1">
          <Network className="h-9 w-9 mx-auto text-primary" />
          <h1 className="text-xl font-semibold">Brancher ce poste</h1>
          <p className="text-sm text-muted-foreground">
            Gescom travaille sur le serveur de la boutique. Saisir
            l'adresse affichée par&nbsp;<b>Gescom Serveur</b> sur le poste
            principal.
          </p>
        </div>

        <div className="bg-background border border-border rounded-lg p-5 space-y-4">
          <div>
            <Label>Adresse du serveur</Label>
            <Input
              className="mt-1 font-mono"
              placeholder="192.168.1.10:7300"
              value={serveur}
              onChange={(e) => { setServeur(e.target.value); setTest(null); }}
              onKeyDown={(e) => { if (e.key === "Enter" && adresseOk) essayer(); }}
            />
            <p className="text-[11px] text-muted-foreground mt-1">
              Le serveur l'affiche au démarrage, sous « pour les caisses ».
            </p>
          </div>

          <div>
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
          </div>

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
                            bg-red-500/10 text-red-600 p-3">
              <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0" />
              <p>{erreur}</p>
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" className="flex-1"
              disabled={!adresseOk || enCours} onClick={essayer}>
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
          poste principal, que les deux machines sont sur le même réseau,
          et que le pare-feu laisse passer le port.
        </p>
      </div>
    </div>
  );
}
