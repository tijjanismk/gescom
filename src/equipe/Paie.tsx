// equipe/Paie.tsx — la paie (PLAN-EQUIPE, chantier G).
//
// Les avances d'abord (D32) : rattachées à la personne, que la fiche de
// paie retiendra. Hors caisse (décision du 24/09) : le moyen dit d'où
// vient l'argent — sauf « pris dans le tiroir » (D42), où la sortie de
// caisse s'écrit avec elle. Elle peut être datée d'un jour passé et
// s'étaler (« X F par paie »). Les fiches du mois (G-2) : FichesPaie.tsx.

import { useState, useEffect, useCallback } from "react";
import { Loader2, HandCoins, Undo2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { FichesPaie } from "./FichesPaie";
import { Cotisations } from "./Cotisations";
import { Prets } from "./Prets";

interface Avance {
  id: string; employe_id: string; nom: string; montant: number; retenu: number; reste: number;
  moyen: string; motif: string | null; date_avance: string; statut: "ouverte" | "annulee";
  trop_verse_numero: string | null; mensualite: number | null; du_tiroir: boolean;
}

// Le jour local, AAAA-MM-JJ (pas toISOString : il est en UTC).
const jourIso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const ilYA = (n: number) => { const d = new Date(); d.setDate(d.getDate() - n); return jourIso(d); };

const MOYENS: [string, string][] = [["especes", "Espèces"], ["orange_money", "Orange Money"], ["moov_money", "Moov Money"]];
// Une avance née d'un trop-versé (revue du 28/10) : rien n'est sorti
// de la trésorerie, elle se retient sur la paie suivante.
const moyenDit = (a: Avance) => a.moyen === "trop_verse"
  ? `versé en trop${a.trop_verse_numero ? ` sur ${a.trop_verse_numero}` : ""}, repris en avance`
  : MOYENS.find(m => m[0] === a.moyen)?.[1] ?? a.moyen;
const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;

function Avances() {
  const [personnes, setPersonnes] = useState<{ id: string; nom: string; fonction: string }[]>([]);
  const [liste, setListe] = useState<Avance[] | null>(null);
  const [toutes, setToutes] = useState(false);
  const [qui, setQui] = useState("");
  const [montant, setMontant] = useState("");
  const [moyen, setMoyen] = useState("especes");
  const [motif, setMotif] = useState("");
  const [date, setDate] = useState(ilYA(0));
  const [tiroir, setTiroir] = useState(false);
  const [etalee, setEtalee] = useState(false);
  const [mensualite, setMensualite] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const donne = peut("personnel:avancer");
  // Le tiroir ne donne que des espèces, et aujourd'hui (D42).
  const duTiroir = tiroir && moyen === "especes";

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

  async function faire(
    action: () => Promise<{ nom: string; montant: number; avertissement?: string | null }>,
    texte: (nom: string, m: number) => string,
  ) {
    setEnCours(true);
    setAvis(null);
    try {
      const r = await action();
      setAvis({ texte: texte(r.nom, r.montant) + (r.avertissement ? ` ${r.avertissement}` : "") });
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
    <div className="space-y-4">
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
              <MoneyInput value={montant} onChange={setMontant} aria-label="Montant de l'avance" className="h-9 w-32" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Donnée en
              <select value={moyen} onChange={e => setMoyen(e.target.value)} aria-label="Donnée en"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                {MOYENS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Le
              <Input type="date" value={duTiroir ? ilYA(0) : date} onChange={e => setDate(e.target.value)} aria-label="Date de l'avance"
                min={ilYA(31)} max={ilYA(0)} disabled={duTiroir} className="h-9 w-40" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Pour quoi (facultatif)
              <Input value={motif} onChange={e => setMotif(e.target.value)} aria-label="Motif de l'avance" className="h-9" /></label>
          </div>
          <div className="flex gap-4 flex-wrap items-center text-sm">
            <label className={`flex items-center gap-2 ${moyen === "especes" ? "" : "text-muted-foreground"}`}>
              <input type="checkbox" checked={duTiroir} disabled={moyen !== "especes"} onChange={e => setTiroir(e.target.checked)}
                aria-label="Pris dans le tiroir" /> Pris dans le tiroir de la caisse
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={etalee} onChange={e => setEtalee(e.target.checked)} aria-label="Étaler la retenue" />
              Retenir par morceaux
            </label>
            {etalee && (
              <label className="flex items-center gap-2 text-muted-foreground">
                <MoneyInput value={mensualite} onChange={setMensualite} aria-label="Retenue par paie" className="h-8 w-28" /> par paie
              </label>
            )}
            <Button size="sm" className="ml-auto" disabled={enCours || !qui || !montant.trim() || (etalee && !mensualite.trim())}
              onClick={() => faire(
                () => invoke("donner_avance", {
                  employeId: qui, montant: parseMontant(montant), moyen, motif: motif || null,
                  date: duTiroir ? null : date, duTiroir,
                  mensualite: etalee ? parseMontant(mensualite) : null,
                }),
                (nom, m) => `Avance de ${f(m)} à ${nom} notée${duTiroir ? ", sortie de la caisse" : ""}. `
                  + (etalee ? `Elle se retiendra par ${f(parseMontant(mensualite))} à chaque paie.` : "Elle se retiendra sur sa prochaine paie."),
              ).then(() => { setMontant(""); setMotif(""); setMensualite(""); setEtalee(false); setTiroir(false); setDate(ilYA(0)); })}>
              <HandCoins className="h-4 w-4 mr-1" /> Donner l'avance
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            {duTiroir
              ? "L'argent sort du tiroir maintenant : la caisse doit être ouverte, et la sortie s'y écrit avec l'avance."
              : "Hors caisse : noter seulement comment l'argent a été donné. Pris dans le tiroir ? Cocher la case, sinon il manquera à la clôture."}
          </p>
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
                  le {a.date_avance.slice(0, 10).split("-").reverse().join("/")} · {moyenDit(a)}
                  {a.du_tiroir ? " · pris dans le tiroir" : ""}
                  {a.mensualite ? ` · ${f(a.mensualite)} par paie` : ""}
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
                    (nom, m) => `Avance de ${f(m)} à ${nom} annulée : elle ne sera pas retenue.`
                      + (a.du_tiroir ? " L'argent revient dans le tiroir (caisse ouverte)." : ""),
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
  const [vue, setVue] = useState<"avances" | "fiches" | "cotisations" | "prets">("avances");
  // D47 : les prêts se lisent comme la paie.
  const voitLesPrets = peut("paie:preparer") || peut("paie:valider");
  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">Paie</h1>
      <div className="flex gap-1" role="tablist">
        {([["avances", "Avances"], ["fiches", "Fiches du mois"], ["cotisations", "Cotisations"],
          ...(voitLesPrets ? [["prets", "Prêts"] as const] : [])] as const).map(([cle, libelle]) => (
          <button key={cle} role="tab" aria-selected={vue === cle} onClick={() => setVue(cle)}
            className={`px-3 py-1.5 text-sm rounded-lg ${vue === cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
            {libelle}
          </button>
        ))}
      </div>
      {vue === "avances" ? <Avances /> : vue === "fiches" ? <FichesPaie /> : vue === "prets" ? <Prets /> : <Cotisations />}
    </div>
  );
}
