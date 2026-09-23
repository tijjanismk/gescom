// components/ExercicesDossier.tsx — les dates de travail du dossier
// ouvert ici (v3, D-4 — D21).
//
// Un exercice = des dates de travail. Une vente, une pièce, un
// règlement daté hors d'un exercice ouvert est refusé par le serveur,
// qui dit quoi faire : prolonger, ou ouvrir le suivant. Cet écran est
// l'endroit où on le fait. Pas de trou : le suivant commence le
// lendemain du précédent. Un exercice clos ne se rouvre pas.

import { useState, useEffect, useCallback } from "react";
import { appeler as invoke, dossierCourant } from "@/lib/pont";
import { Loader2, CalendarPlus, CalendarClock, Lock } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { jourEnLettres, lendemain } from "@/lib/utils";

interface Exercice {
  id: string;
  date_debut: string;
  date_fin: string;
  fin_effective: string;
  prolonge_jusqu_au: string | null;
  clos: boolean;
}

const champDate = "h-9 px-2 text-sm border border-border rounded-md bg-background";

function unAnMoinsUnJour(debut: string): string {
  const d = new Date(debut + "T12:00:00");
  d.setFullYear(d.getFullYear() + 1);
  d.setDate(d.getDate() - 1);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function ExercicesDossier() {
  const [exercices, setExercices] = useState<Exercice[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  // L'exercice dont on saisit la prolongation, et jusqu'à quand.
  const [prolonge, setProlonge] = useState<{ id: string; jusquau: string } | null>(null);
  // L'exercice dont on confirme la clôture.
  const [aClore, setAClore] = useState<string | null>(null);
  const [suivantAu, setSuivantAu] = useState("");
  const aujourdhui = new Date().toISOString().slice(0, 10);
  const dossier = dossierCourant();

  const charger = useCallback(async () => {
    try {
      const ex = await invoke<Exercice[]>("lire_exercices");
      setExercices(ex);
      const dernier = ex[ex.length - 1];
      if (dernier) setSuivantAu(unAnMoinsUnJour(lendemain(dernier.fin_effective)));
    } catch (e) {
      setErreur(String(e));
    }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  async function agir(commande: string, params: Record<string, unknown>, reussi: string) {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke(commande, params);
      setAvis({ texte: reussi });
      setProlonge(null);
      setAClore(null);
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  if (erreur) return <p className="text-sm text-red-600">{erreur}</p>;
  if (!exercices) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  const dernier = exercices[exercices.length - 1];
  const debutSuivant = dernier ? lendemain(dernier.fin_effective) : aujourdhui;

  return (
    <div className="border border-border rounded-lg p-4 space-y-3" data-testid="exercices">
      <div>
        <h3 className="text-sm font-semibold">
          Dates de travail {dossier ? <>de « {dossier.societe} »</> : "du dossier ouvert"}
        </h3>
        <p className="text-xs text-muted-foreground mt-1">
          Une écriture hors d'un exercice ouvert est refusée. Pas encore fini
          les papiers ? On prolonge. L'année terminée, le comptable clôt ; on
          n'y écrit plus, et l'exercice suivant commence le lendemain.
        </p>
      </div>

      <div className="divide-y divide-border border border-border rounded-md">
        {exercices.map(e => {
          const couvre = e.date_debut <= aujourdhui && aujourdhui <= e.fin_effective;
          return (
            <div key={e.id} className="px-3 py-2.5 space-y-2" data-testid="exercice">
              <div className="flex items-center justify-between gap-2 flex-wrap">
                <div className="text-sm">
                  Du <strong>{jourEnLettres(e.date_debut)}</strong> au <strong>{jourEnLettres(e.fin_effective)}</strong>
                  {e.prolonge_jusqu_au && e.prolonge_jusqu_au > e.date_fin && (
                    <span className="text-xs text-muted-foreground"> — prolongé (fin prévue le {jourEnLettres(e.date_fin)})</span>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  {couvre && !e.clos && <Badge variant="secondary">aujourd'hui</Badge>}
                  {e.clos
                    ? <Badge variant="outline"><Lock className="h-3 w-3 mr-1" />clos</Badge>
                    : <Badge className="bg-emerald-600 hover:bg-emerald-600">ouvert</Badge>}
                  {!e.clos && (
                    <>
                      <Button size="sm" variant="outline" disabled={enCours}
                        onClick={() => { setAClore(null); setProlonge({ id: e.id, jusquau: lendemain(e.fin_effective, 31) }); }}>
                        <CalendarClock className="h-4 w-4 mr-1" /> Prolonger
                      </Button>
                      <Button size="sm" variant="outline" disabled={enCours}
                        onClick={() => { setProlonge(null); setAClore(e.id); }}>
                        Clore
                      </Button>
                    </>
                  )}
                </div>
              </div>
              {prolonge?.id === e.id && (
                <div className="flex items-center gap-2 flex-wrap text-sm">
                  <span className="text-muted-foreground">Prolonger jusqu'au</span>
                  <input type="date" aria-label="Prolonger jusqu'au" value={prolonge.jusquau}
                    onChange={ev => setProlonge({ id: e.id, jusquau: ev.target.value })} className={champDate} />
                  <Button size="sm" disabled={enCours || !prolonge.jusquau}
                    onClick={() => agir("prolonger_exercice", { exerciceId: e.id, prolongeJusquAu: prolonge.jusquau },
                      `Prolongé jusqu'au ${jourEnLettres(prolonge.jusquau)}.`)}>
                    Valider
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setProlonge(null)}>Annuler</Button>
                </div>
              )}
              {aClore === e.id && (
                <div className="flex items-center gap-2 flex-wrap text-sm bg-amber-50 dark:bg-amber-950/30 rounded px-2 py-1.5">
                  <span>
                    Clore du {jourEnLettres(e.date_debut)} au {jourEnLettres(e.fin_effective)} ? On n'y écrira plus, et
                    ça ne se défait pas.
                  </span>
                  <Button size="sm" variant="destructive" disabled={enCours}
                    onClick={() => agir("clore_exercice", { exerciceId: e.id },
                      `Exercice clos. Du ${jourEnLettres(e.date_debut)} au ${jourEnLettres(e.fin_effective)}, on n'écrit plus.`)}>
                    Confirmer la clôture
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setAClore(null)}>Annuler</Button>
                </div>
              )}
            </div>
          );
        })}
        {exercices.length === 0 && (
          <p className="px-3 py-2.5 text-sm text-muted-foreground">Aucun exercice : rien ne peut s'écrire.</p>
        )}
      </div>

      <div className="flex items-end gap-2 flex-wrap">
        <div className="text-sm">
          <span className="text-muted-foreground">Exercice suivant : du </span>
          <strong data-testid="debut-suivant">{jourEnLettres(debutSuivant)}</strong>
          <span className="text-muted-foreground"> au </span>
        </div>
        <input type="date" aria-label="Fin de l'exercice suivant" value={suivantAu}
          onChange={e => setSuivantAu(e.target.value)} className={champDate} />
        <Button size="sm" disabled={enCours || !suivantAu}
          onClick={() => agir("ouvrir_exercice", { dateDebut: debutSuivant, dateFin: suivantAu },
            `Exercice ouvert du ${jourEnLettres(debutSuivant)} au ${jourEnLettres(suivantAu)}.`)}>
          <CalendarPlus className="h-4 w-4 mr-1" /> Ouvrir l'exercice suivant
        </Button>
      </div>

      {avis && (
        <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>
      )}
    </div>
  );
}
