// equipe/Cotisations.tsx — les cotisations du dossier (PLAN-EQUIPE, G-4 — D33).
//
// Facultatives et jamais devinées : vides par défaut, remplies par le
// comptable, appliquées aux seules personnes déclarées. Un taux faux sur
// un bulletin est pire qu'aucun : l'écran ne propose aucun barème.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Plus, Trash2, Pencil } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Cotisation { id: string; libelle: string; qui: "salarie" | "employeur"; taux: number; plafond: number | null; compte: string | null }

const vide = { id: "", libelle: "", qui: "salarie" as const, taux: "", plafond: "", compte: "" };
const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const pct = (n: number) => String(n).replace(".", ",");

export function Cotisations() {
  const [liste, setListe] = useState<Cotisation[] | null>(null);
  const [edite, setEdite] = useState<typeof vide | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  const regle = peut("paie:preparer");

  const charger = useCallback(async () => {
    try { setListe(await invoke<Cotisation[]>("lire_cotisations")); } catch (e) { setAvis({ texte: String(e), erreur: true }); }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  async function faire(cmd: string, args: Record<string, unknown>, ok: string) {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke(cmd, args);
      setAvis({ texte: ok });
      setEdite(null);
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }
  const nombre = (s: string) => Number(s.replace(/\s/g, "").replace(",", "."));

  return (
    <div className="space-y-4 max-w-3xl">
      <p className="text-sm text-muted-foreground">
        Les cotisations (INPS, AMO, ITS…) ne s'appliquent qu'aux personnes <strong>déclarées</strong>, et seulement
        une fois saisies ici. Tant qu'il n'y en a aucune, aucune n'est retenue : le comptable les remplit, avec les
        taux en vigueur. Un changement ne touche pas les fiches déjà validées.
      </p>
      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}
      {!liste ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-cotisations">
          {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground" data-testid="aucune-cotisation">Aucune cotisation réglée : rien n'est retenu.</p>}
          {liste.map(c => (
            <div key={c.id} className="flex items-center gap-3 px-4 py-2 text-sm" data-testid="cotisation">
              <div className="flex-1">
                <div className="font-medium">{c.libelle}</div>
                <div className="text-xs text-muted-foreground">
                  {c.qui === "salarie" ? "retenue sur le salaire" : "à la charge de la société"} · {pct(c.taux)} % du brut
                  {c.plafond ? `, plafonné à ${f(c.plafond)}` : ""}{c.compte ? ` · compte ${c.compte}` : ""}
                </div>
              </div>
              {regle && (<>
                <Button size="sm" variant="ghost" aria-label={`Modifier ${c.libelle}`} onClick={() => setEdite({
                  id: c.id, libelle: c.libelle, qui: c.qui as never, taux: pct(c.taux), plafond: c.plafond ? String(c.plafond) : "", compte: c.compte ?? "",
                })}><Pencil className="h-4 w-4" /></Button>
                <Button size="sm" variant="ghost" aria-label={`Retirer ${c.libelle}`} disabled={enCours}
                  onClick={() => faire("retirer_cotisation", { cotisationId: c.id }, `${c.libelle} retirée.`)}><Trash2 className="h-4 w-4" /></Button>
              </>)}
            </div>
          ))}
        </div>
      )}
      {regle && !edite && (
        <Button size="sm" variant="outline" onClick={() => setEdite({ ...vide })}><Plus className="h-4 w-4 mr-1" /> Ajouter une cotisation</Button>
      )}
      {regle && edite && (
        <div className="border border-border rounded-lg p-4 flex gap-3 flex-wrap items-end" data-testid="formulaire-cotisation">
          <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-48">Nom
            <Input value={edite.libelle} onChange={e => setEdite({ ...edite, libelle: e.target.value })} aria-label="Nom de la cotisation"
              placeholder="INPS part salariale" className="h-9" /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Qui la paie
            <select value={edite.qui} onChange={e => setEdite({ ...edite, qui: e.target.value as never })} aria-label="Qui la paie"
              className="h-9 px-2 text-sm border border-border rounded-md bg-background">
              <option value="salarie">Le salarié (retenue)</option>
              <option value="employeur">L'employeur (charge)</option>
            </select></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Taux %
            <Input value={edite.taux} onChange={e => setEdite({ ...edite, taux: e.target.value })} aria-label="Taux" inputMode="decimal" className="h-9 w-20" /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Plafond (facultatif)
            <Input value={edite.plafond} onChange={e => setEdite({ ...edite, plafond: e.target.value })} aria-label="Plafond" inputMode="numeric" className="h-9 w-28" /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Compte (facultatif)
            <Input value={edite.compte} onChange={e => setEdite({ ...edite, compte: e.target.value })} aria-label="Compte de l'organisme"
              placeholder="431" className="h-9 w-24" /></label>
          <Button size="sm" disabled={enCours} onClick={() => faire("enregistrer_cotisation", {
            cotisation: {
              id: edite.id, libelle: edite.libelle, qui: edite.qui, taux: edite.taux.trim() ? nombre(edite.taux) : 0,
              plafond: edite.plafond.trim() ? nombre(edite.plafond) : null, compte: edite.compte.trim() || null,
            },
          }, `${edite.libelle.trim()} enregistrée. Les brouillons se recalculent pour la prendre.`)}>Enregistrer</Button>
          <Button size="sm" variant="ghost" onClick={() => setEdite(null)}>Annuler</Button>
        </div>
      )}
    </div>
  );
}
