// components/RechercheServeur.tsx — « Rechercher le serveur » (27/10/2026).
//
// La caisse pose sa question sur le réseau local (commande locale
// `rechercher_serveurs` : diffusion UDP sur le port 40000) et liste les
// serveurs Gescom qui répondent. Un clic remplit l'adresse — le NOM du
// poste de préférence, qui ne change pas quand la box redistribue les
// adresses — que l'écran TESTE ensuite comme une adresse tapée. Rien
// trouvé : on le dit, avec les causes probables, et la saisie à la main
// reste là.
//
// Utilisé par l'écran de branchement (PageServeur) et Paramètres →
// Réseau (OngletReseau). Hors de l'application installée, rien : un
// navigateur ne peut pas diffuser sur le réseau.

import { useState } from "react";
import { AlertTriangle, Loader2, Search, Server } from "lucide-react";

import { Button } from "@/components/ui/button";
import { appeler as invoke, estDansTauri } from "@/lib/pont";

/** Ce que rend `rechercher_serveurs` (`noyau::decouverte::ServeurTrouve`). */
export interface ServeurTrouve {
  poste: string;
  port: number;
  adresses: string[];
  vu_de: string[];
  version_protocole: number;
  version_serveur: string;
  adresse_nom: string;
  adresse_ip: string;
  adresse_conseillee: string;
  nom_resolu: boolean;
  joignable: boolean;
  meme_protocole: boolean;
}

interface Props {
  /** Un serveur choisi : l'adresse à remplir, puis à tester. */
  onChoisir: (adresse: string) => void;
  desactive?: boolean;
}

export function RechercheServeur({ onChoisir, desactive }: Props) {
  const [enCours, setEnCours] = useState(false);
  const [trouves, setTrouves] = useState<ServeurTrouve[] | null>(null);
  const [erreur, setErreur] = useState("");

  if (!estDansTauri()) return null;

  async function rechercher() {
    setEnCours(true);
    setErreur("");
    setTrouves(null);
    try {
      setTrouves(await invoke<ServeurTrouve[]>("rechercher_serveurs"));
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="space-y-2" data-testid="recherche-serveur">
      <Button type="button" variant="outline" onClick={rechercher} disabled={enCours || desactive}>
        {enCours
          ? <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          : <Search className="mr-2 h-4 w-4" />}
        {enCours ? "Recherche sur le réseau…" : "Rechercher le serveur"}
      </Button>

      {erreur && (
        <p className="flex items-start gap-1.5 text-xs text-destructive" role="alert">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" /> {erreur}
        </p>
      )}

      {trouves && trouves.length === 0 && (
        <div className="space-y-1 rounded-md border border-border border-l-4 border-l-amber-600 bg-card p-3 text-xs" role="status">
          <p className="font-medium">Aucun serveur Gescom n'a répondu sur ce réseau.</p>
          <p className="text-muted-foreground">Causes probables :</p>
          <ul className="list-disc space-y-0.5 pl-4 text-muted-foreground">
            <li>
              le pare-feu du poste principal bloque la recherche (UDP 40000) : réinstaller
              Gescom Serveur, ou lancer <span className="font-mono">outils\parefeu.ps1 -Ouvrir</span> en
              administrateur ;
            </li>
            <li>ce poste n'est pas sur le même réseau (autre box, Wi-Fi invité, câble débranché) ;</li>
            <li>Gescom Serveur est arrêté sur le poste principal.</li>
          </ul>
          <p>Sinon, saisir l'adresse à la main : la fenêtre Gescom Serveur l'affiche.</p>
        </div>
      )}

      {trouves && trouves.length > 0 && (
        <div className="space-y-1.5" role="status">
          <p className="text-xs text-muted-foreground">
            {trouves.length === 1 ? "Un serveur a répondu" : `${trouves.length} serveurs ont répondu`} :
            cliquer pour remplir l'adresse, qui sera essayée.
          </p>
          <ul className="space-y-1.5">
            {trouves.map(t => {
              const ips = t.adresses.filter(a => a !== "127.0.0.1");
              return (
                <li key={`${t.poste}:${t.port}`}>
                  <button
                    type="button"
                    className="w-full rounded-md border border-border p-2.5 text-left hover:bg-muted/50
                               focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary
                               disabled:cursor-not-allowed disabled:opacity-60"
                    disabled={!t.joignable || desactive}
                    onClick={() => onChoisir(t.adresse_conseillee)}
                  >
                    <span className="flex flex-wrap items-center gap-2 text-sm font-medium">
                      <Server className="h-4 w-4 shrink-0 text-primary" />
                      {t.poste || "Serveur sans nom"}
                      <span className="font-mono text-xs text-muted-foreground">{t.adresse_conseillee}</span>
                    </span>
                    <span className="mt-0.5 block text-[11px] text-muted-foreground">
                      {(ips.length ? ips.join(", ") : t.adresse_ip)} · Gescom Serveur {t.version_serveur}
                      {!t.nom_resolu && t.adresse_nom
                        ? " · son nom ne répond pas depuis ce poste : adresse IP proposée (elle peut changer)"
                        : ""}
                    </span>
                    {!t.joignable && (
                      <span className="mt-0.5 block text-[11px] text-destructive">
                        Port {t.port} hors de la plage 40000-40100 : Gescom ne peut pas le joindre.
                      </span>
                    )}
                    {t.joignable && !t.meme_protocole && (
                      <span className="mt-0.5 block text-[11px] text-destructive">
                        Autre version de Gescom : mettre ce poste et le serveur à la même version.
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
