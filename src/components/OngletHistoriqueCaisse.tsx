// components/OngletHistoriqueCaisse.tsx
//
// L'écart de clôture est la seule mesure qui confronte le logiciel au
// monde réel. Un écart isolé ne dit rien — c'est la SUITE des écarts
// qui parle. Cet écran existe pour lire cette suite.

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  History as HistoryIcon, Loader2, RefreshCw, ChevronDown, ChevronRight,
  TrendingDown, TrendingUp, CheckCircle2, AlertTriangle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

interface Session {
  id: string; statut: string;
  fond_ouverture: number; ouvert_le: string; ferme_le: string | null;
  solde_theorique: number | null; especes_comptees: number | null;
  ecart: number | null; ouvert_par: string;
  entrees_especes: number; sorties_especes: number; nb_mouvements: number;
}
interface Mouvement {
  id: string; sens: "entree" | "sortie"; moyen: string;
  montant: number; motif: string; libelle: string;
  categorie: string; date_mouvement: string;
}
interface RapportEcarts {
  jours: number; nb_clotures: number;
  nuls: number; manques: number; excedents: number;
  cumul: number; pire_manque: number; moyenne: number;
  diagnostic: string;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    weekday: "short", day: "2-digit", month: "2-digit",
  });
}
function fmtHeure(iso: string): string {
  return new Date(iso).toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}
function fmtMotif(m: string): string {
  return {
    vente: "Vente", achat: "Achat", depense: "Dépense",
    reglement_fournisseur: "Règlement fournisseur",
    retour_fournisseur: "Retour fournisseur",
    remboursement: "Remboursement",
    remboursement_reliquat: "Remboursement reliquat",
    complement_echange: "Complément d'échange",
    ouverture: "Fond d'ouverture",
  }[m] ?? m;
}
function fmtMoyen(m: string): string {
  return {
    especes: "Espèces", orange_money: "Orange Money",
    moov_money: "Moov Money", cheque: "Chèque",
  }[m] ?? m;
}

