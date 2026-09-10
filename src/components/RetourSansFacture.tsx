// components/RetourSansFacture.tsx
//
// Rendre une marchandise entrée SANS facture.
//
// Le cas type : on emprunte dix sacs au magasin d'à côté un vendredi
// de rupture, on les rend le lundi. Rien
// n'a été facturé, donc rien n'est dû — et rendre ces sacs ne doit
// créer aucun avoir, sans quoi on inventerait un crédit chez un
// fournisseur à qui l'on ne doit rien.
//
// C'est le miroir exact de l'entrée sans facture (D42) : du stock qui
// sort, et rien d'autre.

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Loader2, PackageMinus, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { UTILISATEUR_ACTIF } from "@/App";

interface Depot { id: string; nom: string; est_defaut?: boolean; }
interface Fournisseur { id: string; nom: string; est_voisin?: boolean; }
interface ArticleStock {
  id: string;
  nom: string;
  unite_base: string;
  stock: number;
}

export function RetourSansFacture({ onTermine }: { onTermine: () => void }) {
  const [depots, setDepots] = useState<Depot[]>([]);
  const [depotId, setDepotId] = useState("");
  const [fournisseurs, setFournisseurs] = useState<Fournisseur[]>([]);
  const [fournisseurId, setFournisseurId] = useState("");
  const [articles, setArticles] = useState<ArticleStock[]>([]);
  const [recherche, setRecherche] = useState("");
  const [articleId, setArticleId] = useState("");
  const [quantite, setQuantite] = useState("");
  const [motif, setMotif] = useState("");
  const [envoi, setEnvoi] = useState(false);
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    invoke<Depot[]>("lire_depots")
      .then(liste => {
        setDepots(liste);
        const defaut = liste.find(d => d.est_defaut) ?? liste[0];
        if (defaut) setDepotId(defaut.id);
      })
      .catch(console.error);
    invoke<Fournisseur[]>("lire_fournisseurs")
      .then(setFournisseurs)
      .catch(console.error);
  }, []);

  // Le stock affiché doit être celui du dépôt choisi. Sans ce
  // rechargement, on proposerait de rendre dix sacs depuis un magasin
  // qui n'en a aucun, et le refus tomberait au clic.
  useEffect(() => {
    if (!depotId) return;
    setChargement(true);
    setArticleId("");
    invoke<ArticleStock[]>("lire_articles_avec_unites", {
      role: UTILISATEUR_ACTIF?.role ?? "employe",
      depotId,
    })
      .then(setArticles)
      .catch(e => console.error("Articles :", e))
      .finally(() => setChargement(false));
  }, [depotId]);

  const filtres = articles
    .filter(a => a.stock > 0)
    .filter(a =>
      !recherche || a.nom.toLowerCase().includes(recherche.toLowerCase()),
    )
    .slice(0, 60);

  const article = articles.find(a => a.id === articleId) ?? null;
  const q = parseFloat(quantite) || 0;
  const trop = !!article && q > article.stock;
  const pretAValider = !!article && q > 0 && !trop && !envoi;

  async function valider() {
    if (!pretAValider || !article) return;
    setEnvoi(true);
    try {
      await invoke("enregistrer_retour_sans_facture", {
        articleId: article.id,
        depotId,
        quantite: q,
        fournisseurId: fournisseurId || null,
        motif: motif.trim() || null,
        utilisateurRole: UTILISATEUR_ACTIF?.role ?? "employe",
      });
      await message(
        `${q} ${article.unite_base} de « ${article.nom} » sortis du stock.\n\n`
        + "Aucun avoir n'a été créé : cette marchandise n'avait pas été "
        + "facturée, il n'y a donc rien à déduire d'une dette.",
        { title: "Retour enregistré", kind: "info" },
      );
      setQuantite("");
      setMotif("");
      setArticleId("");
      // Recharger le stock : la ligne qu'on vient de rendre a changé.
      const maj = await invoke<ArticleStock[]>("lire_articles_avec_unites", {
        role: UTILISATEUR_ACTIF?.role ?? "employe",
        depotId,
      });
      setArticles(maj);
      onTermine();
    } catch (e) {
      await message(String(e), { title: "Retour refusé", kind: "error" });
    } finally {
      setEnvoi(false);
    }
  }

  return (
    <div className="space-y-4 rounded-lg border border-border p-4">
      <div>
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <PackageMinus className="h-4 w-4" />
          Retour sans facture
        </h3>
        <p className="mt-0.5 text-xs text-muted-foreground">
          Marchandise reçue sans facture — dépannage, emprunt au voisin.
          Le stock sort ; aucun avoir, aucune dette, aucun mouvement de
          caisse.
        </p>
      </div>

      <div className="flex flex-wrap items-end gap-3">
        <div className="min-w-[180px]">
          <Label className="mb-1 block text-xs">Dépôt</Label>
          <Select value={depotId} onValueChange={v => { if (v) setDepotId(v); }}>
            <SelectTrigger className="h-9"><SelectValue /></SelectTrigger>
            <SelectContent>
              {depots.map(d => (
                <SelectItem key={d.id} value={d.id}>{d.nom}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="min-w-[200px]">
          <Label className="mb-1 block text-xs">
            Fournisseur <span className="text-muted-foreground">(facultatif)</span>
          </Label>
          <Select value={fournisseurId}
                  onValueChange={v => setFournisseurId(!v || v === "aucun" ? "" : v)}>
            <SelectTrigger className="h-9">
              <SelectValue placeholder="Non précisé" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="aucun">Non précisé</SelectItem>
              {fournisseurs.map(f => (
                <SelectItem key={f.id} value={f.id}>
                  {f.nom}{f.est_voisin ? " · voisin" : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="relative max-w-[240px] flex-1">
          <Label className="mb-1 block text-xs">Article</Label>
          <Search className="absolute left-2.5 top-[30px] h-4 w-4 text-muted-foreground" />
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
                 placeholder="Chercher un article…"
                 className="h-9 pl-8 text-sm" />
        </div>
      </div>

      {chargement ? (
        <div className="flex items-center gap-2 py-4 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Lecture du stock…
        </div>
      ) : (
        <div className="max-h-52 overflow-y-auto rounded border border-border">
          {filtres.length === 0 ? (
            <p className="p-3 text-sm text-muted-foreground">
              Aucun article en stock dans ce dépôt.
            </p>
          ) : filtres.map(a => (
            <button key={a.id} onClick={() => setArticleId(a.id)}
              className={cn(
                "flex w-full items-center justify-between px-3 py-2 text-left text-sm",
                "border-b border-border last:border-b-0 hover:bg-muted/50",
                articleId === a.id && "bg-primary/5",
              )}>
              <span className={cn(articleId === a.id && "font-medium text-primary")}>
                {a.nom}
              </span>
              <span className="text-xs text-muted-foreground">
                {a.stock} {a.unite_base}
              </span>
            </button>
          ))}
        </div>
      )}

      {article && (
        <div className="flex flex-wrap items-end gap-3 border-t border-border pt-3">
          <div className="w-32">
            <Label className="mb-1 block text-xs">
              Quantité ({article.unite_base})
            </Label>
            <Input type="number" min="0" step="any" max={article.stock}
              value={quantite} onChange={e => setQuantite(e.target.value)}
              placeholder={`max ${article.stock}`}
              className={cn("h-9 text-right text-sm", trop && "border-destructive")} />
          </div>

          <div className="min-w-[220px] flex-1">
            <Label className="mb-1 block text-xs">
              Motif <span className="text-muted-foreground">(facultatif)</span>
            </Label>
            <Input value={motif} onChange={e => setMotif(e.target.value)}
              placeholder="Rendu au voisin, marchandise abîmée…"
              className="h-9 text-sm" />
          </div>

          <Button onClick={valider} disabled={!pretAValider}>
            {envoi
              ? <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              : <PackageMinus className="mr-2 h-4 w-4" />}
            Sortir du stock
          </Button>
        </div>
      )}

      {trop && (
        <p className="text-xs text-destructive">
          Il n'y a que {article?.stock} {article?.unite_base} en stock. On ne
          rend pas ce qu'on n'a pas — la marchandise doit physiquement
          quitter la boutique.
        </p>
      )}
    </div>
  );
}
