// PaletteCommandes — Ctrl+K : aller quelque part ou faire un geste,
// sans chercher le bouton.
//
// Une seule liste, groupée : « Aller à » (les mêmes entrées que le
// menu, filtrées par les mêmes droits), « Sur cette page » (ce que
// l'écran courant a déclaré), « Paramètres » (les onglets), « Compte »
// — et, dès deux lettres, les **clients et fournisseurs** dont le nom
// correspond : choisir l'un ouvre sa fiche. Flèches + Entrée au
// clavier, la souris pour le reste.

import { useEffect, useMemo, useRef, useState } from "react";
import { Search, CornerDownLeft, User, Truck } from "lucide-react";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { peut } from "@/lib/droits";
import { appeler as invoke } from "@/lib/pont";
import {
  actionsDePage, filtrer, surChangementActions, type ActionPalette,
} from "@/lib/palette";

interface Props {
  ouverte: boolean;
  onFermer: () => void;
  /** Les actions globales, déjà filtrées par le droit. */
  globales: ActionPalette[];
  onNaviguer: (page: string, params?: unknown) => void;
}

interface TiersTrouve { id: string; nom: string; code?: string; telephone?: string; }

/** Les tiers dont le nom correspond — cinq de chaque, pas plus. */
async function chercherTiers(requete: string): Promise<{ clients: TiersTrouve[]; fournisseurs: TiersTrouve[] }> {
  const rien = { donnees: [] as TiersTrouve[] };
  const [c, f] = await Promise.all([
    peut("clients:creer")
      ? invoke<{ donnees: TiersTrouve[] }>("lire_clients_pagines", {
          page: 0, limite: 5, recherche: requete,
          avecCreancesSeulement: false, ventesFiltre: null, tri: null,
        }).catch(() => rien)
      : rien,
    peut("achats:creer")
      ? invoke<{ donnees: TiersTrouve[] }>("lire_fournisseurs_pagines", {
          page: 0, limite: 5, recherche: requete, avecDettesSeulement: null,
        }).catch(() => rien)
      : rien,
  ]);
  return { clients: c.donnees ?? [], fournisseurs: f.donnees ?? [] };
}