export function OngletHistoriqueCaisse() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [rapport, setRapport] = useState<RapportEcarts | null>(null);
  const [chargement, setChargement] = useState(true);
  const [ouverte, setOuverte] = useState<string | null>(null);
  const [mouvements, setMouvements] = useState<Mouvement[]>([]);
  const [periode, setPeriode] = useState(30);

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const [s, r] = await Promise.all([
        invoke<Session[]>("lire_sessions_caisse", { limite: 60 }),
        invoke<RapportEcarts>("lire_rapport_ecarts", { jours: periode }),
      ]);
      setSessions(s);
      setRapport(r);
    } catch (e) {
      console.error("Erreur historique caisse :", e);
    } finally {
      setChargement(false);
    }
  }, [periode]);

  useEffect(() => { charger(); }, [charger]);

  async function ouvrirSession(id: string) {
    if (ouverte === id) { setOuverte(null); return; }
    setOuverte(id);
    try {
      setMouvements(await invoke<Mouvement[]>("lire_mouvements_session",
        { sessionId: id }));
    } catch (e) {
      console.error(e);
      setMouvements([]);
    }
  }

  if (chargement && sessions.length === 0) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-8">
        <Loader2 className="h-4 w-4 animate-spin" /> Chargement…
      </div>
    );
  }

  return (
    <div className="space-y-6">

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <HistoryIcon className="h-5 w-5" /> Historique de caisse
          </h2>
          <p className="text-xs text-muted-foreground">
            L'écart confronte ce que dit le logiciel à ce qu'il y a dans
            le tiroir. C'est la seule mesure qui vienne du monde réel.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select value={periode} onChange={e => setPeriode(Number(e.target.value))}
            className="h-8 px-2 text-sm border border-border rounded-md bg-background">
            <option value={7}>7 jours</option>
            <option value={30}>30 jours</option>
            <option value={90}>90 jours</option>
          </select>
          <Button variant="outline" size="sm" onClick={charger}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
        </div>
      </div>

      {/* ── Diagnostic ── */}
      {rapport && rapport.nb_clotures > 0 && (
        <Card>
          <CardContent className="pt-4">
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
              <div>
                <p className="text-xs text-muted-foreground">Clôtures</p>
                <p className="text-xl font-bold">{rapport.nb_clotures}</p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground flex items-center gap-1">
                  <CheckCircle2 className="h-3 w-3 text-green-600" /> Justes
                </p>
                <p className="text-xl font-bold text-green-700">{rapport.nuls}</p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground flex items-center gap-1">
                  <TrendingDown className="h-3 w-3 text-red-600" /> Manques
                </p>
                <p className="text-xl font-bold text-red-600">{rapport.manques}</p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground flex items-center gap-1">
                  <TrendingUp className="h-3 w-3 text-orange-500" /> Excédents
                </p>
                <p className="text-xl font-bold text-orange-500">{rapport.excedents}</p>
              </div>
            </div>

            <div className={`flex gap-3 p-3 rounded-lg text-sm ${
              rapport.nuls === rapport.nb_clotures
                ? "bg-green-50 text-green-900"
                : "bg-orange-50 text-orange-900"
            }`}>
              {rapport.nuls === rapport.nb_clotures
                ? <CheckCircle2 className="h-4 w-4 shrink-0 mt-0.5" />
                : <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />}
              <p>{rapport.diagnostic}</p>
            </div>

            {rapport.cumul !== 0 && (
              <div className="flex flex-wrap gap-x-6 gap-y-1 mt-3 text-xs
                              text-muted-foreground">
                <span>Cumul sur la période : <strong className={
                  rapport.cumul < 0 ? "text-red-600" : "text-orange-500"
                }>{fmt(rapport.cumul)}</strong></span>
                <span>Moyenne par clôture : <strong>{fmt(rapport.moyenne)}</strong></span>
                {rapport.pire_manque < 0 && (
                  <span>Pire manque : <strong className="text-red-600">
                    {fmt(rapport.pire_manque)}</strong></span>
                )}
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* ── Sessions ── */}
      {sessions.length === 0 ? (
        <p className="text-sm text-muted-foreground py-6">
          Aucune session enregistrée.
        </p>
      ) : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {sessions.map(s => {
            const est = ouverte === s.id;
            const ec = s.ecart;
            return (
              <div key={s.id}>
                <button onClick={() => ouvrirSession(s.id)}
                  className="w-full flex flex-wrap items-center gap-3 px-4 py-3
                             hover:bg-muted/30 text-left">
                  {est ? <ChevronDown className="h-4 w-4 shrink-0" />
                       : <ChevronRight className="h-4 w-4 shrink-0" />}
                  <div className="flex-1 min-w-[150px]">
                    <p className="text-sm font-medium">
                      {fmtDate(s.ouvert_le)}
                      {s.statut === "ouverte" && (
                        <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded
                                         bg-green-100 text-green-700">en cours</span>
                      )}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {fmtHeure(s.ouvert_le)}
                      {s.ferme_le && ` → ${fmtHeure(s.ferme_le)}`}
                      {" · "}{s.ouvert_par} · {s.nb_mouvements} mouvement(s)
                    </p>
                  </div>
                  <div className="text-right min-w-[90px]">
                    <p className="text-[10px] text-muted-foreground">Fond</p>
                    <p className="text-sm">{fmt(s.fond_ouverture)}</p>
                  </div>
                  <div className="text-right min-w-[100px]">
                    <p className="text-[10px] text-muted-foreground">Théorique</p>
                    <p className="text-sm">
                      {s.solde_theorique !== null ? fmt(s.solde_theorique) : "—"}
                    </p>
                  </div>
                  <div className="text-right min-w-[100px]">
                    <p className="text-[10px] text-muted-foreground">Compté</p>
                    <p className="text-sm">
                      {s.especes_comptees !== null ? fmt(s.especes_comptees) : "—"}
                    </p>
                  </div>
                  <div className="text-right min-w-[100px]">
                    <p className="text-[10px] text-muted-foreground">Écart</p>
                    <p className={`text-sm font-semibold ${
                      ec === null ? "text-muted-foreground"
                      : ec === 0 ? "text-green-700"
                      : ec < 0 ? "text-red-600" : "text-orange-500"
                    }`}>
                      {ec === null ? "—" : ec === 0 ? "0 F" : fmt(ec)}
                    </p>
                  </div>
                </button>

                {est && (
                  <div className="border-t border-border bg-muted/20 px-4 py-2">
                    {mouvements.length === 0 ? (
                      <p className="text-xs text-muted-foreground py-2">
                        Aucun mouvement.
                      </p>
                    ) : (
                      <table className="w-full text-xs">
                        <tbody className="divide-y divide-border/60">
                          {mouvements.map(m => (
                            <tr key={m.id}>
                              <td className="py-1.5 w-14 text-muted-foreground">
                                {fmtHeure(m.date_mouvement)}
                              </td>
                              <td className="py-1.5">
                                {m.libelle || fmtMotif(m.motif)}
                                {m.libelle && (
                                  <span className="text-muted-foreground">
                                    {" "}· {fmtMotif(m.motif)}
                                  </span>
                                )}
                                {m.categorie && m.categorie !== "autre" && (
                                  <span className="text-muted-foreground">
                                    {" "}· {m.categorie}
                                  </span>
                                )}
                              </td>
                              <td className="py-1.5 w-24 text-muted-foreground">
                                {fmtMoyen(m.moyen)}
                              </td>
                              <td className={`py-1.5 w-28 text-right font-medium ${
                                m.sens === "entree" ? "text-green-700" : "text-red-600"
                              }`}>
                                {m.sens === "entree" ? "+" : "−"} {fmt(m.montant)}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                    <p className="text-[11px] text-muted-foreground py-2">
                      Seuls les mouvements en espèces entrent dans le
                      rapprochement. Le mobile money et les chèques sont
                      tracés mais hors tiroir.
                    </p>
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
