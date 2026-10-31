// equipe/Prets.tsx — le prêt au personnel sur plusieurs mois (EQ-1, D47).
//
// 60 000 F prêtés pour la Tabaski, retenus 10 000 F par mois sur la
// fiche de paie à partir du mois suivant. La retenue ne dépasse jamais
// le net ; une échéance manquée allonge le prêt, elle ne se double pas.
// Soldé, il se ferme. Au départ, la dernière fiche retient tout le
// reste ; ce qui ne passe pas se rembourse ou se remet. Prêter, régler,
// annuler : le patron (`paie:valider`) ; lire : qui lit la paie.

import { useState, useEffect, useCallback } from "react";
import { Loader2, HandCoins, Undo2, ChevronDown, ChevronRight } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";

interface Mouvement {
  date: string; genre: "paie" | "remboursement" | "remise"; montant: number;
  numero?: string | null; mois?: string; moyen?: string | null; motif?: string | null;
}

interface Pret {
  id: string; employe_id: string; nom: string; partie: boolean; montant: number; mensualite: number;
  retenir_des: string; date_pret: string; etat: "en_cours" | "solde" | "annule"; moyen: string;
  motif: string | null; du_tiroir: boolean; retenu_paie: number; dernier_mois_retenu: string | null;
  rembourse: number; remis: number; reste: number; fin_prevue: string | null; mouvements?: Mouvement[];
}

const jourIso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const ilYA = (n: number) => { const d = new Date(); d.setDate(d.getDate() - n); return jourIso(d); };
// Le mois qui suit un jour AAAA-MM-JJ, AAAA-MM.
const moisApres = (jour: string, n = 1) => {
  const [a, m] = jour.split("-").map(Number);
  const t = a * 12 + (m - 1) + n;
  return `${Math.floor(t / 12)}-${String((t % 12) + 1).padStart(2, "0")}`;
};
const MOIS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
const moisDit = (m: string | null) => {
  if (!m) return "";
  const [a, n] = m.split("-").map(Number);
  return `${MOIS[n - 1] ?? m} ${a}`;
};
const jourDit = (d: string) => d.slice(0, 10).split("-").reverse().join("/");
const MOYENS: [string, string][] = [["especes", "Espèces"], ["orange_money", "Orange Money"], ["moov_money", "Moov Money"], ["virement", "Virement"]];
const moyenDit = (m: string | null | undefined) => MOYENS.find(x => x[0] === m)?.[1] ?? m ?? "";
const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;

function Mouvements({ liste }: { liste: Mouvement[] }) {
  if (liste.length === 0) return <p className="text-xs text-muted-foreground">Rien de retenu ni de remboursé pour l'instant.</p>;
  return (
    <ul className="text-xs space-y-0.5" data-testid="mouvements-pret">
      {liste.map((m, i) => (
        <li key={i} className="flex gap-2">
          <span className="text-muted-foreground w-20">{jourDit(m.date)}</span>
          <span className="flex-1">
            {m.genre === "paie" ? `Retenu sur la paie de ${moisDit(m.mois ?? null)}${m.numero ? ` (${m.numero})` : ""}`
              : m.genre === "remboursement" ? `Remboursé — ${moyenDit(m.moyen)}${m.motif ? ` — ${m.motif}` : ""}`
              : `Reste remis${m.motif ? ` — ${m.motif}` : ""}`}
          </span>
          <span className="tabular-nums">{f(m.montant)}</span>
        </li>
      ))}
    </ul>
  );
}

