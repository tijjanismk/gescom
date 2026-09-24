// equipe/JoursTravailles.tsx — la grille du mois (PLAN-EQUIPE, F-3).
//
// Une case par personne et par jour : vide (pas su), P (présent),
// ½ (demi-journée), A (absent). Un clic fait tourner la case. Le numéro
// du jour marque « tout le monde présent » sans toucher aux absences.
// Le compte de la ligne est ce que la paie d'un journalier lira.

import { useState, useEffect, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";

interface Grille {
  mois: string;
  jours: string[];
  aujourd_hui: string;
  personnes: {
    employe_id: string; nom: string; fonction: string;
    date_entree: string | null; date_depart: string | null;
    etats: Record<string, string>; jours_travailles: number;
  }[];
}

const SUIVANT: Record<string, string | null> = { "": "present", present: "demi", demi: "absent", absent: null };
const MARQUE: Record<string, string> = { present: "P", demi: "½", absent: "A" };
const COULEUR: Record<string, string> = {
  present: "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200",
  demi: "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200",
  absent: "bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-200",
};

export function JoursTravailles() {
  const [mois, setMois] = useState(new Date().toISOString().slice(0, 7));
  const [g, setG] = useState<Grille | null>(null);
  const [avis, setAvis] = useState<string | null>(null);
  const [enCours, setEnCours] = useState(false);
  const gere = peut("personnel:gerer");

  const charger = useCallback(async () => {
    try {
      setG(await invoke<Grille>("lire_presences_mois", { mois }));
    } catch (e) {
      setAvis(String(e));
    }
  }, [mois]);
  useEffect(() => { charger(); }, [charger]);

  async function faire(action: () => Promise<unknown>) {
    setEnCours(true);
    setAvis(null);
    try {
      await action();
      await charger();
    } catch (e) {
      setAvis(String(e));
    } finally {
      setEnCours(false);
    }
  }

  if (!g) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  const hors = (p: Grille["personnes"][number], j: string) =>
    j > g.aujourd_hui || (p.date_entree !== null && j < p.date_entree) || (p.date_depart !== null && j > p.date_depart);
  const weekend = (j: string) => [0, 6].includes(new Date(j + "T12:00:00").getDay());

  return (
    <div className="space-y-3" data-testid="jours-travailles">
      <div className="flex items-center gap-3">
        <input type="month" value={mois} onChange={e => setMois(e.target.value)} aria-label="Mois"
          className="h-8 px-2 text-sm border border-border rounded-md bg-background" />
        {enCours && <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />}
        <span className="text-xs text-muted-foreground">
          P présent · ½ demi-journée · A absent · vide : pas su.{gere ? " Un clic sur le numéro du jour : tout le monde présent." : ""}
        </span>
      </div>
      {avis && <p className="text-sm text-red-600" role="status">{avis}</p>}
      {g.personnes.length === 0 ? (
        <p className="text-sm text-muted-foreground">Personne dans ce mois.</p>
      ) : (
        <div className="overflow-x-auto border border-border rounded-lg">
          <table className="text-xs border-collapse">
            <thead>
              <tr>
                <th className="sticky left-0 bg-background text-left px-3 py-1.5 font-medium min-w-40">Personne</th>
                {g.jours.map(j => (
                  <th key={j} className={`px-0.5 py-1.5 font-normal ${weekend(j) ? "text-muted-foreground" : ""}`}>
                    {gere && j <= g.aujourd_hui ? (
                      <button className="w-6 hover:underline" disabled={enCours} aria-label={`Tous présents le ${j}`}
                        onClick={() => faire(() => invoke("marquer_tous_presents", { jour: j }))}>
                        {Number(j.slice(8))}
                      </button>
                    ) : <span className="inline-block w-6">{Number(j.slice(8))}</span>}
                  </th>
                ))}
                <th className="px-3 py-1.5 font-medium">Jours</th>
              </tr>
            </thead>
            <tbody>
              {g.personnes.map(p => (
                <tr key={p.employe_id} className="border-t border-border" data-testid="ligne-jours">
                  <td className="sticky left-0 bg-background px-3 py-1">
                    <div className="font-medium text-sm">{p.nom}</div>
                    <div className="text-muted-foreground">{p.fonction}</div>
                  </td>
                  {g.jours.map(j => {
                    const etat = p.etats[j] ?? "";
                    const interdit = hors(p, j);
                    return (
                      <td key={j} className={`px-0.5 py-1 text-center ${weekend(j) ? "bg-muted/40" : ""}`}>
                        <button
                          disabled={!gere || interdit || enCours}
                          aria-label={`${p.nom} le ${j}`}
                          title={interdit ? "Hors de ses dates" : undefined}
                          onClick={() => faire(() => invoke("marquer_presence", { employeId: p.employe_id, jour: j, etat: SUIVANT[etat] }))}
                          className={`w-6 h-6 rounded text-[11px] font-semibold ${etat ? COULEUR[etat] : "border border-border"} ${interdit ? "opacity-30" : ""}`}>
                          {MARQUE[etat] ?? ""}
                        </button>
                      </td>
                    );
                  })}
                  <td className="px-3 py-1 text-right text-sm font-semibold tabular-nums" data-testid="total-jours">
                    {p.jours_travailles.toLocaleString("fr-FR")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
