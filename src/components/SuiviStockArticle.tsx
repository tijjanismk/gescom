// components/SuiviStockArticle.tsx — Paramètres → Articles : suivre ou
// non le stock d'un article (D36).
//
// Sachets, crédit de recharge, livraison, vrac : ils se vendent
// normalement, jamais à découvert, et sortent des écrans de stock. Cesser
// de suivre dit d'abord combien d'unités sortent ainsi du stock ;
// reprendre demande de compter chaque magasin — le stock repart de la
// quantité réelle, jamais d'un chiffre faux.

import { useState } from "react";
import { Loader2 } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

interface StockMagasin { depot_id: string; nom: string; quantite: number; }

export function SuiviStockArticle({ article, onChange }: {
  article: { id: string; nom: string; unite_base: string; gere_en_stock?: boolean };
  onChange: () => void;
}) {
  const suivi = article.gere_en_stock !== false;
  const [magasins, setMagasins] = useState<StockMagasin[] | null>(null);
  const [comptes, setComptes] = useState<Record<string, string>>({});
  const [enCours, setEnCours] = useState(false);
  const [erreur, setErreur] = useState("");

  async function cesser() {
    setErreur("");
    try {
      const stocks = await invoke<StockMagasin[]>("lire_stock_article_depots", { articleId: article.id });
      const total = stocks.reduce((s, m) => s + m.quantite, 0);
      const oui = await ask(
        `« ${article.nom} » ne suivra plus son stock : il se vendra sans `
        + `jamais être à découvert et quittera l'écran Stock.\n\n`
        + (total !== 0
          ? `${total} ${article.unite_base} sortent ainsi du stock et de sa valeur. `
          : "")
        + "Pour le suivre à nouveau, il faudra compter chaque magasin.",
        { title: "Ne plus suivre le stock", kind: "warning" },
      );
      if (!oui) return;
      setEnCours(true);
      await invoke("changer_suivi_stock", { articleId: article.id, suivre: false });
      onChange();
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  async function ouvrirReprise() {
    setErreur("");
    try {
      const stocks = await invoke<StockMagasin[]>("lire_stock_article_depots", { articleId: article.id });
      setMagasins(stocks);
      setComptes({});
    } catch (e) {
      setErreur(String(e));
    }
  }

  async function reprendre() {
    if (!magasins) return;
    setEnCours(true);
    setErreur("");
    try {
      await invoke("changer_suivi_stock", {
        articleId: article.id,
        suivre: true,
        comptages: magasins.map(m => ({
          depot_id: m.depot_id,
          quantite: parseFloat((comptes[m.depot_id] ?? "").replace(",", ".")),
        })),
      });
      setMagasins(null);
      // La liste relue le dit (« Stock suivi ») : pas de boîte de plus.
      onChange();
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  const toutCompte = !!magasins && magasins.every(m => (comptes[m.depot_id] ?? "").trim() !== "");

  return (
    <div className="flex flex-wrap items-center gap-2 pb-2 mb-1 border-b border-border text-sm"
      data-testid="suivi-stock">
      <span className="text-muted-foreground">
        {suivi ? "Stock suivi" : "Stock non suivi : vendu sans compter, hors des écrans de stock"}
      </span>
      <Button size="sm" variant="outline" className="h-7 text-xs ml-auto" disabled={enCours}
        onClick={suivi ? cesser : ouvrirReprise}>
        {enCours ? <Loader2 className="h-3 w-3 animate-spin" /> : suivi ? "Ne plus suivre le stock" : "Suivre le stock"}
      </Button>
      {erreur && !magasins && <p className="w-full text-xs text-red-600">{erreur}</p>}

      <Dialog open={!!magasins} onOpenChange={o => { if (!o) setMagasins(null); }}>
        <DialogContent className="max-w-sm">
          <DialogHeader><DialogTitle>Suivre le stock : {article.nom}</DialogTitle></DialogHeader>
          <div className="space-y-3 pt-1">
            <p className="text-xs text-muted-foreground">
              Compter ce qu'il y a vraiment dans chaque magasin
              ({article.unite_base}). Le stock repart de ces quantités.
            </p>
            {magasins?.map(m => (
              <div key={m.depot_id}>
                <Label>{m.nom}</Label>
                <Input className="mt-1" inputMode="decimal" placeholder="Quantité comptée"
                  aria-label={`Quantité comptée — ${m.nom}`}
                  value={comptes[m.depot_id] ?? ""}
                  onChange={e => setComptes(c => ({ ...c, [m.depot_id]: e.target.value }))} />
              </div>
            ))}
            {erreur && <p className="rounded-md bg-red-500/10 p-2 text-xs text-red-600">{erreur}</p>}
            <div className="flex gap-2 pt-1">
              <Button variant="outline" className="flex-1" onClick={() => setMagasins(null)}>Annuler</Button>
              <Button className="flex-1" disabled={!toutCompte || enCours} onClick={reprendre}>
                {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Suivre le stock"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
