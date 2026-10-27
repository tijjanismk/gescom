// components/OngletMenu.tsx — Paramètres → Menu (D37).
//
// Ce que le menu de ce dossier montre. Un petit commerçant n'a que faire
// de Relances, Historique, Chèques… : il les cache ici, d'une case ou
// d'un réglage tout fait. Cacher n'est qu'un affichage — rien ne
// s'efface, aucune règle ne change, l'Historique enregistre toujours.

import { useEffect, useState } from "react";
import { CheckCircle2, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { chargerMenu, definirMenu, type ReglageMenu } from "@/lib/menu";

export function OngletMenu() {
  const [reglage, setReglage] = useState<ReglageMenu | null>(null);
  const [caches, setCaches] = useState<string[]>([]);
  const [erreur, setErreur] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [fait, setFait] = useState(false);

  useEffect(() => {
    chargerMenu()
      .then(r => { setReglage(r); setCaches(r.caches); })
      .catch(e => setErreur(String(e)));
  }, []);

  if (!reglage) {
    return erreur
      ? <p className="text-sm text-red-600">{erreur}</p>
      : (
        <div className="flex items-center gap-2 py-8 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Lecture du menu…
        </div>
      );
  }

  const modifie = [...caches].sort().join() !== [...reglage.caches].sort().join();
  const basculer = (href: string) => {
    setFait(false);
    setCaches(c => c.includes(href) ? c.filter(x => x !== href) : [...c, href]);
  };

  async function enregistrer() {
    setEnCours(true);
    setErreur("");
    try {
      const r = await definirMenu(caches);
      setReglage(r);
      setCaches(r.caches);
      setFait(true);
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="max-w-2xl space-y-5">
      <p className="text-sm text-muted-foreground">
        Les entrées du menu de ce dossier, pour toutes ses caisses.
        Cacher une entrée n'est qu'un affichage : rien ne s'efface, aucune
        règle ne change, et l'Historique continue d'enregistrer chaque geste.
      </p>

      <div className="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" onClick={() => { setFait(false); setCaches(reglage.boutique_simple); }}>
          Boutique simple
        </Button>
        <Button variant="outline" size="sm" onClick={() => { setFait(false); setCaches([]); }}>
          Complet
        </Button>
      </div>

      <div className="rounded-lg border border-border divide-y divide-border">
        {reglage.toujours.map(e => (
          <label key={e.href} className="flex items-center gap-3 px-4 py-2 text-sm text-muted-foreground">
            <input type="checkbox" checked disabled aria-label={e.nom} />
            {e.nom}
            <span className="ml-auto text-xs">toujours dans le menu</span>
          </label>
        ))}
        {reglage.masquables.map(e => (
          <label key={e.href} className="flex items-center gap-3 px-4 py-2 text-sm cursor-pointer hover:bg-muted/40">
            <input type="checkbox" aria-label={e.nom}
              checked={!caches.includes(e.href)} onChange={() => basculer(e.href)} />
            {e.nom}
            {caches.includes(e.href) && <span className="ml-auto text-xs text-muted-foreground">caché</span>}
          </label>
        ))}
      </div>

      {erreur && <p className="rounded-md bg-red-500/10 p-2 text-xs text-red-600">{erreur}</p>}

      <div className="flex items-center gap-3 border-t border-border pt-4">
        <Button onClick={enregistrer} disabled={!modifie || enCours}>
          {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer le menu"}
        </Button>
        {fait && (
          <span role="status" className="flex items-center gap-1.5 text-xs text-green-700">
            <CheckCircle2 className="h-3.5 w-3.5" /> Menu enregistré
          </span>
        )}
      </div>
    </div>
  );
}
