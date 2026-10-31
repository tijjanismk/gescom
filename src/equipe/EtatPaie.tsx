// equipe/EtatPaie.tsx — l'état de paie du mois (PLAN-EQUIPE, EQ-2 — D48).
//
// Une ligne par personne (ses fiches validées dont la période finit dans
// le mois), les totaux, puis ce qui n'est pas fini : les brouillons et
// les personnes sans fiche, hors des totaux. Lecture seule : on paie et
// on corrige dans « Fiches du mois ». Tout est compté par le serveur.
// Imprimer : l'aperçu est ce qui s'imprime (comme le bulletin) ; CSV :
// fabriqué par le serveur, même droit de lecture.

import { useState, useEffect, useRef } from "react";
import { Loader2, Printer, Download, X, AlertTriangle } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { chargerHabillage } from "@/lib/impression";
import { genererEtatPaieHTML, colonnesEtat, moisDit, type EtatPaie as Etat } from "@/lib/genererEtatPaie";

const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const jj = (iso: string) => iso.slice(0, 10).split("-").reverse().join("/");
// Le mois local, AAAA-MM (pas toISOString : il est en UTC).
const moisCourant = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`; };

function Apercu({ etat, fermer }: { etat: Etat; fermer: () => void }) {
  const [html, setHtml] = useState("");
  const cadre = useRef<HTMLIFrameElement>(null);
  useEffect(() => {
    let annule = false;
    chargerHabillage("bulletin")
      .catch(() => ({}))
      .then(h => { if (!annule) setHtml(genererEtatPaieHTML(etat, h)); });
    return () => { annule = true; };
  }, [etat]);

  return (
    <div className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center p-4" role="dialog" aria-label="État de paie à imprimer">
      <div className="bg-background rounded-lg shadow-xl flex flex-col" style={{ width: 860, maxWidth: "96vw", height: "90vh" }}>
        <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
          <h3 className="font-semibold flex-1">État de paie — {moisDit(etat.mois)}</h3>
          <Button size="sm" disabled={!html} onClick={() => cadre.current?.contentWindow?.print()}>
            <Printer className="h-4 w-4 mr-1" /> Imprimer
          </Button>
          <Button size="sm" variant="ghost" aria-label="Fermer l'aperçu" onClick={fermer}><X className="h-4 w-4" /></Button>
        </div>
        <div className="flex-1 overflow-auto bg-muted/40 p-4">
          {!html ? <Loader2 className="h-6 w-6 animate-spin text-muted-foreground mx-auto" /> : (
            <iframe ref={cadre} title="État de paie" srcDoc={html} sandbox="allow-same-origin allow-modals"
              style={{ width: 794, height: 1123, border: "none", background: "#fff", display: "block", margin: "0 auto", boxShadow: "0 1px 8px rgba(0,0,0,.15)" }} />
          )}
        </div>
      </div>
    </div>
  );
}

export function EtatPaie() {
  const [mois, setMois] = useState(moisCourant);
  const [etat, setEtat] = useState<Etat | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [apercu, setApercu] = useState(false);

  useEffect(() => {
    let annule = false;
    setEtat(null);
    setErreur(null);
    invoke<Etat>("lire_etat_paie", { mois })
      .then(e => { if (!annule) setEtat(e); })
      .catch(e => { if (!annule) setErreur(String(e)); });
    return () => { annule = true; };
  }, [mois]);

  async function exporter() {
    try {
      const r = await invoke<{ nom_fichier: string; contenu: string }>("exporter_etat_paie_csv", { mois });
      // BOM : Excel lit alors les accents comme de l'UTF-8.
      const blob = new Blob(["﻿" + r.contenu], { type: "text/csv;charset=utf-8" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = r.nom_fichier;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    } catch (e) {
      setErreur(String(e));
    }
  }

  const colonnes = etat ? colonnesEtat(etat.total) : [];
  const chiffre = (n: number) => n === 0 ? <span className="text-muted-foreground">—</span> : f(n);

  return (
    <div className="space-y-4" data-testid="etat-paie">
      <div className="flex gap-3 items-end flex-wrap">
        <label className="text-xs text-muted-foreground flex flex-col gap-1">Mois
          <Input type="month" value={mois} onChange={e => e.target.value && setMois(e.target.value)} aria-label="Mois de l'état" className="h-9 w-44" />
        </label>
        <div className="flex gap-2 ml-auto">
          <Button size="sm" variant="outline" disabled={!etat} onClick={() => setApercu(true)}>
            <Printer className="h-4 w-4 mr-1" /> Imprimer
          </Button>
          <Button size="sm" variant="outline" disabled={!etat} onClick={exporter}>
            <Download className="h-4 w-4 mr-1" /> Exporter CSV
          </Button>
        </div>
      </div>

      {erreur && <p className="text-sm text-red-700" role="status">{erreur}</p>}
      {!etat ? (!erreur && <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />) : (
        <>
          <div className="border border-border rounded-lg overflow-x-auto">
            <table className="w-full text-sm" data-testid="tableau-etat">
              <thead>
                <tr className="border-b border-border text-xs text-muted-foreground">
                  <th className="text-left font-medium px-3 py-2">Personne</th>
                  {colonnes.map(([c, t]) => <th key={c} className="text-right font-medium px-3 py-2 whitespace-nowrap">{t}</th>)}
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {etat.lignes.length === 0 && (
                  <tr><td colSpan={colonnes.length + 1} className="px-3 py-3 text-muted-foreground">Aucune fiche validée pour {moisDit(etat.mois)}.</td></tr>
                )}
                {etat.lignes.map(l => (
                  <tr key={l.employe_id} data-testid="ligne-etat">
                    <td className="px-3 py-2">
                      <div className="font-medium">{l.nom}</div>
                      <div className="text-xs text-muted-foreground">{l.fonction} · {l.numeros.join(", ")}</div>
                    </td>
                    {colonnes.map(([c]) => (
                      <td key={c} className={`px-3 py-2 text-right tabular-nums whitespace-nowrap ${c === "net" || c === "reste" ? "font-semibold" : ""}`}>
                        {chiffre(l[c])}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr className="border-t-2 border-foreground font-semibold" data-testid="total-etat">
                  <td className="px-3 py-2">Total</td>
                  {colonnes.map(([c]) => <td key={c} className="px-3 py-2 text-right tabular-nums whitespace-nowrap">{f(etat.total[c])}</td>)}
                </tr>
              </tfoot>
            </table>
          </div>

          <p className="text-sm">
            Coût du mois pour la boutique (brut{etat.total.charges > 0 ? " + charges patronales" : ""}) :{" "}
            <span className="font-semibold tabular-nums" data-testid="cout-etat">{f(etat.cout)}</span>
            <span className="text-muted-foreground"> — le chiffre de Rapports → CA mensuel.</span>
          </p>

          {(etat.brouillons.length > 0 || etat.sans_fiche.length > 0) && (
            <div className="bg-card border border-border border-l-4 border-l-amber-600 rounded-lg p-3 space-y-2 text-sm" data-testid="pas-fini">
              <div className="flex items-center gap-2 font-medium">
                <AlertTriangle className="h-4 w-4 text-amber-700" /> Pas encore dans les totaux
              </div>
              {etat.brouillons.length > 0 && (
                <div>
                  <div className="text-xs text-muted-foreground">À valider (dans « Fiches du mois »)</div>
                  <ul className="list-disc pl-5">
                    {etat.brouillons.map(b => (
                      <li key={b.id}>{b.nom} ({b.fonction}), du {jj(b.du)} au {jj(b.au)} : {f(b.net)} provisoire</li>
                    ))}
                  </ul>
                </div>
              )}
              {etat.sans_fiche.length > 0 && (
                <div>
                  <div className="text-xs text-muted-foreground">Employés ce mois, sans aucune fiche</div>
                  <ul className="list-disc pl-5">
                    {etat.sans_fiche.map(p => <li key={p.employe_id}>{p.nom} ({p.fonction})</li>)}
                  </ul>
                </div>
              )}
            </div>
          )}

          <p className="text-xs text-muted-foreground">
            Seules les fiches validées comptent ; une fiche compte dans le mois où finit sa période.
            Le « déjà versé » est celui d'aujourd'hui.
          </p>
        </>
      )}
      {apercu && etat && <Apercu etat={etat} fermer={() => setApercu(false)} />}
    </div>
  );
}
