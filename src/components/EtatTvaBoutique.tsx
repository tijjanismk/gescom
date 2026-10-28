// components/EtatTvaBoutique.tsx
//
// D43 — « La boutique facture-t-elle la TVA ? », en tête de Paramètres →
// TVA (choix du propriétaire, 28/10/2026). Changer l'état demande ce que
// deviennent les articles existants : seulement les nouveaux, ou aussi
// tous ceux du catalogue — en UNE transaction côté serveur
// (`definir_tva_boutique`), tracée à l'Historique.

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { message } from "@tauri-apps/plugin-dialog";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ReglagesDossier } from "@/lib/reglagesDossier";

interface Reglages extends ReglagesDossier { nb_articles: number; nb_dossiers: number }

export function EtatTvaBoutique({ onChange }: { onChange?: () => void }) {
  const [r, setR] = useState<Reglages | null>(null);
  const [voulu, setVoulu] = useState<number | null>(null);
  const [enCours, setEnCours] = useState(false);

  const lire = () => invoke<Reglages>("lire_reglages_dossier").then(setR).catch(() => setR(null));
  useEffect(() => { lire(); }, []);

  async function appliquer(tousLesArticles: boolean) {
    if (voulu === null) return;
    setEnCours(true);
    try {
      const v = await invoke<Reglages & { articles_modifies: number }>("definir_tva_boutique", {
        tvaPct: voulu, appliquerAuxArticles: tousLesArticles,
      });
      setR(v);
      setVoulu(null);
      onChange?.();
      await message(
        tousLesArticles
          ? `TVA à ${v.tva_pct} % : ${v.articles_modifies} article(s) mis à ce taux, et les nouveaux le prendront.`
          : `TVA à ${v.tva_pct} % pour les nouveaux articles. Les articles existants gardent leur taux.`,
        { title: "TVA de la boutique", kind: "info" },
      );
    } catch (e) {
      await message(`${e}`, { title: "TVA refusée", kind: "error" });
    } finally {
      setEnCours(false);
    }
  }

  if (!r) return null;
  const modifiable = peut("parametres:modifier");
  const libelle = (t: number) => t === 0 ? "Non, pas de TVA" : `Oui, ${t} %`;

  return (
    <div className="border border-border bg-card rounded-lg p-4 space-y-3">
      <div>
        <p className="text-sm font-semibold">La boutique facture-t-elle la TVA ?</p>
        <p className="text-xs text-muted-foreground mt-0.5">
          Aujourd'hui : <strong className="text-foreground">{r.tva_pct === 0 ? "non" : `oui, ${r.tva_pct} %`}</strong>.
          C'est le taux que prend chaque nouvel article ; plus bas, chaque article garde le sien.
        </p>
      </div>
      <div className="flex gap-2">
        {r.taux_proposes.map(t => (
          <Button key={t} size="sm" disabled={!modifiable || enCours}
            variant={(voulu ?? r.tva_pct) === t ? "default" : "outline"}
            onClick={() => setVoulu(t === r.tva_pct ? null : t)}>
            {libelle(t)}
          </Button>
        ))}
      </div>

      {voulu !== null && (
        <div className="rounded-md border border-border border-l-4 border-l-primary bg-background p-3 space-y-2 text-sm" role="status">
          <p>
            Passer à <strong>{voulu === 0 ? "pas de TVA" : `${voulu} %`}</strong>. Et les{" "}
            <strong>{r.nb_articles}</strong> article(s) déjà au catalogue ?
          </p>
          {r.nb_dossiers > 1 && (
            <p className="text-xs text-muted-foreground">
              Les articles sont communs aux {r.nb_dossiers} dossiers : « tous les articles » change aussi leur taux dans les autres.
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" disabled={enCours} onClick={() => appliquer(false)}>
              Seulement les nouveaux articles
            </Button>
            <Button size="sm" disabled={enCours || r.nb_articles === 0} onClick={() => appliquer(true)}>
              {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : `Aussi les ${r.nb_articles} article(s) existants`}
            </Button>
            <Button size="sm" variant="ghost" disabled={enCours} onClick={() => setVoulu(null)}>Annuler</Button>
          </div>
        </div>
      )}
    </div>
  );
}
