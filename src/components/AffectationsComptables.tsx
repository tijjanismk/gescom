// components/AffectationsComptables.tsx — quelle opération va sur quel
// compte, pour le dossier ouvert (v3, E-2, D23).
//
// Des défauts livrés qui tiennent la route ; on ne change que ce qui
// diffère (une caisse sur 5711, un loyer sur 6221…). Chaque opération
// n'accepte que les comptes qui lui conviennent : le serveur juge, la
// liste ne propose que ceux-là.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Undo2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";

interface Affectation {
  operation: string;
  libelle: string;
  groupe: string;
  compte: string;
  libelle_compte: string | null;
  defaut: string;
  modifiee: boolean;
  prefixes: string[];
}

export function AffectationsComptables({ plan }: { plan: { numero: string; libelle: string }[] }) {
  const [liste, setListe] = useState<Affectation[] | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState<string | null>(null);

  const charger = useCallback(async () => {
    try {
      setListe(await invoke<Affectation[]>("lire_affectations"));
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  async function poser(a: Affectation, compte: string | null) {
    setEnCours(a.operation);
    setAvis(null);
    try {
      const r = await invoke<{ compte: string }>("definir_affectation", { operation: a.operation, compte });
      setAvis({ texte: `${a.libelle} → ${r.compte}${compte === null ? " (par défaut)" : ""}.` });
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(null);
    }
  }

  if (!liste) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;
  const groupes = [...new Set(liste.map(a => a.groupe))];

  return (
    <div className="border border-border rounded-lg p-4 space-y-3" data-testid="affectations">
      <div>
        <h3 className="text-sm font-semibold">Affectations</h3>
        <p className="text-xs text-muted-foreground mt-1">
          Le compte de chaque opération, pour ce dossier. Les journaux remis au comptable s'en servent.
        </p>
      </div>
      {groupes.map(g => (
        <div key={g}>
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-1">{g}</p>
          <div className="divide-y divide-border border border-border rounded-md">
            {liste.filter(a => a.groupe === g).map(a => {
              const possibles = plan.filter(c => a.prefixes.some(p => c.numero.startsWith(p)));
              return (
                <div key={a.operation} className="flex items-center gap-3 px-3 py-1.5 text-sm" data-testid="affectation">
                  <span className="flex-1">{a.libelle}</span>
                  <select aria-label={`Compte : ${a.libelle}`} value={a.compte} disabled={enCours !== null}
                    onChange={e => poser(a, e.target.value)}
                    className="h-8 px-2 text-sm border border-border rounded-md bg-background max-w-64">
                    {possibles.map(c => <option key={c.numero} value={c.numero}>{c.numero} — {c.libelle}</option>)}
                  </select>
                  {a.modifiee ? (
                    <Button size="sm" variant="ghost" title={`Revenir au défaut (${a.defaut})`}
                      aria-label={`Défaut : ${a.libelle}`} disabled={enCours !== null} onClick={() => poser(a, null)}>
                      <Undo2 className="h-3.5 w-3.5" />
                    </Button>
                  ) : <span className="w-9 text-xs text-muted-foreground text-center">défaut</span>}
                </div>
              );
            })}
          </div>
        </div>
      ))}
      {avis && (
        <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>
      )}
    </div>
  );
}
