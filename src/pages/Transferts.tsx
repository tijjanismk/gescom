// pages/Transferts.tsx — Transferts inter-dépôts
//
// Déplacer de la marchandise entre deux dépôts du même propriétaire.
// Ni vente, ni achat, ni créance : le stock bouge, le CA ne bouge pas.
//
// Produit un bon numéroté (BTR-AAAA-NNNNN), imprimable et signable par
// le gérant qui reçoit.

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeftRight, Search, Loader2, Plus, X, Printer, RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { UTILISATEUR_ACTIF } from "@/App";

interface Depot { id: string; nom: string; est_defaut?: boolean; }
interface Unite { id: string; libelle: string; facteur: number; }
interface Article {
  id: string; nom: string; unite_base: string; stock: number;
  unites: Unite[];
}
interface Ligne {
  article_id: string; article_nom: string;
  unite_vente_id: string; unite_libelle: string; facteur: number;
  quantite: number; stock_dispo: number;
}
interface BonListe {
  bon: string; date: string; depot_source: string; depot_dest: string;
  nb_lignes: number; auteur: string; motif: string;
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}
function fmtQte(n: number): string {
  return n % 1 === 0 ? String(n) : n.toFixed(2);
}

export function Transferts() {
  const [depots, setDepots] = useState<Depot[]>([]);
  const [source, setSource] = useState("");
  const [dest, setDest] = useState("");
  const [articles, setArticles] = useState<Article[]>([]);
  const [recherche, setRecherche] = useState("");
  const [lignes, setLignes] = useState<Ligne[]>([]);
  const [motif, setMotif] = useState("");
  const [envoi, setEnvoi] = useState(false);
  const [historique, setHistorique] = useState<BonListe[]>([]);
  const [chargement, setChargement] = useState(true);

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const role = UTILISATEUR_ACTIF?.role ?? "employe";
      const [d, a, h] = await Promise.all([
        invoke<Depot[]>("lire_depots"),
        invoke<Article[]>("lire_articles_avec_unites", { role }),
        invoke<BonListe[]>("lire_transferts", { limite: 50 }),
      ]);
      setDepots(d);
      setArticles(a);
      setHistorique(h);
      if (!source && d.length > 0) {
        setSource(d.find(x => x.est_defaut)?.id ?? d[0].id);
      }
    } catch (e) {
      console.error("Erreur transferts :", e);
    } finally {
      setChargement(false);
    }
  }, [source]);

  useEffect(() => { charger(); }, []);

  const resultats = recherche.trim().length >= 2
    ? articles.filter(a =>
        a.nom.toLowerCase().includes(recherche.toLowerCase())
      ).slice(0, 8)
    : [];

  function ajouter(a: Article) {
    if (lignes.some(l => l.article_id === a.id)) return;
    const u = a.unites[0];
    setLignes(prev => [...prev, {
      article_id: a.id, article_nom: a.nom,
      unite_vente_id: u.id, unite_libelle: u.libelle, facteur: u.facteur,
      quantite: 1, stock_dispo: a.stock,
    }]);
    setRecherche("");
  }

  function modifier(i: number, champ: keyof Ligne, val: any) {
    setLignes(prev => prev.map((l, k) => k === i ? { ...l, [champ]: val } : l));
  }

  function changerUnite(i: number, uniteId: string) {
    const l = lignes[i];
    const a = articles.find(x => x.id === l.article_id);
    const u = a?.unites.find(x => x.id === uniteId);
    if (!u) return;
    setLignes(prev => prev.map((x, k) => k === i
      ? { ...x, unite_vente_id: u.id, unite_libelle: u.libelle, facteur: u.facteur }
      : x));
  }

  // Le stock affiché est celui du dépôt par défaut, pas du dépôt source.
  // On signale donc le dépassement sans bloquer : le Rust vérifie
  // vraiment, dépôt par dépôt, et refuse le transfert si besoin.
  const depassement = lignes.some(l => l.quantite * l.facteur > l.stock_dispo);

  async function valider() {
    if (!source || !dest || lignes.length === 0) return;
    if (source === dest) {
      await message("Les dépôts source et destination sont identiques.",
        { title: "Transfert impossible", kind: "warning" });
      return;
    }
    setEnvoi(true);
    try {
      const res = await invoke<{ bon: string; nb_lignes: number }>(
        "enregistrer_transfert", {
          depotSource: source,
          depotDest: dest,
          motif: motif.trim() || null,
          utilisateurRole: UTILISATEUR_ACTIF?.role ?? "employe",
          lignes: lignes.map(l => ({
            article_id:     l.article_id,
            unite_vente_id: l.unite_vente_id,
            quantite:       l.quantite,
            facteur:        l.facteur,
          })),
        });
      await message(
        `Bon ${res.bon} créé — ${res.nb_lignes} ligne(s).\n` +
        `Le stock a été déplacé.`,
        { title: "Transfert enregistré", kind: "info" });
      setLignes([]); setMotif("");
      await charger();
    } catch (e) {
      await message(`${e}`, { title: "Transfert refusé", kind: "error" });
    } finally {
      setEnvoi(false);
    }
  }

  async function imprimer(bon: string) {
    try {
      const d = await invoke<any>("lire_bon_transfert", { bon });
      const html = genererBonHTML(d);
      await invoke("imprimer_facture", { html, nomFichier: `${bon}.html` });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    }
  }

  if (chargement) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <ArrowLeftRight className="h-5 w-5" />
          <h1 className="text-2xl font-semibold">Transferts</h1>
        </div>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      {depots.length < 2 && (
        <div className="flex items-center gap-3 p-4 bg-muted rounded-lg mb-6">
          <ArrowLeftRight className="h-5 w-5 text-muted-foreground shrink-0" />
          <div>
            <p className="text-sm font-medium">Un seul dépôt configuré</p>
            <p className="text-xs text-muted-foreground">
              Créer un second dépôt dans Paramètres pour pouvoir
              transférer de la marchandise.
            </p>
          </div>
        </div>
      )}

      {/* ── Saisie ── */}
      <Card className="mb-6">
        <CardContent className="pt-4 space-y-4">

          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-[200px]">
              <Label className="text-xs mb-1 block">Dépôt source</Label>
              <select value={source} onChange={e => setSource(e.target.value)}
                className="w-full h-9 px-2 text-sm border border-border
                           rounded-md bg-background">
                {depots.map(d => (
                  <option key={d.id} value={d.id}>{d.nom}</option>
                ))}
              </select>
            </div>
            <ArrowLeftRight className="h-4 w-4 mb-2.5 text-muted-foreground" />
            <div className="min-w-[200px]">
              <Label className="text-xs mb-1 block">Dépôt destination</Label>
              <select value={dest} onChange={e => setDest(e.target.value)}
                className="w-full h-9 px-2 text-sm border border-border
                           rounded-md bg-background">
                <option value="">Choisir…</option>
                {depots.filter(d => d.id !== source).map(d => (
                  <option key={d.id} value={d.id}>{d.nom}</option>
                ))}
              </select>
            </div>
            <div className="flex-1 min-w-[200px]">
              <Label className="text-xs mb-1 block">Motif (optionnel)</Label>
              <Input value={motif} onChange={e => setMotif(e.target.value)}
                placeholder="Réapprovisionnement…" className="h-9" />
            </div>
          </div>

          {/* Recherche d'article */}
          <div className="relative">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input value={recherche} onChange={e => setRecherche(e.target.value)}
              placeholder="Rechercher un article…" className="h-9 pl-9" />
            {resultats.length > 0 && (
              <div className="absolute z-20 left-0 right-0 mt-1 bg-card border
                              border-border rounded-md shadow-lg max-h-64 overflow-auto">
                {resultats.map(a => (
                  <button key={a.id} onClick={() => ajouter(a)}
                    className="w-full flex items-center justify-between px-3 py-2
                               hover:bg-muted text-left text-sm">
                    <span>{a.nom}</span>
                    <span className="text-xs text-muted-foreground">
                      stock {fmtQte(a.stock)} {a.unite_base}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Lignes */}
          {lignes.length > 0 && (
            <div className="border border-border rounded-md divide-y divide-border">
              {lignes.map((l, i) => {
                const trop = l.quantite * l.facteur > l.stock_dispo;
                return (
                  <div key={l.article_id}
                    className="flex flex-wrap items-center gap-3 px-3 py-2">
                    <div className="flex-1 min-w-[160px]">
                      <p className="text-sm font-medium">{l.article_nom}</p>
                      <p className="text-xs text-muted-foreground">
                        stock connu : {fmtQte(l.stock_dispo)}
                      </p>
                    </div>
                    {(articles.find(a => a.id === l.article_id)?.unites.length ?? 0) > 1 && (
                      <select value={l.unite_vente_id}
                        onChange={e => changerUnite(i, e.target.value)}
                        className="h-8 text-xs border border-border rounded
                                   px-1 bg-background">
                        {articles.find(a => a.id === l.article_id)!.unites.map(u => (
                          <option key={u.id} value={u.id}>{u.libelle}</option>
                        ))}
                      </select>
                    )}
                    <Input type="number" min="0.01" step="any" value={l.quantite}
                      onChange={e => modifier(i, "quantite", parseFloat(e.target.value) || 0)}
                      className={`h-8 w-24 text-right text-sm ${
                        trop ? "border-destructive" : ""}`} />
                    <span className="text-xs text-muted-foreground w-16">
                      {l.unite_libelle}
                    </span>
                    <button onClick={() => setLignes(p => p.filter((_, k) => k !== i))}
                      className="p-1 text-muted-foreground hover:text-destructive">
                      <X className="h-4 w-4" />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          {depassement && (
            <p className="text-xs text-orange-600">
              Une quantité dépasse le stock connu. Le transfert sera refusé
              si le dépôt source ne l'a pas réellement.
            </p>
          )}

          <div className="flex justify-end">
            <Button onClick={valider}
              disabled={envoi || !dest || lignes.length === 0}>
              {envoi
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Plus className="h-4 w-4 mr-2" /> Enregistrer le transfert</>}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* ── Historique ── */}
      <h2 className="text-sm font-semibold mb-2 text-muted-foreground uppercase">
        Bons de transfert ({historique.length})
      </h2>
      {historique.length === 0 ? (
        <p className="text-sm text-muted-foreground py-6">
          Aucun transfert enregistré.
        </p>
      ) : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {historique.map(b => (
            <div key={b.bon}
              className="flex items-center gap-3 px-4 py-2.5 hover:bg-muted/30">
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium font-mono">{b.bon}</p>
                <p className="text-xs text-muted-foreground">
                  {fmtDate(b.date)} · {b.depot_source} → {b.depot_dest}
                  · {b.nb_lignes} ligne(s)
                  {b.motif && ` · ${b.motif}`}
                </p>
              </div>
              <Button variant="ghost" size="sm" onClick={() => imprimer(b.bon)}>
                <Printer className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// =====================================================================
//  Bon de transfert imprimable
// =====================================================================
//
//  Volontairement sobre : pas de montant. Un transfert ne vaut rien
//  comptablement, il déplace. Ce que le gérant signe, c'est la
//  réception de quantités.

function genererBonHTML(d: any): string {
  const lignes = d.lignes.map((l: any, i: number) => `
    <tr style="background:${i % 2 ? "#f9f9f9" : "#fff"};border-bottom:1px solid #eee">
      <td style="padding:6px 8px">${l.article}</td>
      <td style="padding:6px 8px;text-align:center">${l.unite}</td>
      <td style="padding:6px 8px;text-align:right;font-weight:600">
        ${l.quantite % 1 === 0 ? l.quantite : l.quantite.toFixed(2)}
      </td>
      <td style="padding:6px 8px;width:90px"></td>
    </tr>`).join("");

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<title>${d.bon}</title>
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  body{font-family:Arial,sans-serif;font-size:12px;padding:14mm}
  @media print{@page{size:210mm 297mm;margin:0}}
</style></head><body>

<div style="display:flex;justify-content:space-between;margin-bottom:16px">
  <div>
    <div style="font-size:17px;font-weight:bold">${d.societe.nom}</div>
    ${d.societe.adresse ? `<div style="font-size:10px;color:#555">${d.societe.adresse}</div>` : ""}
    ${d.societe.telephone ? `<div style="font-size:10px;color:#555">Tél : ${d.societe.telephone}</div>` : ""}
  </div>
  <div style="text-align:right">
    <div style="font-size:15px;font-weight:bold">BON DE TRANSFERT</div>
    <div style="margin-top:4px">N° <strong>${d.bon}</strong></div>
    <div style="font-size:10px;color:#555">
      Date : ${new Date(d.date).toLocaleDateString("fr-ML")} à
      ${new Date(d.date).toLocaleTimeString("fr-ML", {
        hour: "2-digit", minute: "2-digit" })}
    </div>
  </div>
</div>

<table style="width:100%;margin-bottom:14px;border:1px solid #ddd">
  <tr>
    <td style="padding:8px;border-right:1px solid #ddd;width:50%">
      <div style="font-size:9px;color:#777;text-transform:uppercase">Départ</div>
      <div style="font-weight:bold">${d.depot_source}</div>
    </td>
    <td style="padding:8px">
      <div style="font-size:9px;color:#777;text-transform:uppercase">Destination</div>
      <div style="font-weight:bold">${d.depot_dest}</div>
    </td>
  </tr>
</table>

${d.motif ? `<div style="margin-bottom:10px;font-size:11px;color:#555">
  <em>Motif : ${d.motif}</em></div>` : ""}

<table style="width:100%;border-collapse:collapse;margin-bottom:20px">
  <thead>
    <tr style="background:#f0f0f0;border-bottom:2px solid #000">
      <th style="text-align:left;padding:6px 8px">Désignation</th>
      <th style="text-align:center;padding:6px 8px">Unité</th>
      <th style="text-align:right;padding:6px 8px">Quantité</th>
      <th style="text-align:center;padding:6px 8px">Reçu</th>
    </tr>
  </thead>
  <tbody>${lignes}</tbody>
</table>

<div style="font-size:10px;color:#777;margin-bottom:24px">
  Ce bon ne constitue ni une vente ni un achat. Aucun montant n'est dû.
</div>

<div style="display:flex;justify-content:space-between;margin-top:40px">
  <div style="width:45%;border-top:1px solid #000;padding-top:6px;text-align:center">
    Remis par — ${d.auteur}
  </div>
  <div style="width:45%;border-top:1px solid #000;padding-top:6px;text-align:center">
    Reçu par
  </div>
</div>

<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}