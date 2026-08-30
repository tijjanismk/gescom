// components/OngletCodesBarres.tsx
//
// Attribution des codes EAN-13 et impression d'étiquettes.
//
// Les codes internes commencent par 20 — préfixe réservé à l'usage
// privé, attribué à aucun pays. Aucune collision possible avec un code
// du commerce. Un article qui a déjà un code fabricant garde le sien :
// il est imprimé sur l'emballage.

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import {
  Barcode, Loader2, RefreshCw, Wand2, Printer, Search, Check, X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface ArticleCode {
  id: string; nom: string; code_barre: string;
  unite_base: string; prix: number;
  interne: boolean; valide: boolean;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

export function OngletCodesBarres() {
  const [articles, setArticles] = useState<ArticleCode[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");
  const [sansCode, setSansCode] = useState(false);
  const [edition, setEdition] = useState<string | null>(null);
  const [saisie, setSaisie] = useState("");
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [travail, setTravail] = useState(false);

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      setArticles(await invoke<ArticleCode[]>("lire_articles_codes_barres",
        { sansCodeSeulement: sansCode }));
    } catch (e) {
      console.error("Erreur codes-barres :", e);
    } finally {
      setChargement(false);
    }
  }, [sansCode]);

  useEffect(() => { charger(); }, [charger]);

  const filtres = recherche.trim()
    ? articles.filter(a =>
        a.nom.toLowerCase().includes(recherche.toLowerCase()) ||
        a.code_barre.includes(recherche.trim()))
    : articles;

  const nbSansCode = articles.filter(a => !a.code_barre).length;

  async function genererUn(a: ArticleCode) {
    try {
      await invoke("generer_code_barre", { articleId: a.id });
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  async function genererTous() {
    setTravail(true);
    try {
      const r = await invoke<{ generes: number; restants: number }>(
        "generer_codes_barres_manquants");
      await message(
        `${r.generes} code(s) généré(s).` +
        (r.restants > 0 ? `\n${r.restants} article(s) restant(s).` : ""),
        { title: "Codes-barres", kind: "info" });
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  async function enregistrerSaisie(a: ArticleCode) {
    try {
      await invoke("definir_code_barre", { articleId: a.id, code: saisie });
      setEdition(null); setSaisie("");
      await charger();
    } catch (e) {
      // Le Rust nomme l'article en conflit, ou explique la clé invalide.
      await message(`${e}`, { title: "Code refusé", kind: "error" });
    }
  }

  function basculer(id: string) {
    setSelection(prev => {
      const s = new Set(prev);
      s.has(id) ? s.delete(id) : s.add(id);
      return s;
    });
  }

  async function imprimerEtiquettes() {
    const choisis = articles.filter(a => selection.has(a.id) && a.code_barre);
    if (choisis.length === 0) return;
    try {
      const html = genererEtiquettesHTML(choisis);
      await invoke("imprimer_facture", {
        html, nomFichier: `etiquettes_${Date.now()}.html`,
      });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    }
  }

  if (chargement && articles.length === 0) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-8">
        <Loader2 className="h-4 w-4 animate-spin" /> Chargement…
      </div>
    );
  }

  return (
    <div className="space-y-4">

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold flex items-center gap-2">
            <Barcode className="h-5 w-5" /> Codes-barres
          </h2>
          <p className="text-xs text-muted-foreground">
            Les codes internes commencent par 20 — réservé à l'usage
            privé, aucune collision avec un code du commerce.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" onClick={charger}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
          {nbSansCode > 0 && (
            <Button size="sm" onClick={genererTous} disabled={travail}>
              {travail
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Wand2 className="h-4 w-4 mr-2" />
                    Générer les {nbSansCode} manquants</>}
            </Button>
          )}
          <Button variant="outline" size="sm"
            onClick={imprimerEtiquettes} disabled={selection.size === 0}>
            <Printer className="h-4 w-4 mr-2" />
            Étiquettes ({selection.size})
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 min-w-[220px]">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
            placeholder="Article ou code…" className="h-9 pl-8 text-sm" />
        </div>
        <label className="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" checked={sansCode}
            onChange={e => setSansCode(e.target.checked)}
            className="w-3.5 h-3.5 rounded accent-primary" />
          Sans code seulement
        </label>
      </div>

      <div className="border border-border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted/50 border-b border-border">
            <tr>
              <th className="w-10 px-3 py-2">
                <input type="checkbox"
                  checked={selection.size > 0 && selection.size === filtres.filter(a => a.code_barre).length}
                  onChange={e => setSelection(e.target.checked
                    ? new Set(filtres.filter(a => a.code_barre).map(a => a.id))
                    : new Set())}
                  className="w-3.5 h-3.5 rounded accent-primary" />
              </th>
              <th className="text-left px-3 py-2 text-xs font-medium
                             text-muted-foreground">Article</th>
              <th className="text-left px-3 py-2 text-xs font-medium
                             text-muted-foreground">Code-barres</th>
              <th className="text-right px-3 py-2 text-xs font-medium
                             text-muted-foreground">Prix</th>
              <th className="w-24 px-3 py-2"></th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {filtres.map(a => (
              <tr key={a.id} className="hover:bg-muted/30">
                <td className="px-3 py-2">
                  <input type="checkbox" disabled={!a.code_barre}
                    checked={selection.has(a.id)}
                    onChange={() => basculer(a.id)}
                    className="w-3.5 h-3.5 rounded accent-primary
                               disabled:opacity-30" />
                </td>
                <td className="px-3 py-2">
                  {a.nom}
                  <span className="text-xs text-muted-foreground">
                    {" "}· {a.unite_base}
                  </span>
                </td>
                <td className="px-3 py-2">
                  {edition === a.id ? (
                    <div className="flex items-center gap-1">
                      <Input value={saisie} autoFocus
                        onChange={e => setSaisie(e.target.value)}
                        onKeyDown={e => {
                          if (e.key === "Enter") enregistrerSaisie(a);
                          if (e.key === "Escape") { setEdition(null); setSaisie(""); }
                        }}
                        placeholder="13 chiffres"
                        className="h-7 w-40 text-sm font-mono" />
                      <button onClick={() => enregistrerSaisie(a)}
                        className="p-1 text-green-700 hover:bg-muted rounded">
                        <Check className="h-4 w-4" />
                      </button>
                      <button onClick={() => { setEdition(null); setSaisie(""); }}
                        className="p-1 text-muted-foreground hover:bg-muted rounded">
                        <X className="h-4 w-4" />
                      </button>
                    </div>
                  ) : a.code_barre ? (
                    <button onClick={() => { setEdition(a.id); setSaisie(a.code_barre); }}
                      className="font-mono text-sm hover:underline text-left">
                      {a.code_barre}
                      {a.interne && (
                        <span className="ml-2 text-[10px] px-1 py-0.5 rounded
                                         bg-muted text-muted-foreground">interne</span>
                      )}
                      {!a.valide && (
                        <span className="ml-2 text-[10px] px-1 py-0.5 rounded
                                         bg-red-100 text-red-700">clé invalide</span>
                      )}
                    </button>
                  ) : (
                    <span className="text-xs text-muted-foreground italic">
                      aucun
                    </span>
                  )}
                </td>
                <td className="px-3 py-2 text-right text-muted-foreground">
                  {a.prix > 0 ? fmt(a.prix) : "—"}
                </td>
                <td className="px-3 py-2 text-right">
                  {!a.code_barre && edition !== a.id && (
                    <div className="flex gap-1 justify-end">
                      <button onClick={() => genererUn(a)}
                        title="Générer un code interne"
                        className="p-1.5 rounded hover:bg-muted text-muted-foreground
                                   hover:text-primary">
                        <Wand2 className="h-3.5 w-3.5" />
                      </button>
                      <button onClick={() => { setEdition(a.id); setSaisie(""); }}
                        title="Saisir un code fabricant"
                        className="p-1.5 rounded hover:bg-muted text-muted-foreground
                                   hover:text-blue-600">
                        <Barcode className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {filtres.length === 0 && (
          <p className="text-sm text-muted-foreground text-center py-6">
            Aucun article.
          </p>
        )}
      </div>

      <p className="text-xs text-muted-foreground">
        Un article qui a déjà un code fabricant le conserve : il est
        imprimé sur l'emballage, le remplacer rendrait la douchette
        inutile.
      </p>
    </div>
  );
}

// =====================================================================
//  Étiquettes imprimables
// =====================================================================
//
//  Le code-barres est dessiné en CSS : des barres de largeurs variables
//  ne se scannent pas de façon fiable sur une imprimante de bureau. On
//  imprime donc le NUMÉRO en gros, lisible et saisissable à la main.
//
//  Pour de vrais codes scannables, il faut une imprimante d'étiquettes
//  et une police EAN-13 dédiée.

function genererEtiquettesHTML(articles: ArticleCode[]): string {
  const cases = articles.map(a => `
    <div class="etq">
      <div class="nom">${a.nom}</div>
      <div class="prix">${new Intl.NumberFormat("fr-ML").format(a.prix)} F</div>
      <div class="code">${a.code_barre}</div>
    </div>`).join("");

  return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8"><title>Étiquettes</title>
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  body{font-family:Arial,sans-serif;padding:8mm;
       display:grid;grid-template-columns:repeat(4,1fr);gap:3mm}
  .etq{border:1px dashed #bbb;padding:3mm;text-align:center;
       height:26mm;display:flex;flex-direction:column;justify-content:space-between}
  .nom{font-size:9px;font-weight:600;line-height:1.15;
       overflow:hidden;max-height:22px}
  .prix{font-size:14px;font-weight:bold}
  .code{font-family:"Courier New",monospace;font-size:11px;
        letter-spacing:1px;border-top:1px solid #ddd;padding-top:1mm}
  @media print{@page{size:A4;margin:6mm} .etq{border-color:#eee}}
</style></head><body>${cases}
<script>window.onload = () => { window.focus(); window.print(); }</script>
</body></html>`;
}
