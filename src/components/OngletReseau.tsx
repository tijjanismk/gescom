// components/OngletReseau.tsx — Paramètres → Réseau.
//
// L'écran qui manquait : sans lui, désigner un serveur demandait
// d'éditer `poste.json` à la main, ce qu'on ne fait pas faire à un
// commerçant au téléphone.
//
// Deux modes, et un seul est un changement de nature :
//
//   monoposte — la base est ici, comme depuis toujours ;
//   poste caisse — la base est ailleurs, ce poste n'ouvre plus la
//                  sienne et tout passe par le réseau.
//
// Le second exige que le serveur réponde AVANT d'être enregistré. Un
// poste basculé vers une adresse injoignable ne peut plus rien faire —
// ni vendre, ni revenir en arrière sans repasser par ici.

import { useEffect, useState } from "react";
import {
  AlertTriangle, CheckCircle2, Loader2, Network, Server, Wifi,
} from "lucide-react";
import { message } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { appeler as invoke, definirServeur, synchroniserConfig } from "@/lib/pont";
import type { ModeReseau } from "@/lib/pont";

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
  // Fixe : le client v2 est toujours un poste caisse.
  const mode: ModeReseau = "poste";
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
        setServeur(c.serveur);
        setPosteNom(c.poste_nom);
      })
      .catch(e => console.error("Config réseau :", e));
  }, []);

  const modifie =
    !!config
    && (mode !== (config.mode === "poste" ? "poste" : "monoposte")
      || serveur.trim() !== config.serveur
      || posteNom.trim() !== config.poste_nom);

  async function tester() {
    setEnTest(true);
    setTest(null);
    setErreurTest("");
    try {
      const s = await invoke<Sante>("tester_serveur", { adresse: serveur });
      setTest(s);
    } catch (e) {
      setErreurTest(String(e));
    } finally {
      setEnTest(false);
    }
  }

  async function enregistrer() {
    // Basculer en poste caisse sans avoir joint le serveur, c'est
    // condamner le poste : il n'ouvre plus sa base et ne joint pas
    // l'autre. On exige donc un test réussi.
    if (mode === "poste" && !test) {
      await message(
        "Tester la connexion avant de basculer ce poste en caisse.\n\n"
        + "Sans serveur joignable, ce poste ne pourra plus ni vendre ni "
        + "revenir en arrière autrement que par cet écran.",
        { title: "Test requis", kind: "warning" },
      );
      return;
    }

    try {
      const c = await invoke<ConfigReseau>("definir_config_reseau", {
        mode,
        serveur: serveur.trim(),
        posteNom: posteNom.trim(),
      });
      setConfig(c);
      // Le navigateur n'est qu'un miroir de `poste.json` : on l'aligne
      // tout de suite, sinon le prochain appel partirait encore au
      // mauvais endroit.
      definirServeur(mode, c.serveur);
      await synchroniserConfig();
      setEnregistre(true);
      setTimeout(() => setEnregistre(false), 3000);

      await message(
        mode === "poste"
          ? "Ce poste est désormais une caisse.\n\nFermer et rouvrir Gescom, "
            + "puis se connecter : l'identifiant est vérifié par le serveur, "
            + "plus par ce poste."
          : "Ce poste travaille de nouveau seul, sur sa base locale.\n\n"
            + "Fermer et rouvrir Gescom pour repartir proprement.",
        { title: "Mode enregistré" },
      );
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
      {/*
        Le choix « Poste seul » a disparu. Le client v2 ne sait parler
        qu'au serveur : le monoposte reste la v1. Garder le bouton
        reviendrait a offrir un geste qui renvoie le poste a l'ecran de
        branchement — un bouton qui casse l'ecran est pire que pas de
        bouton.
      */}
      <div className="rounded-lg border border-border p-4">
        <div className="flex items-start gap-3">
          <Network className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
          <div>
            <p className="text-sm font-medium">Poste caisse</p>
            <p className="text-xs text-muted-foreground">
              La base de la boutique est sur le poste principal. Tout
              passe par lui : ce poste ne garde aucune donnée.
            </p>
          </div>
        </div>
      </div>

      {/* ---- Serveur ---- */}
      {mode === "poste" && (
        <div className="space-y-3 rounded-lg border border-border p-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-[220px] flex-1">
              <Label className="mb-1 block text-xs">
                Adresse du poste principal
              </Label>
              <Input value={serveur} onChange={e => { setServeur(e.target.value); setTest(null); }}
                placeholder="192.168.1.10:7300" className="h-9 font-mono text-sm" />
              <p className="mt-1 text-[11px] text-muted-foreground">
                Sans port, 7300 est supposé. L'adresse est celle du réseau
                local de la boutique, pas une adresse Internet.
              </p>
            </div>
            <Button variant="outline" onClick={tester}
              disabled={enTest || !serveur.trim()}>
              {enTest
                ? <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                : <Wifi className="mr-2 h-4 w-4" />}
              Tester
            </Button>
          </div>

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
