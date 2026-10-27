// components/OngletReseau.tsx — Paramètres → Réseau.
//
// L'écran qui manquait : sans lui, désigner un serveur demandait
// d'éditer `poste.json` à la main, ce qu'on ne fait pas faire à un
// commerçant au téléphone.
//
// Deux rôles (D22/D24, révisés le 25/10/2026) ; la fenêtre ne parle
// qu'au serveur dans les deux cas :
//
//   garde la base — le serveur est sur cette machine (127.0.0.1) ;
//   poste caisse  — le serveur est sur le poste principal, à l'adresse
//                   saisie ; ce poste ne garde aucune donnée.
//
// L'adresse du poste principal se tape ou se trouve (« Rechercher le
// serveur », 27/10/2026 : utile aussi quand le serveur a changé de port).
//
// Changer de rôle est un geste VOLONTAIRE, jamais une bascule
// automatique. Devenir caisse exige que le serveur réponde AVANT d'être
// enregistré : un poste basculé vers une adresse injoignable ne peut
// plus rien faire — ni vendre, ni revenir en arrière sans repasser par
// ici.

import { useEffect, useState } from "react";
import {
  AlertTriangle, CheckCircle2, Database, Loader2, Network, Server, Wifi,
} from "lucide-react";
import { ask, message } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { appeler as invoke, definirServeur, synchroniserConfig, ADRESSE_LOCALE } from "@/lib/pont";
import { garderLaBase } from "@/pages/PageRolePoste";
import { RechercheServeur } from "@/components/RechercheServeur";

interface ConfigReseau {
  mode: string;
  serveur: string;
  poste_nom: string;
  poste_empreinte: string;
}

interface Sante {
  version_protocole: number;
  version_serveur: string;
  demarre_le: string;
  postes_connectes: number;
  base_saine: boolean;
  derniere_sauvegarde?: string | null;
}