export function PaletteCommandes({ ouverte, onFermer, globales, onNaviguer }: Props) {
  const [requete, setRequete] = useState("");
  const [curseur, setCurseur] = useState(0);
  const [version, setVersion] = useState(0);
  const listeRef = useRef<HTMLDivElement>(null);
  const [tiers, setTiers] = useState<ActionPalette[]>([]);

  // La recherche de tiers part au serveur : on attend que la frappe se
  // pose (200 ms), et on jette une réponse arrivée après une frappe
  // plus récente.
  useEffect(() => {
    const q = requete.trim();
    if (!ouverte || q.length < 2) { setTiers([]); return; }
    let vivant = true;
    const t = setTimeout(async () => {
      const r = await chercherTiers(q);
      if (!vivant) return;
      const clients: ActionPalette[] = r.clients.map((c) => ({
        id: `client:${c.id}`, libelle: c.nom, groupe: "Clients",
        detail: [c.code, c.telephone].filter(Boolean).join(" · ") || "ouvrir la fiche",
        executer: () => onNaviguer("fiche_client", { clientId: c.id }),
      }));
      const fournisseurs: ActionPalette[] = r.fournisseurs.map((f) => ({
        id: `fournisseur:${f.id}`, libelle: f.nom, groupe: "Fournisseurs",
        detail: f.telephone || "ouvrir la fiche",
        executer: () => onNaviguer("fiche_fournisseur", { fournisseurId: f.id }),
      }));
      setTiers([...clients, ...fournisseurs]);
    }, 200);
    return () => { vivant = false; clearTimeout(t); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requete, ouverte]);

  // Les actions de page bougent quand un écran se monte : on se
  // réabonne pour les revoir sans rouvrir la palette.
  useEffect(() => surChangementActions(() => setVersion((v) => v + 1)), []);

  // Sur cette page d'abord : c'est le geste qu'on cherche le plus
  // souvent, et il est le plus court à taper.
  const toutes = useMemo(
    () => [...actionsDePage(), ...globales],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [globales, version, ouverte],
  );
  // Les tiers viennent déjà filtrés par le serveur : on les ajoute
  // après les actions, sans les repasser au filtre local.
  const visibles = useMemo(() => [...filtrer(toutes, requete), ...tiers], [toutes, requete, tiers]);

  useEffect(() => {
    if (ouverte) { setRequete(""); setCurseur(0); }
  }, [ouverte]);
  useEffect(() => { setCurseur(0); }, [requete]);

  // La ligne sous le curseur reste visible quand on descend au clavier.
  useEffect(() => {
    listeRef.current
      ?.querySelector<HTMLElement>(`[data-rang="${curseur}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [curseur]);

  function lancer(a: ActionPalette | undefined) {
    if (!a) return;
    onFermer();
    // Après la fermeture : une action qui ouvre une fenêtre ne doit pas
    // la voir se refermer avec la palette.
    setTimeout(a.executer, 0);
  }

  function surTouche(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setCurseur((c) => Math.min(c + 1, visibles.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setCurseur((c) => Math.max(c - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      lancer(visibles[curseur]);
    }
  }

  // Les groupes dans l'ordre d'apparition, les lignes numérotées en
  // continu pour que les flèches traversent les groupes.
  const groupes: { nom: string; lignes: { a: ActionPalette; rang: number }[] }[] = [];
  visibles.forEach((a, rang) => {
    let g = groupes.find((x) => x.nom === a.groupe);
    if (!g) { g = { nom: a.groupe, lignes: [] }; groupes.push(g); }
    g.lignes.push({ a, rang });
  });

  return (
    <Dialog open={ouverte} onOpenChange={(o) => { if (!o) onFermer(); }}>
      {/* `sm:max-w-3xl` et pas `max-w-3xl` : DialogContent pose
          `sm:max-w-sm`, qui l'emporterait sur une classe sans préfixe —
          c'est pour ça que « plus large » ne changeait rien. */}
      <DialogContent className="sm:max-w-3xl w-[92vw] gap-0 p-0 overflow-hidden">
        <DialogTitle className="sr-only">Palette de commandes</DialogTitle>
        <div className="flex items-center gap-2 border-b px-3">
          <Search className="h-4 w-4 shrink-0 text-muted-foreground" />
          <input
            autoFocus
            value={requete}
            onChange={(e) => setRequete(e.target.value)}
            onKeyDown={surTouche}
            placeholder="Aller à… faire… ou le nom d'un client, d'un fournisseur"
            className="h-12 w-full bg-transparent text-[15px] outline-none placeholder:text-muted-foreground"
          />
          <kbd className="hidden sm:inline rounded border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
            Échap
          </kbd>
        </div>
        <div ref={listeRef} className="max-h-[60vh] overflow-y-auto py-1">
          {visibles.length === 0 && (
            <p className="px-4 py-8 text-center text-sm text-muted-foreground">
              Rien ne correspond à « {requete} ».
              {requete.trim().length < 2 && " Deux lettres au moins pour chercher un client ou un fournisseur."}
            </p>
          )}
          {groupes.map((g) => (
            <div key={g.nom} className="py-1">
              <div className="flex items-center gap-1.5 px-4 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                {g.nom === "Clients" && <User className="h-3 w-3" />}
                {g.nom === "Fournisseurs" && <Truck className="h-3 w-3" />}
                {g.nom}
              </div>
              {g.lignes.map(({ a, rang }) => (
                <button
                  key={a.id}
                  data-rang={rang}
                  onMouseEnter={() => setCurseur(rang)}
                  onClick={() => lancer(a)}
                  className={cn(
                    "flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm",
                    rang === curseur ? "bg-primary text-primary-foreground" : "hover:bg-muted",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate">{a.libelle}</div>
                    {a.detail && (
                      <div className={cn(
                        "truncate text-[11px]",
                        rang === curseur ? "text-primary-foreground/80" : "text-muted-foreground",
                      )}>
                        {a.detail}
                      </div>
                    )}
                  </div>
                  {rang === curseur && <CornerDownLeft className="h-3.5 w-3.5 shrink-0 opacity-70" />}
                </button>
              ))}
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
