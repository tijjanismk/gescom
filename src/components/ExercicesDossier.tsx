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
import { Loader2, CalendarPlus, CalendarClock, Lock, Scissors } from "lucide-react";
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

/** Le même jour, `ans` années plus tard, en `AAAA-MM-JJ`. */
function plusAns(iso: string, ans: number): string {
  const d = new Date(iso.slice(0, 10) + "T12:00:00");
  d.setFullYear(d.getFullYear() + ans);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** La veille du même jour `ans` années plus tard : un exercice de
 *  `ans` années pleines qui commence à `debut`. */
function ansMoinsUnJour(debut: string, ans = 1): string {
  return lendemain(plusAns(debut, ans), -1);
}

/** « 3 ans », « 1 an et 3 mois », « 12 jours » : ce qui sépare deux
 *  jours, dit comme on le dit. C'est ce qui fait voir une faute
 *  d'année avant de valider — une prolongation ne se raccourcit pas. */
function ecartEnClair(de: string, a: string): string {
  const d1 = new Date(de.slice(0, 10) + "T12:00:00");
  const d2 = new Date(a.slice(0, 10) + "T12:00:00");
  if (isNaN(d1.getTime()) || isNaN(d2.getTime()) || d2 <= d1) return "";
  let mois = (d2.getFullYear() - d1.getFullYear()) * 12 + (d2.getMonth() - d1.getMonth());
  if (d2.getDate() < d1.getDate()) mois -= 1;
  if (mois < 1) {
    const j = Math.round((d2.getTime() - d1.getTime()) / 86_400_000);
    return `${j} jour${j > 1 ? "s" : ""}`;
  }
  const ans = Math.floor(mois / 12);
  const reste = mois % 12;
  return [ans ? `${ans} an${ans > 1 ? "s" : ""}` : "", reste ? `${reste} mois` : ""]
    .filter(Boolean).join(" et ");
}

/** Pourquoi cet exercice ne se clôt pas encore, ou `null` (D38). Le
 *  serveur juge (`juger_cloture`) ; l'écran le dit avant de demander
 *  confirmation. */
function raisonDeNePasClore(e: Exercice, tous: Exercice[], aujourdhui: string): string | null {
  if (e.fin_effective >= aujourdhui) {
    return `On ne clôt qu'un exercice terminé : celui-ci va jusqu'au ${jourEnLettres(e.fin_effective)}. `
      + "Pour l'arrêter avant, le couper, puis clore la partie terminée.";
  }
  const avant = tous.find(x => !x.clos && x.date_debut < e.date_debut);
  return avant
    ? `Clore d'abord l'exercice du ${jourEnLettres(avant.date_debut)} au ${jourEnLettres(avant.fin_effective)} : on clôt dans l'ordre des dates.`
    : null;
}

/** Les durées qu'on choisit d'un clic ; le champ date reste libre
 *  (le serveur refuse au-delà de cinq ans). */
const DUREES_RAPIDES = [1, 2, 3];

export function ExercicesDossier() {
  const [exercices, setExercices] = useState<Exercice[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  // L'exercice dont on saisit la prolongation, et jusqu'à quand.
  const [prolonge, setProlonge] = useState<{ id: string; jusquau: string } | null>(null);
  // L'exercice dont on confirme la clôture.
  const [aClore, setAClore] = useState<string | null>(null);
  // Le dernier exercice qu'on coupe (D38) : jusqu'au jour `au`, et le
  // suivant, ouvert le lendemain, jusqu'à `suivantAu`.
  const [coupe, setCoupe] = useState<{ id: string; au: string; suivantAu: string } | null>(null);
  const [suivantAu, setSuivantAu] = useState("");
  const aujourdhui = new Date().toISOString().slice(0, 10);
  const dossier = dossierCourant();

  const charger = useCallback(async () => {
    try {
      const ex = await invoke<Exercice[]>("lire_exercices");
      setExercices(ex);
      const dernier = ex[ex.length - 1];
      if (dernier) setSuivantAu(ansMoinsUnJour(lendemain(dernier.fin_effective)));
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
      setCoupe(null);
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
          les papiers ? On prolonge — d'un mois ou de plusieurs années, cinq
          au plus. L'année terminée, le comptable clôt ; on n'y écrit plus, et
          l'exercice suivant commence le lendemain.
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
                        onClick={() => { setAClore(null); setCoupe(null); setProlonge({ id: e.id, jusquau: lendemain(e.fin_effective, 31) }); }}>
                        <CalendarClock className="h-4 w-4 mr-1" /> Prolonger
                      </Button>
                      {/* D38 : seul le dernier se coupe — avec un suivant,
                          couper ferait un trou ou un chevauchement. */}
                      {e.id === dernier?.id && (
                        <Button size="sm" variant="outline" disabled={enCours} data-testid="couper-exercice"
                          onClick={() => { setAClore(null); setProlonge(null); setCoupe({ id: e.id, au: "", suivantAu: "" }); }}>
                          <Scissors className="h-4 w-4 mr-1" /> Couper
                        </Button>
                      )}
                      <Button size="sm" variant="outline" disabled={enCours}
                        onClick={() => {
                          setProlonge(null); setCoupe(null);
                          // D38 : le serveur refuserait ; on le dit tout de
                          // suite plutôt qu'après la confirmation.
                          const pasClore = raisonDeNePasClore(e, exercices, aujourdhui);
                          if (pasClore) { setAClore(null); setAvis({ texte: pasClore, erreur: true }); }
                          else { setAvis(null); setAClore(e.id); }
                        }}>
                        Clore
                      </Button>
                    </>
                  )}
                </div>
              </div>
              {prolonge?.id === e.id && (
                <div className="space-y-1.5 text-sm">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-muted-foreground">Prolonger jusqu'au</span>
                    <input type="date" aria-label="Prolonger jusqu'au" value={prolonge.jusquau}
                      onChange={ev => setProlonge({ id: e.id, jusquau: ev.target.value })} className={champDate} />
                    {DUREES_RAPIDES.map(n => (
                      <Button key={n} size="sm" variant="outline" disabled={enCours}
                        data-testid={`prolonger-${n}-an`}
                        onClick={() => setProlonge({ id: e.id, jusquau: plusAns(e.fin_effective, n) })}>
                        +{n} an{n > 1 ? "s" : ""}
                      </Button>
                    ))}
                    <Button size="sm" disabled={enCours || !prolonge.jusquau}
                      onClick={() => agir("prolonger_exercice", { exerciceId: e.id, prolongeJusquAu: prolonge.jusquau },
                        `Prolongé jusqu'au ${jourEnLettres(prolonge.jusquau)}.`)}>
                      Valider
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setProlonge(null)}>Annuler</Button>
                  </div>
                  {ecartEnClair(e.fin_effective, prolonge.jusquau) && (
                    <p className="text-xs text-muted-foreground" data-testid="prolongation-en-clair">
                      Soit {ecartEnClair(e.fin_effective, prolonge.jusquau)} de plus.
                      {prolonge.jusquau > plusAns(e.fin_effective, 1) && (
                        <span className="text-amber-700"> Une prolongation ne se raccourcit pas : vérifier l'année.</span>
                      )}
                    </p>
                  )}
                </div>
              )}
              {coupe?.id === e.id && (
                <div className="space-y-1.5 text-sm bg-muted/40 rounded px-2 py-1.5">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-muted-foreground">Arrêter cet exercice au</span>
                    <input type="date" aria-label="Couper l'exercice au" value={coupe.au}
                      min={e.date_debut} max={lendemain(e.fin_effective, -1)}
                      onChange={ev => {
                        const au = ev.target.value;
                        // Un an après la coupe, ou l'ancienne fin si elle va
                        // plus loin : ce qui s'écrivait doit s'écrire encore.
                        const unAn = au ? plusAns(au, 1) : "";
                        setCoupe({ id: e.id, au, suivantAu: unAn && unAn < e.fin_effective ? e.fin_effective : unAn });
                      }}
                      className={champDate} />
                    <span className="text-muted-foreground">; le suivant va jusqu'au</span>
                    <input type="date" aria-label="Fin de l'exercice après la coupe" value={coupe.suivantAu}
                      min={e.fin_effective}
                      onChange={ev => setCoupe({ ...coupe, suivantAu: ev.target.value })} className={champDate} />
                  </div>
                  {coupe.au && coupe.suivantAu && (
                    <div className="flex items-center gap-2 flex-wrap">
                      <span data-testid="coupe-en-clair">
                        Du {jourEnLettres(e.date_debut)} au {jourEnLettres(coupe.au)}, puis
                        du {jourEnLettres(lendemain(coupe.au))} au {jourEnLettres(coupe.suivantAu)}.
                      </span>
                      <Button size="sm" disabled={enCours}
                        onClick={() => agir("couper_exercice",
                          { exerciceId: e.id, coupeAu: coupe.au, suivantAu: coupe.suivantAu },
                          `Exercice coupé au ${jourEnLettres(coupe.au)} ; le suivant va du ${jourEnLettres(lendemain(coupe.au))} au ${jourEnLettres(coupe.suivantAu)}.`)}>
                        Couper
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => setCoupe(null)}>Annuler</Button>
                    </div>
                  )}
                  {!(coupe.au && coupe.suivantAu) && (
                    <Button size="sm" variant="ghost" onClick={() => setCoupe(null)}>Annuler</Button>
                  )}
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
        {DUREES_RAPIDES.map(n => (
          <Button key={n} size="sm" variant="outline" disabled={enCours}
            data-testid={`suivant-${n}-an`}
            onClick={() => setSuivantAu(ansMoinsUnJour(debutSuivant, n))}>
            {n} an{n > 1 ? "s" : ""}
          </Button>
        ))}
        {ecartEnClair(debutSuivant, lendemain(suivantAu)) && (
          <span className="text-xs text-muted-foreground self-center" data-testid="suivant-en-clair">
            soit {ecartEnClair(debutSuivant, lendemain(suivantAu))}
          </span>
        )}
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