export function Prets() {
  const [personnes, setPersonnes] = useState<{ id: string; nom: string; fonction: string; statut?: string }[]>([]);
  const [liste, setListe] = useState<Pret[] | null>(null);
  const [tous, setTous] = useState(false);
  const [qui, setQui] = useState("");
  const [montant, setMontant] = useState("");
  const [mensualite, setMensualite] = useState("");
  const [moyen, setMoyen] = useState("especes");
  const [motif, setMotif] = useState("");
  const [date, setDate] = useState(ilYA(0));
  const [premier, setPremier] = useState("");
  const [tiroir, setTiroir] = useState(false);
  const [ouvert, setOuvert] = useState<string | null>(null);
  const [detail, setDetail] = useState<Pret | null>(null);
  const [reglement, setReglement] = useState<{ id: string; genre: "remboursement" | "remise" } | null>(null);
  const [rembMontant, setRembMontant] = useState("");
  const [rembMoyen, setRembMoyen] = useState("especes");
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const prete = peut("paie:valider");
  const duTiroir = tiroir && moyen === "especes";
  const jourPret = duTiroir ? ilYA(0) : date;
  // La première retenue : le mois suivant, reculable de trois mois (D47).
  const premiers = [1, 2, 3, 4].map(n => moisApres(jourPret, n));
  const premierChoisi = premiers.includes(premier) ? premier : premiers[0];

  const charger = useCallback(async () => {
    try {
      setListe(await invoke<Pret[]>("lire_prets", { enCoursSeulement: !tous }));
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }, [tous]);
  useEffect(() => { charger(); }, [charger]);
  useEffect(() => {
    invoke<{ id: string; nom: string; fonction: string; statut?: string }[]>("lire_personnel")
      .then(l => setPersonnes(l.filter(p => p.statut !== "partie")))
      .catch(() => setPersonnes([]));
  }, []);

  async function relireDetail(id: string) {
    try {
      setDetail(await invoke<Pret>("lire_pret", { pretId: id }));
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }

  async function faire(action: () => Promise<Pret>, texte: (p: Pret) => string) {
    setEnCours(true);
    setAvis(null);
    try {
      const p = await action();
      setAvis({ texte: texte(p) });
      setReglement(null);
      await charger();
      if (ouvert === p.id) setDetail(p);
      return true;
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
      return false;
    } finally {
      setEnCours(false);
    }
  }

  const m = parseMontant(montant);
  const e = parseMontant(mensualite);
  const echeances = m > 0 && e > 0 && e < m ? Math.ceil(m / e) : 0;

  return (
    <div className="space-y-4">
      {prete && (
        <div className="border border-border rounded-lg p-4 space-y-3" data-testid="nouveau-pret">
          <h3 className="text-sm font-semibold">Prêter sur plusieurs mois</h3>
          <div className="flex gap-3 flex-wrap items-end">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">À qui
              <select value={qui} onChange={ev => setQui(ev.target.value)} aria-label="À qui"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background min-w-48">
                <option value="">—</option>
                {personnes.map(p => <option key={p.id} value={p.id}>{p.nom} ({p.fonction})</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Montant prêté
              <MoneyInput value={montant} onChange={setMontant} aria-label="Montant du prêt" className="h-9 w-32" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Retenu chaque mois
              <MoneyInput value={mensualite} onChange={setMensualite} aria-label="Retenue par mois" className="h-9 w-32" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Première retenue
              <select value={premierChoisi} onChange={ev => setPremier(ev.target.value)} aria-label="Première retenue"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                {premiers.map(p => <option key={p} value={p}>paie de {moisDit(p)}</option>)}
              </select></label>
          </div>
          <div className="flex gap-3 flex-wrap items-end">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Donné en
              <select value={moyen} onChange={ev => setMoyen(ev.target.value)} aria-label="Donné en"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                {MOYENS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Le
              <Input type="date" value={jourPret} onChange={ev => setDate(ev.target.value)} aria-label="Date du prêt"
                min={ilYA(31)} max={ilYA(0)} disabled={duTiroir} className="h-9 w-40" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Pour quoi (facultatif)
              <Input value={motif} onChange={ev => setMotif(ev.target.value)} aria-label="Motif du prêt" placeholder="Tabaski, rentrée…" className="h-9" /></label>
          </div>
          <div className="flex gap-4 flex-wrap items-center text-sm">
            <label className={`flex items-center gap-2 ${moyen === "especes" ? "" : "text-muted-foreground"}`}>
              <input type="checkbox" checked={duTiroir} disabled={moyen !== "especes"} onChange={ev => setTiroir(ev.target.checked)}
                aria-label="Pris dans le tiroir" /> Pris dans le tiroir de la caisse
            </label>
            {echeances > 0 && (
              <span className="text-muted-foreground" data-testid="echeancier">
                {echeances} échéances, de {moisDit(premierChoisi)} à {moisDit(moisApres(`${premierChoisi}-01`, echeances - 1))} si chaque paie le permet
              </span>
            )}
            <Button size="sm" className="ml-auto" disabled={enCours || !qui || !montant.trim() || !mensualite.trim()}
              onClick={() => faire(
                () => invoke<Pret>("preter", {
                  employeId: qui, montant: m, mensualite: e, moyen, motif: motif || null,
                  date: duTiroir ? null : date, premierMois: premierChoisi, duTiroir,
                }),
                p => `Prêt de ${f(p.montant)} à ${p.nom} noté${p.du_tiroir ? ", sorti de la caisse" : ""}. `
                  + `${f(p.mensualite)} seront retenus chaque mois à partir de la paie de ${moisDit(p.retenir_des)}.`,
              ).then(ok => { if (ok) { setMontant(""); setMensualite(""); setMotif(""); setTiroir(false); setDate(ilYA(0)); setPremier(""); } })}>
              <HandCoins className="h-4 w-4 mr-1" /> Prêter
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            Un seul prêt en cours par personne ; il ne compte pas dans le plafond d'avance. La retenue ne dépasse jamais
            le net du mois : une échéance manquée allonge le prêt, elle ne se double pas le mois suivant.
            {duTiroir ? " L'argent sort du tiroir maintenant : la caisse doit être ouverte." : " Hors caisse : noter seulement comment l'argent a été donné."}
          </p>
        </div>
      )}

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold">{tous ? "Tous les prêts" : "Prêts en cours"}</h3>
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          <input type="checkbox" checked={tous} onChange={ev => setTous(ev.target.checked)} /> Voir aussi les soldés et annulés
        </label>
      </div>
      {!liste ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-prets">
          {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun prêt en cours.</p>}
          {liste.map(p => {
            const deplie = ouvert === p.id;
            const rien = p.retenu_paie + p.rembourse + p.remis === 0;
            return (
              <div key={p.id} className="px-4 py-2 text-sm space-y-2" data-testid="pret">
                <div className="flex items-center gap-3">
                  <button aria-label={`${deplie ? "Replier" : "Déplier"} le prêt de ${p.nom}`} aria-expanded={deplie}
                    onClick={() => { if (deplie) { setOuvert(null); setDetail(null); } else { setOuvert(p.id); setDetail(null); relireDetail(p.id); } }}
                    className="text-muted-foreground hover:text-foreground">
                    {deplie ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                  </button>
                  <div className="flex-1">
                    <div className="font-medium">{p.nom} — {f(p.montant)}{p.motif ? ` (${p.motif})` : ""}</div>
                    <div className="text-xs text-muted-foreground">
                      le {jourDit(p.date_pret)} · {moyenDit(p.moyen)}{p.du_tiroir ? " · pris dans le tiroir" : ""}
                      {` · ${f(p.mensualite)} par mois depuis ${moisDit(p.retenir_des)}`}
                      {p.fin_prevue ? ` · fin prévue : ${moisDit(p.fin_prevue)}` : ""}
                    </div>
                  </div>
                  {p.partie && p.etat === "en_cours" && <Badge variant="outline" className="border-orange-600 text-orange-700">partie, reste dû</Badge>}
                  {p.etat === "annule" ? <Badge variant="outline">annulé</Badge>
                    : p.etat === "solde" ? <Badge variant="outline">soldé</Badge>
                    : <span className="tabular-nums font-semibold" title="Reste dû">{f(p.reste)}</span>}
                  {prete && p.etat === "en_cours" && rien && (
                    <Button size="sm" variant="ghost" aria-label={`Annuler le prêt de ${p.nom}`} disabled={enCours}
                      onClick={() => faire(
                        () => invoke<Pret>("annuler_pret", { pretId: p.id, motif: null }).then(() => invoke<Pret>("lire_pret", { pretId: p.id })),
                        x => `Prêt de ${f(x.montant)} à ${x.nom} annulé : il ne sera pas retenu.`
                          + (x.du_tiroir ? " L'argent revient dans le tiroir (caisse ouverte)." : ""),
                      )}>
                      <Undo2 className="h-4 w-4" />
                    </Button>
                  )}
                </div>
                {deplie && (
                  <div className="pl-7 space-y-2">
                    <div className="text-xs text-muted-foreground">
                      Retenu sur la paie : {f(p.retenu_paie)}
                      {p.rembourse > 0 ? ` · remboursé à part : ${f(p.rembourse)}` : ""}
                      {p.remis > 0 ? ` · remis : ${f(p.remis)}` : ""}
                    </div>
                    {detail?.id === p.id ? <Mouvements liste={detail.mouvements ?? []} /> : <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />}
                    {prete && p.etat === "en_cours" && (
                      reglement?.id === p.id ? (
                        <div className="flex gap-2 flex-wrap items-center" data-testid="reglement-pret">
                          {reglement.genre === "remboursement" ? (
                            <>
                              <MoneyInput value={rembMontant} onChange={setRembMontant} aria-label="Montant remboursé" className="h-8 w-28" />
                              <select value={rembMoyen} onChange={ev => setRembMoyen(ev.target.value)} aria-label="Remboursé en"
                                className="h-8 px-2 text-sm border border-border rounded-md bg-background">
                                {MOYENS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                              </select>
                              <Button size="sm" disabled={enCours || !rembMontant.trim()}
                                onClick={() => faire(
                                  () => invoke<Pret>("rembourser_pret", { pretId: p.id, montant: parseMontant(rembMontant), moyen: rembMoyen, motif: null }),
                                  x => `${f(parseMontant(rembMontant))} remboursés par ${x.nom}. ` + (x.reste > 0 ? `Il reste ${f(x.reste)}.` : "Le prêt est soldé."),
                                ).then(ok => { if (ok) setRembMontant(""); })}>
                                Noter le remboursement
                              </Button>
                            </>
                          ) : (
                            <>
                              <span>La société renonce aux {f(p.reste)} qui restent (charge au compte 658).</span>
                              <Button size="sm" variant="destructive" disabled={enCours}
                                onClick={() => faire(
                                  () => invoke<Pret>("remettre_pret", { pretId: p.id, motif: null }),
                                  x => `Le reste du prêt de ${x.nom} est remis : le prêt est fermé.`,
                                )}>
                                Confirmer la remise
                              </Button>
                            </>
                          )}
                          <Button size="sm" variant="ghost" onClick={() => setReglement(null)}>Laisser</Button>
                        </div>
                      ) : (
                        <div className="flex gap-2">
                          <Button size="sm" variant="outline" onClick={() => { setReglement({ id: p.id, genre: "remboursement" }); setRembMontant(""); }}>
                            Remboursement à part
                          </Button>
                          <Button size="sm" variant="outline" onClick={() => setReglement({ id: p.id, genre: "remise" })}>
                            Remettre le reste
                          </Button>
                        </div>
                      )
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
