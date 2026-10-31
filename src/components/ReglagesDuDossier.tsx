// components/ReglagesDuDossier.tsx
//
// D43 — ce que l'assistant du premier démarrage a posé et qui sert
// ensuite, ici le fond de caisse proposé à l'ouverture. La TVA de la
// boutique se règle dans Paramètres → TVA (EtatTvaBoutique), là où on la
// cherche (choix du propriétaire, 28/10/2026).

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Loader2, Save, SlidersHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { Input } from "@/components/ui/input";
import { lireSeuil } from "@/components/ChampSeuil";
import type { ReglagesDossier } from "@/lib/reglagesDossier";

export function ReglagesDuDossier() {
  const [r, setR] = useState<ReglagesDossier | null>(null);
  const [fond, setFond] = useState("");
  // D45 : le seuil d'alerte des articles qui n'ont pas le leur.
  const [seuil, setSeuil] = useState("");
  const [enCours, setEnCours] = useState(false);

  useEffect(() => {
    invoke<ReglagesDossier>("lire_reglages_dossier").then(v => {
      setR(v);
      setFond(v.fond_caisse > 0 ? String(v.fond_caisse) : "");
      setSeuil(v.seuil_defaut !== undefined ? String(v.seuil_defaut) : "");
    }).catch(() => setR(null));
  }, []);

  async function enregistrer() {
    setEnCours(true);
    try {
      const v = await invoke<ReglagesDossier>("definir_reglages_dossier", {
        // La TVA ne change pas ici : on renvoie celle du dossier.
        tvaPct: r?.tva_pct ?? 0, fondCaisse: parseMontant(fond) || 0,
        seuilDefaut: lireSeuil(seuil),
      });
      setR(v);
      await message("Réglages du dossier enregistrés.", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`${e}`, { title: "Réglages refusés", kind: "error" });
    } finally {
      setEnCours(false);
    }
  }

  if (!r) return null;
  return (
    <div className="space-y-4 max-w-lg border-t border-border pt-6">
      <div className="flex items-center gap-2">
        <SlidersHorizontal className="h-4 w-4 text-muted-foreground" />
        <h3 className="text-sm font-semibold">Réglages du dossier</h3>
      </div>
      <p className="text-xs text-muted-foreground">
        La TVA de la boutique ({r.tva_pct === 0 ? "pas de TVA" : `${r.tva_pct} %`}) se change dans Paramètres → TVA.
      </p>
      <div>
        <Label htmlFor="fond-habituel">Fond de caisse habituel (F)</Label>
        <MoneyInput id="fond-habituel" value={fond} onChange={setFond} placeholder="0" className="mt-1" />
        <p className="text-xs text-muted-foreground mt-1">
          Proposé à l'ouverture de la caisse ; le caissier tape ce qu'il y a vraiment dans le tiroir.
        </p>
      </div>
      <div>
        <Label htmlFor="seuil-defaut">Seuil d'alerte des articles</Label>
        <Input id="seuil-defaut" inputMode="decimal" value={seuil} onChange={e => setSeuil(e.target.value)}
          placeholder="5" className="mt-1" />
        <p className="text-xs text-muted-foreground mt-1">
          Pour les articles qui n'ont pas le leur : sous ce stock, ils passent « en alerte ». 0 : pas d'alerte.
        </p>
      </div>
      <Button onClick={enregistrer} disabled={enCours} variant="outline" className="w-full">
        {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Save className="h-4 w-4 mr-2" /> Enregistrer ces réglages</>}
      </Button>
    </div>
  );
}
