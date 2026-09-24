// components/JournauxComptables.tsx — les journaux pour le comptable
// (v3, E-3, D23). Fabriqués à la lecture depuis les ventes, achats,
// paiements et la caisse, avec les comptes de l'affectation ; rien n'est
// stocké. L'export CSV (point-virgule, Excel) va dans son logiciel.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Download, CheckCircle2, AlertTriangle } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Ligne { compte: string; debit: number; credit: number }
interface Ecriture { journal: string; date: string; piece: string; libelle: string; lignes: Ligne[] }
interface Journal { code: string; libelle: string; ecritures: Ecriture[]; total_debit: number; total_credit: number }

const f = (n: number) => (n ? n.toLocaleString("fr-FR") : "");

function debutMois(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-01`;
}

export function JournauxComptables() {
  const [du, setDu] = useState(debutMois());
  const [au, setAu] = useState(new Date().toISOString().slice(0, 10));
  const [journal, setJournal] = useState("");
  const [donnees, setDonnees] = useState<{ journaux: Journal[]; comptes: Record<string, string | null> } | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [chargement, setChargement] = useState(false);

  const charger = useCallback(async () => {
    setChargement(true);
    setErreur(null);
    try {
      setDonnees(await invoke("lire_journaux_comptables", { du, au, journal: journal || null }));
    } catch (e) {
      setErreur(String(e));
      setDonnees(null);
    } finally {
      setChargement(false);
    }
  }, [du, au, journal]);
  useEffect(() => { charger(); }, [charger]);

  async function exporter() {
    try {
      const r = await invoke<{ nom_fichier: string; contenu: string }>("exporter_journaux_csv", { du, au, journal: journal || null });
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

  return (
    <div className="space-y-4">
      <div className="flex items-end gap-3 flex-wrap">
        <label className="text-xs text-muted-foreground flex flex-col">
          Du
          <Input type="date" value={du} onChange={e => setDu(e.target.value)} aria-label="Début de la période" className="h-8 w-36 mt-1" />
        </label>
        <label className="text-xs text-muted-foreground flex flex-col">
          Au
          <Input type="date" value={au} onChange={e => setAu(e.target.value)} aria-label="Fin de la période" className="h-8 w-36 mt-1" />
        </label>
        <label className="text-xs text-muted-foreground flex flex-col">
          Journal
          <select value={journal} onChange={e => setJournal(e.target.value)} aria-label="Journal"
            className="h-8 px-2 mt-1 text-sm border border-border rounded-md bg-background">
            <option value="">Tous</option>
            <option value="VT">Ventes</option>
            <option value="AC">Achats</option>
            <option value="RG">Règlements</option>
            <option value="CA">Caisse</option>
          </select>
        </label>
        <Button size="sm" variant="outline" onClick={exporter} disabled={!donnees}>
          <Download className="h-4 w-4 mr-1" /> Exporter CSV
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Les écritures se fabriquent à la lecture, avec les comptes de Paramètres → Comptabilité. Rien n'est
        enregistré à côté des ventes : les journaux disent toujours ce que dit la boutique.
      </p>

      {erreur && <p className="text-sm text-red-600" role="alert">{erreur}</p>}
      {chargement && <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />}

      {donnees?.journaux.map(j => {
        const equilibre = j.total_debit === j.total_credit;
        return (
          <div key={j.code} className="border border-border rounded-lg overflow-hidden" data-testid={`journal-${j.code}`}>
            <div className="flex items-center justify-between px-4 py-2 bg-muted/50">
              <span className="text-sm font-semibold">{j.code} — {j.libelle}</span>
              <span className={`text-xs flex items-center gap-1 ${equilibre ? "text-emerald-700" : "text-red-600"}`}>
                {equilibre ? <CheckCircle2 className="h-3.5 w-3.5" /> : <AlertTriangle className="h-3.5 w-3.5" />}
                {j.ecritures.length} écriture{j.ecritures.length > 1 ? "s" : ""} · débit {f(j.total_debit) || 0} · crédit {f(j.total_credit) || 0}
                {equilibre ? " · équilibré" : " · DÉSÉQUILIBRÉ"}
              </span>
            </div>
            {j.ecritures.length > 0 && (
              <table className="w-full text-sm">
                <thead className="text-xs text-muted-foreground">
                  <tr className="border-b border-border">
                    <th className="text-left px-3 py-1.5 font-medium">Date</th>
                    <th className="text-left px-3 py-1.5 font-medium">Pièce</th>
                    <th className="text-left px-3 py-1.5 font-medium">Compte</th>
                    <th className="text-left px-3 py-1.5 font-medium">Libellé</th>
                    <th className="text-right px-3 py-1.5 font-medium">Débit</th>
                    <th className="text-right px-3 py-1.5 font-medium">Crédit</th>
                  </tr>
                </thead>
                <tbody>
                  {j.ecritures.map((e, i) => e.lignes.map((l, k) => (
                    <tr key={`${i}-${k}`} className={k === e.lignes.length - 1 ? "border-b border-border" : ""}>
                      <td className="px-3 py-1 text-xs text-muted-foreground">{k === 0 ? e.date.split("-").reverse().join("/") : ""}</td>
                      <td className="px-3 py-1 text-xs font-mono">{k === 0 ? e.piece : ""}</td>
                      <td className="px-3 py-1 font-mono" title={donnees.comptes[l.compte] ?? ""}>{l.compte}</td>
                      <td className="px-3 py-1">{k === 0 ? e.libelle : <span className="text-muted-foreground text-xs">{donnees.comptes[l.compte] ?? ""}</span>}</td>
                      <td className="px-3 py-1 text-right tabular-nums">{f(l.debit)}</td>
                      <td className="px-3 py-1 text-right tabular-nums">{f(l.credit)}</td>
                    </tr>
                  )))}
                </tbody>
              </table>
            )}
          </div>
        );
      })}
    </div>
  );
}