export function OngletReseau() {
  const [config, setConfig] = useState<ConfigReseau | null>(null);
  // Le rôle affiché : celui du fichier, ou « poste » quand on a demandé
  // à faire de ce poste une caisse (rien n'est écrit avant Enregistrer).
  const [devenirCaisse, setDevenirCaisse] = useState(false);
  const [serveur, setServeur] = useState("");
  const [posteNom, setPosteNom] = useState("");
  const [test, setTest] = useState<Sante | null>(null);
  const [erreurTest, setErreurTest] = useState("");
  const [enTest, setEnTest] = useState(false);
  const [enregistre, setEnregistre] = useState(false);

  useEffect(() => {
    invoke<ConfigReseau>("lire_config_reseau")
      .then(c => {
        setConfig(c);
        setServeur(c.mode === "poste" ? c.serveur : "");
        setPosteNom(c.poste_nom);
      })
      .catch(e => console.error("Config réseau :", e));
  }, []);

  const gardeLaBase = config?.mode === "monoposte" && !devenirCaisse;
  const mode = gardeLaBase ? "monoposte" : "poste";

  const modifie =
    !!config
    && (mode !== config.mode
      || (mode === "poste" && serveur.trim() !== config.serveur)
      || posteNom.trim() !== config.poste_nom);

  // L'adresse du champ, ou celle qu'un serveur trouvé vient d'y mettre.
  async function tester(adresse: string = serveur) {
    setEnTest(true);
    setTest(null);
    setErreurTest("");
    try {
      const s = await invoke<Sante>("tester_serveur", { adresse });
      setTest(s);
    } catch (e) {
      setErreurTest(String(e));
    } finally {
      setEnTest(false);
    }
  }

  async function enregistrer() {
    // Basculer en poste caisse sans avoir joint le serveur, c'est
    // condamner le poste : il ne joint personne. On exige donc un test
    // réussi dès que l'adresse change.
    if (mode === "poste" && serveur.trim() !== config?.serveur && !test) {
      await message(
        "Tester la connexion avant d'enregistrer l'adresse.\n\n"
        + "Sans serveur joignable, ce poste ne pourra plus ni vendre ni "
        + "revenir en arrière autrement que par cet écran.",
        { title: "Test requis", kind: "warning" },
      );
      return;
    }

    try {
      const c = await invoke<ConfigReseau>("definir_config_reseau", {
        mode,
        serveur: mode === "poste" ? serveur.trim() : null,
        posteNom: posteNom.trim() || null,
      });
      setConfig(c);
      setDevenirCaisse(false);
      // Le navigateur n'est qu'un miroir de `poste.json` : on l'aligne
      // tout de suite, sinon le prochain appel partirait encore au
      // mauvais endroit.
      // Rust rend l'adresse retenue — en monoposte, le serveur local au
      // port qu'il a pris dans la plage (D39).
      definirServeur(mode, c.serveur || ADRESSE_LOCALE);
      await synchroniserConfig();
      setEnregistre(true);
      setTimeout(() => setEnregistre(false), 3000);

      if (mode === "poste") {
        await message(
          "Ce poste est une caisse de " + c.serveur + ".\n\nFermer et rouvrir "
          + "Gescom, puis se connecter : l'identifiant est vérifié par le "
          + "serveur du poste principal.",
          { title: "Rôle enregistré" },
        );
      }
    } catch (e) {
      await message(String(e), { title: "Enregistrement", kind: "error" });
    }
  }

  /** Une caisse devient le poste qui garde la base : geste volontaire. */
  async function garderIci() {
    const oui = await ask(
      "Ce poste va garder sa propre base, sur cette machine.\n\n"
      + "Les ventes enregistrées sur le poste principal NE s'y trouvent "
      + "pas : c'est une autre base. À faire seulement si ce poste devient "
      + "le poste principal, ou travaille seul.",
      { title: "Ce poste garde la base", kind: "warning" },
    );
    if (!oui) return;
    try {
      await garderLaBase();
      // Repartir de zéro : la fenêtre joint (ou lance) son serveur, puis
      // redemande la connexion — le compte est celui de CETTE base.
      window.location.reload();
    } catch (e) {
      await message(String(e), { title: "Enregistrement", kind: "error" });
    }
  }

  if (!config) {
    return (
      <div className="flex items-center gap-2 py-8 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" /> Lecture de la configuration…
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-6">

      {/* ---- Ce poste ---- */}
      {gardeLaBase ? (
        <div className="rounded-lg border border-border p-4" data-testid="role-poste">
          <div className="flex items-start gap-3">
            <Database className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            <div className="space-y-2">
              <p className="text-sm font-medium">Ce poste garde la base</p>
              <p className="text-xs text-muted-foreground">
                Le serveur de la boutique tourne sur cette machine
                ({ADRESSE_LOCALE}) : Gescom le lance s'il ne répond pas.
                Les autres caisses le rejoignent à l'adresse que Gescom
                Serveur affiche « pour les caisses ».
              </p>
              <Button variant="outline" size="sm" onClick={() => { setDevenirCaisse(true); setTest(null); }}>
                En faire une caisse d'un autre poste
              </Button>
            </div>
          </div>
        </div>
      ) : (
        <div className="rounded-lg border border-border p-4" data-testid="role-poste">
          <div className="flex items-start gap-3">
            <Network className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            <div className="space-y-2">
              <p className="text-sm font-medium">Poste caisse</p>
              <p className="text-xs text-muted-foreground">
                La base de la boutique est sur le poste principal. Tout
                passe par lui : ce poste ne garde aucune donnée.
              </p>
              {devenirCaisse ? (
                <Button variant="outline" size="sm" onClick={() => { setDevenirCaisse(false); setTest(null); }}>
                  Annuler : ce poste garde la base
                </Button>
              ) : (
                <Button variant="outline" size="sm" onClick={garderIci}>
                  Ce poste garde la base
                </Button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ---- Serveur (caisse seulement) ---- */}
      {mode === "poste" && (
        <div className="space-y-3 rounded-lg border border-border p-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-[220px] flex-1">
              <Label className="mb-1 block text-xs">
                Adresse du poste principal
              </Label>
              <Input value={serveur} onChange={e => { setServeur(e.target.value); setTest(null); }}
                placeholder="192.168.1.10" className="h-9 font-mono text-sm" />
              <p className="mt-1 text-[11px] text-muted-foreground">
                Sans port, 40000 est supposé (le serveur écoute entre 40000 et 40100). L'adresse est celle du réseau
                local de la boutique, pas une adresse Internet.
              </p>
            </div>
            <Button variant="outline" onClick={() => tester()}
              disabled={enTest || !serveur.trim()}>
              {enTest
                ? <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                : <Wifi className="mr-2 h-4 w-4" />}
              Tester
            </Button>
          </div>

          {/* Le serveur a changé de port, ou on ne connaît pas son adresse :
              le chercher sur le réseau, un clic remplit et teste. */}
          <RechercheServeur
            desactive={enTest}
            onChoisir={(a) => { setServeur(a); tester(a); }}
          />

          {erreurTest && (
            <div className="flex items-start gap-2 rounded border border-destructive/40
                            bg-destructive/5 p-2.5 text-xs text-destructive">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <span>{erreurTest}</span>
            </div>
          )}

          {test && (
            <div className="rounded border border-green-600/40 bg-green-600/5 p-3 text-xs">
              <p className="flex items-center gap-1.5 font-medium text-green-700">
                <CheckCircle2 className="h-3.5 w-3.5" /> Serveur joignable
              </p>
              <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-muted-foreground">
                <dt>Version</dt><dd>{test.version_serveur}</dd>
                <dt>Protocole</dt><dd>v{test.version_protocole}</dd>
                <dt>Postes connectés</dt><dd>{test.postes_connectes}</dd>
                <dt>Base</dt>
                <dd className={test.base_saine ? "" : "font-medium text-destructive"}>
                  {test.base_saine ? "saine" : "abîmée — restaurer une sauvegarde"}
                </dd>
                <dt>Dernière sauvegarde</dt>
                <dd>{test.derniere_sauvegarde ?? "aucune depuis le démarrage"}</dd>
              </dl>
            </div>
          )}
        </div>
      )}

      {/* ---- Identité du poste ---- */}
      <div>
        <h2 className="text-sm font-semibold">Identité de ce poste</h2>
        <div className="mt-3 flex flex-wrap items-end gap-3">
          <div className="min-w-[220px] flex-1">
            <Label className="mb-1 block text-xs">Nom</Label>
            <Input value={posteNom} onChange={e => setPosteNom(e.target.value)}
              placeholder="Caisse 1" className="h-9 text-sm" />
            <p className="mt-1 text-[11px] text-muted-foreground">
              C'est ce nom que le patron verra dans la liste des postes
              connectés.
            </p>
          </div>
          <div>
            <Label className="mb-1 block text-xs">Empreinte</Label>
            <code className="block rounded border border-border bg-muted/40 px-3 py-2
                             font-mono text-xs">
              {config.poste_empreinte.slice(0, 8)}…
            </code>
          </div>
        </div>
      </div>

      {/* ---- Enregistrer ---- */}
      <div className="flex items-center gap-3 border-t border-border pt-4">
        <Button onClick={enregistrer} disabled={!modifie}>
          <Server className="mr-2 h-4 w-4" /> Enregistrer
        </Button>
        {enregistre && (
          <span className="flex items-center gap-1.5 text-xs text-green-700">
            <CheckCircle2 className="h-3.5 w-3.5" /> Enregistré
          </span>
        )}
        {modifie && (
          <span className="text-xs text-muted-foreground">
            Modifications non enregistrées
          </span>
        )}
      </div>
    </div>
  );
}
