// components/ChampSeuil.tsx
//
// D45 — le seuil d'alerte d'un article, en unité de base. Vide : celui
// du dossier (Paramètres → Société, 5 d'office). 0 : pas d'alerte,
// seulement la rupture. Le serveur juge (nombre positif, borné) ; l'écran
// ne fait que lire ce qui est tapé.

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

/** « 12 », « 2,5 », vide → null. Illisible → NaN (le serveur refusera). */
export function lireSeuil(texte: string): number | null {
  const t = texte.trim().replace(/\s/g, "").replace(",", ".");
  return t === "" ? null : Number(t);
}

/** Le seuil du dossier, pour l'afficher en exemple. */
export function useSeuilDuDossier(): number | null {
  const [s, setS] = useState<number | null>(null);
  useEffect(() => {
    invoke<{ seuil_defaut?: number }>("lire_reglages_dossier")
      .then(r => setS(r.seuil_defaut ?? null))
      .catch(() => setS(null));
  }, []);
  return s;
}

export function ChampSeuil({ id, valeur, onChange, unite }: {
  id: string; valeur: string; onChange: (v: string) => void; unite?: string;
}) {
  const duDossier = useSeuilDuDossier();
  return (
    <div>
      <Label htmlFor={id}>Seuil d'alerte{unite ? ` (en ${unite})` : ""}</Label>
      <Input id={id} inputMode="decimal" value={valeur} onChange={e => onChange(e.target.value)}
        placeholder={duDossier !== null ? `celui du dossier : ${duDossier}` : "celui du dossier"}
        className="mt-1" />
      <p className="text-xs text-muted-foreground mt-1">
        Sous ce stock, l'article passe « en alerte ». Vide : le seuil du dossier ; 0 : pas d'alerte.
      </p>
    </div>
  );
}

/** Sur la fiche d'un article (Paramètres → Articles) : changer son seuil. */
export function SeuilArticle({ article, onChange }: {
  article: { id: string; unite_base: string; seuil_alerte?: number | null; gere_en_stock?: boolean };
  onChange: () => void;
}) {
  const [texte, setTexte] = useState(article.seuil_alerte != null ? String(article.seuil_alerte) : "");
  const [enCours, setEnCours] = useState(false);
  const duDossier = useSeuilDuDossier();
  if (article.gere_en_stock === false) return null;
  const actuel = article.seuil_alerte != null ? String(article.seuil_alerte) : "";

  async function enregistrer() {
    setEnCours(true);
    try {
      await invoke("definir_seuil_article", { articleId: article.id, seuilAlerte: lireSeuil(texte) });
      onChange();
    } catch (e) {
      await message(`${e}`, { title: "Seuil refusé", kind: "error" });
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="flex flex-wrap items-end gap-2 py-2 border-b border-border">
      <div className="w-56">
        <Label htmlFor={`seuil-${article.id}`} className="text-xs">
          Seuil d'alerte (en {article.unite_base})
        </Label>
        <Input id={`seuil-${article.id}`} inputMode="decimal" value={texte}
          onChange={e => setTexte(e.target.value)} className="mt-1 h-8 text-sm"
          placeholder={duDossier !== null ? `celui du dossier : ${duDossier}` : "celui du dossier"} />
      </div>
      <Button size="sm" variant="outline" className="h-8" disabled={enCours || texte.trim() === actuel}
        onClick={enregistrer}>
        {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer"}
      </Button>
      <p className="text-xs text-muted-foreground w-full">
        {article.seuil_alerte != null
          ? (article.seuil_alerte === 0 ? "Pas d'alerte pour cet article, seulement la rupture." : `En alerte sous ${article.seuil_alerte} ${article.unite_base}.`)
          : `Suit le seuil du dossier${duDossier !== null ? ` (${duDossier})` : ""}.`}
      </p>
    </div>
  );
}
