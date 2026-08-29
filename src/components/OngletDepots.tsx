// components/OngletDepots.tsx
//
// Gestion des dépôts — lieux de stockage d'un MÊME commerce.
//
// Un dépôt est un point de vente, un entrepôt ou une réserve. Chacun a
// son stock ; les ventes, achats et pièces portent le dépôt où ils ont
// eu lieu.
//
// Ce n'est PAS du multi-magasin au sens comptable : il y a une seule
// caisse, un seul tiroir. Le jour où chaque lieu encaisse pour son
// compte, il faudra un depot_id sur session_caisse et un rapprochement
// par lieu — un vrai chantier, pas un renommage.

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message, confirm } from "@tauri-apps/plugin-dialog";
import {
  Warehouse, Plus, Loader2, Star, Pencil, Power, RefreshCw, TrendingUp,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

interface DepotDetail {
  id: string; nom: string;
  est_defaut: boolean; actif: boolean;
  nb_articles: number; valeur_stock: number; nb_ventes: number;
}
interface ResumeDepot {
  depot_id: string; nom: string;
  ca: number; nb_ventes: number; encaisse: number; impaye: number;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// ---------------------------------------------------------------------

function ModalDepot({
  ouvert, depot, onFermer, onEnregistre,
}: {
  ouvert: boolean;
  depot: DepotDetail | null;   // null = création
  onFermer: () => void;
  onEnregistre: () => void;
}) {
  const [nom, setNom] = useState("");
  const [parDefaut, setParDefaut] = useState(false);
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert) {
      setNom(depot?.nom ?? "");
      setParDefaut(depot?.est_defaut ?? false);
    }
  }, [ouvert, depot]);

  async function enregistrer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      if (depot) {
        await invoke("renommer_depot", { depotId: depot.id, nom: nom.trim() });
        if (parDefaut && !depot.est_defaut) {
          await invoke("definir_depot_defaut", { depotId: depot.id });
        }
      } else {
        await invoke("creer_depot", { nom: nom.trim(), estDefaut: parDefaut });
      }
      onEnregistre();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent style={{ width: "420px", maxWidth: "92vw" }}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Warehouse className="h-4 w-4" />
            {depot ? "Renommer le dépôt" : "Nouveau dépôt"}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div>
            <Label className="text-xs mb-1.5 block">Nom</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Djelibougou, Moribabougou…" autoFocus
              onKeyDown={e => e.key === "Enter" && enregistrer()} />
          </div>

          {!depot?.est_defaut && (
            <label className="flex items-center gap-2 cursor-pointer">
              <input type="checkbox" checked={parDefaut}
                onChange={e => setParDefaut(e.target.checked)}
                className="w-3.5 h-3.5 rounded accent-primary" />
              <span className="text-sm">Définir comme dépôt par défaut</span>
            </label>
          )}

          {!depot && (
            <p className="text-xs text-muted-foreground">
              Tous les articles y seront créés avec un stock à zéro.
              Utiliser un transfert ou un achat pour l'approvisionner.
            </p>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Annuler
            </Button>
            <Button onClick={enregistrer}
              disabled={chargement || !nom.trim()} className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : (depot ? "Renommer" : "Créer")}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------

export function OngletDepots() {
  const [depots, setDepots] = useState<DepotDetail[]>([]);
  const [resume, setResume] = useState<ResumeDepot[]>([]);
  const [chargement, setChargement] = useState(true);
  const [modal, setModal] = useState(false);
  const [aModifier, setAModifier] = useState<DepotDetail | null>(null);

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const [d, r] = await Promise.all([
        invoke<DepotDetail[]>("lire_depots_detail"),
        invoke<ResumeDepot[]>("lire_resume_par_depot", {
          dateDebut: null, dateFin: null,
        }),
      ]);
      setDepots(d);
      setResume(r);
    } catch (e) {
      console.error("Erreur depots :", e);
    } finally {
      setChargement(false);
    }
  }, []);

  useEffect(() => { charger(); }, [charger]);

  async function definirDefaut(d: DepotDetail) {
    try {
      await invoke("definir_depot_defaut", { depotId: d.id });
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  async function desactiver(d: DepotDetail) {
    const ok = await confirm(
      `Désactiver « ${d.nom} » ?\n\n` +
      `Le dépôt n'apparaîtra plus dans les sélecteurs. ` +
      `Son historique est conservé.`,
      { title: "Désactiver le dépôt", kind: "warning" },
    );
    if (!ok) return;
    try {
      await invoke("desactiver_depot", { depotId: d.id });
      await charger();
    } catch (e) {
      // Le Rust refuse s'il reste du stock — le message est explicite.
      await message(`${e}`, { title: "Désactivation refusée", kind: "error" });
    }
  }

  if (chargement) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-8">
        <Loader2 className="h-4 w-4 animate-spin" /> Chargement…
      </div>
    );
  }

  const actifs = depots.filter(d => d.actif);

  return (
    <div className="space-y-6">

      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">Dépôts</h2>
          <p className="text-xs text-muted-foreground">
            Chaque dépôt a son propre stock. Le sélecteur en haut de la
            barre latérale filtre le tableau de bord et le journal.
            La caisse reste unique, commune à tous les dépôts.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={charger}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
          <Button size="sm" onClick={() => { setAModifier(null); setModal(true); }}>
            <Plus className="h-4 w-4 mr-2" /> Nouveau dépôt
          </Button>
        </div>
      </div>

      {actifs.length === 1 && (
        <div className="flex items-center gap-3 p-3 bg-muted rounded-lg">
          <Warehouse className="h-4 w-4 text-muted-foreground shrink-0" />
          <p className="text-xs text-muted-foreground">
            Un seul dépôt : le sélecteur reste masqué dans la barre
            latérale. Il apparaîtra dès le second.
          </p>
        </div>
      )}

      {/* ── Liste ── */}
      <div className="border border-border rounded-lg divide-y divide-border">
        {depots.map(d => (
          <div key={d.id}
            className={`flex flex-wrap items-center gap-3 px-4 py-3
                        ${!d.actif ? "opacity-50" : "hover:bg-muted/30"}`}>
            <Warehouse className="h-4 w-4 text-muted-foreground shrink-0" />
            <div className="flex-1 min-w-[160px]">
              <p className="text-sm font-medium flex items-center gap-2">
                {d.nom}
                {d.est_defaut && (
                  <span className="text-[10px] px-1.5 py-0.5 rounded
                                   bg-primary/10 text-primary font-medium">
                    par défaut
                  </span>
                )}
                {!d.actif && (
                  <span className="text-[10px] text-muted-foreground">
                    désactivé
                  </span>
                )}
              </p>
              <p className="text-xs text-muted-foreground">
                {d.nb_articles} article(s) en stock · {d.nb_ventes} vente(s)
              </p>
            </div>
            <div className="text-right min-w-[110px]">
              <p className="text-xs text-muted-foreground">Valeur du stock</p>
              <p className="text-sm font-semibold">{fmt(d.valeur_stock)}</p>
            </div>
            <div className="flex items-center gap-1">
              {!d.est_defaut && d.actif && (
                <button onClick={() => definirDefaut(d)}
                  title="Définir comme dépôt par défaut"
                  className="p-1.5 rounded hover:bg-muted text-muted-foreground
                             hover:text-primary">
                  <Star className="h-3.5 w-3.5" />
                </button>
              )}
              <button onClick={() => { setAModifier(d); setModal(true); }}
                title="Renommer"
                className="p-1.5 rounded hover:bg-muted text-muted-foreground
                           hover:text-blue-600">
                <Pencil className="h-3.5 w-3.5" />
              </button>
              {d.actif && !d.est_defaut && (
                <button onClick={() => desactiver(d)}
                  title="Désactiver"
                  className="p-1.5 rounded hover:bg-muted text-muted-foreground
                             hover:text-destructive">
                  <Power className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* ── Comparatif du mois ── */}
      {resume.length > 1 && (
        <Card>
          <CardContent className="pt-4">
            <div className="flex items-center gap-2 mb-3">
              <TrendingUp className="h-4 w-4 text-muted-foreground" />
              <h3 className="text-sm font-semibold">Comparatif du mois</h3>
            </div>
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border">
                  <th className="text-left py-2 text-xs font-medium
                                 text-muted-foreground">Dépôt</th>
                  <th className="text-right py-2 text-xs font-medium
                                 text-muted-foreground">Ventes</th>
                  <th className="text-right py-2 text-xs font-medium
                                 text-muted-foreground">CA</th>
                  <th className="text-right py-2 text-xs font-medium
                                 text-muted-foreground">Encaissé</th>
                  <th className="text-right py-2 text-xs font-medium
                                 text-muted-foreground">Impayé</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {resume.map(r => (
                  <tr key={r.depot_id}>
                    <td className="py-2">{r.nom}</td>
                    <td className="py-2 text-right text-muted-foreground">
                      {r.nb_ventes}
                    </td>
                    <td className="py-2 text-right font-semibold">
                      {fmt(r.ca)}
                    </td>
                    <td className="py-2 text-right text-green-700">
                      {fmt(r.encaisse)}
                    </td>
                    <td className={`py-2 text-right font-medium ${
                      r.impaye > 0 ? "text-red-600" : "text-muted-foreground"
                    }`}>
                      {r.impaye > 0 ? fmt(r.impaye) : "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
              <tfoot className="border-t-2 border-foreground">
                <tr>
                  <td className="py-2 font-bold">Total</td>
                  <td className="py-2 text-right font-bold">
                    {resume.reduce((s, r) => s + r.nb_ventes, 0)}
                  </td>
                  <td className="py-2 text-right font-bold">
                    {fmt(resume.reduce((s, r) => s + r.ca, 0))}
                  </td>
                  <td className="py-2 text-right font-bold text-green-700">
                    {fmt(resume.reduce((s, r) => s + r.encaisse, 0))}
                  </td>
                  <td className="py-2 text-right font-bold text-red-600">
                    {fmt(resume.reduce((s, r) => s + r.impaye, 0))}
                  </td>
                </tr>
              </tfoot>
            </table>
            <p className="text-xs text-muted-foreground mt-3">
              Les transferts entre dépôts n'apparaissent pas ici : ils
              déplacent du stock, ils ne créent pas de chiffre d'affaires.
            </p>
          </CardContent>
        </Card>
      )}

      <ModalDepot
        ouvert={modal}
        depot={aModifier}
        onFermer={() => setModal(false)}
        onEnregistre={() => { setModal(false); charger(); }}
      />
    </div>
  );
}
