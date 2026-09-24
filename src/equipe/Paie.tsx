// equipe/Paie.tsx — la paie (PLAN-EQUIPE, chantier G).
//
// Les avances d'abord (D32) : rattachées à la personne, que la fiche de
// paie retiendra. Hors caisse (décision du 24/09) : le moyen dit d'où
// vient l'argent, le tiroir du jour n'est pas touché. Les fiches du mois (G-2) :
// FichesPaie.tsx.

import { useState, useEffect, useCallback } from "react";
import { Loader2, HandCoins, Undo2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { FichesPaie } from "./FichesPaie";
import { Cotisations } from "./Cotisations";

interface Avance {
  id: string; employe_id: string; nom: string; montant: number; retenu: number; reste: number;
  moyen: string; motif: string | null; date_avance: string; statut: "ouverte" | "annulee";
}

const MOYENS: [string, string][] = [["especes", "Espèces"], ["orange_money", "Orange Money"], ["moov_money", "Moov Money"]];
const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;

function Avances() {
  const [personnes, setPersonnes] = useState<{ id: string; nom: string; fonction: string }[]>([]);
  const [liste, setListe] = useState<Avance[] | null>(null);
  const [toutes, setToutes] = useState(false);
  const [qui, setQui] = useState("");
  const [montant, setMontant] = useState("");
  const [moyen, setMoyen] = useState("especes");
  const [motif, setMotif] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const donne = peut("personnel:avancer");

  const charger = useCallback(async () => {
    try {
      setListe(await invoke<Avance[]>("lire_avances", { enCoursSeulement: !toutes }));
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }, [toutes]);
  useEffect(() => { charger(); }, [charger]);
  useEffect(() => {
    invoke<{ id: string; nom: string; fonction: string }[]>("lire_personnel").then(setPersonnes).catch(() => setPersonnes([]));
  }, []);

  async function faire(action: () => Promise<{ nom: string; montant: number }>, texte: (nom: string, m: number) => string) {
    setEnCours(true);
    setAvis(null);
    try {
      const r = await action();
      setAvis({ texte: texte(r.nom, r.montant) });
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  const parPersonne = new Map<string, { nom: string; reste: number }>();
  for (const a of liste ?? []) {
    if (a.statut !== "ouverte") continue;
    const p = parPersonne.get(a.employe_id) ?? { nom: a.nom, reste: 0 };
    p.reste += a.reste;
    parPersonne.set(a.employe_id, p);
  }

  return (
    <div className="space-y-4 max-w-3xl">
      {donne && (
        <div className="border border-border rounded-lg p-4 space-y-3" data-testid="nouvelle-avance">
          <h3 className="text-sm font-semibold">Donner une avance</h3>
          <div className="flex gap-3 flex-wrap items-end">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">À qui
              <select value={qui} onChange={e => setQui(e.target.value)} aria-label="À qui"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background min-w-48">
                <option value="">—</option>
                {personnes.map(p => <option key={p.id} value={p.id}>{p.nom} ({p.fonction})</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Montant
              <Input value={montant} onChange={e => setMontant(e.target.value)} aria-label="Montant de l'avance"
                inputMode="numeric" className="h-9 w-32" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Donnée en
              <select value={moyen} onChange={e => setMoyen(e.target.value)} aria-label="Donnée en"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                {MOYENS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Pour quoi (facultatif)
              <Input value={motif} onChange={e => setMotif(e.target.value)} aria-label="Motif de l'avance" className="h-9" /></label>
            <Button size="sm" disabled={enCours || !qui || !montant.trim()}
              onClick={() => faire(
                () => invoke("donner_avance", { employeId: qui, montant: Number(montant.replace(/\s/g, "")), moyen, motif: motif || null }),
                (nom, m) => `Avance de ${f(m)} à ${nom} notée. Elle se retiendra sur sa prochaine paie.`,
              ).then(() => { setMontant(""); setMotif(""); })}>
              <HandCoins className="h-4 w-4 mr-1" /> Donner l'avance
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">La paie ne passe pas par la caisse du jour : noter seulement comment l'argent a été donné.</p>
        </div>
      )}

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      {parPersonne.size > 0 && (
        <div className="flex gap-2 flex-wrap" data-testid="en-cours-par-personne">
          {[...parPersonne.values()].map(p => (
            <Badge key={p.nom} variant="secondary">{p.nom} : {f(p.reste)} en cours</Badge>
          ))}
        </div>
      )}

      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold">{toutes ? "Toutes les avances" : "Avances en cours"}</h3>
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          <input type="checkbox" checked={toutes} onChange={e => setToutes(e.target.checked)} /> Voir aussi les retenues et annulées
        </label>
      </div>
      {!liste ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-avances">
          {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucune avance en cours.</p>}
          {liste.map(a => (
            <div key={a.id} className="flex items-center gap-3 px-4 py-2 text-sm" data-testid="avance">
              <div className="flex-1">
                <div className="font-medium">{a.nom} — {f(a.montant)}</div>
                <div className="text-xs text-muted-foreground">
                  le {a.date_avance.slice(0, 10).split("-").reverse().join("/")} · {MOYENS.find(m => m[0] === a.moyen)?.[1] ?? a.moyen}
                  {a.motif ? ` · ${a.motif}` : ""}
                  {a.retenu > 0 ? ` · ${f(a.retenu)} déjà retenus` : ""}
                </div>
              </div>
              {a.statut === "annulee" ? <Badge variant="outline">annulée</Badge>
                : a.reste === 0 ? <Badge variant="outline">retenue</Badge>
                : <span className="tabular-nums font-semibold">{f(a.reste)}</span>}
              {donne && a.statut === "ouverte" && a.retenu === 0 && (
                <Button size="sm" variant="ghost" aria-label={`Annuler l'avance de ${a.nom}`} disabled={enCours}
                  onClick={() => faire(
                    () => invoke("annuler_avance", { avanceId: a.id, motif: null }),
                    (nom, m) => `Avance de ${f(m)} à ${nom} annulée : elle ne sera pas retenue.`,
                  )}>
                  <Undo2 className="h-4 w-4" />
                </Button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function Paie() {
  const [vue, setVue] = useState<"avances" | "fiches" | "cotisations">("avances");
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">Paie</h1>
      <div className="flex gap-1" role="tablist">
        {([["avances", "Avances"], ["fiches", "Fiches du mois"], ["cotisations", "Cotisations"]] as const).map(([cle, libelle]) => (
          <button key={cle} role="tab" aria-selected={vue === cle} onClick={() => setVue(cle)}
            className={`px-3 py-1.5 text-sm rounded-lg ${vue === cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
            {libelle}
          </button>
        ))}
      </div>
      {vue === "avances" ? <Avances /> : vue === "fiches" ? <FichesPaie /> : <Cotisations />}
    </div>
  );
}
