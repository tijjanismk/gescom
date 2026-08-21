import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  TrendingUp, Users, Package, FileText,
  Loader2, Printer, Download, RefreshCw,
  BarChart2, AlertTriangle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { message } from "@tauri-apps/plugin-dialog";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

// =====================================================================
//  Types
// =====================================================================

interface MoisCA { mois: string; ca: number; nb_ventes: number; encaisse: number; }
interface ClientRapport {
  id: string; code: string; nom: string; telephone?: string;
  ca: number; nb_ventes: number; creances: number;
}
interface ArticleRapport {
  id: string; nom: string; unite: string;
  ca: number; qte_vendue: number; nb_ventes: number; marge: number;
}
interface StockRapport {
  nom: string; unite: string; quantite: number;
  prix_achat: number; valeur_stock: number; depot: string; statut: string;
}
interface CreanceRapport {
  moins_30j: { montant: number; nb: number };
  tranche_30_60: { montant: number; nb: number };
  tranche_60_90: { montant: number; nb: number };
  plus_90j: { montant: number; nb: number };
}

// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtCompact(n: number) {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M F";
  if (n >= 1_000) return (n / 1_000).toFixed(0) + "k F";
  return fmt(n);
}
function nomMois(ym: string) {
  const [y, m] = ym.split("-");
  return new Date(parseInt(y), parseInt(m) - 1).toLocaleDateString("fr-ML", {
    month: "long", year: "numeric"
  });
}

// Période par défaut : début du mois → aujourd'hui
function debutMois() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-01T00:00:00`;
}
function aujourd_hui() {
  return new Date().toISOString().slice(0, 10) + "T23:59:59";
}

// =====================================================================
//  Export Excel via SheetJS
// =====================================================================

async function exporterExcel(
  feuilles: { nom: string; donnees: any[] }[],
  nomFichier: string,
) {
  try {
    const XLSX = await import("xlsx");
    const wb = XLSX.utils.book_new();
    for (const { nom, donnees } of feuilles) {
      const ws = XLSX.utils.json_to_sheet(donnees);
      XLSX.utils.book_append_sheet(wb, ws, nom);
    }
    XLSX.writeFile(wb, nomFichier);
  } catch (e) {
    await message(`Erreur export : ${e}`, { title: "Erreur", kind: "error" });
  }
}

// =====================================================================
//  Impression HTML
// =====================================================================

async function imprimerRapport(html: string, nom: string) {
  try {
    await tauriInvoke("imprimer_facture", { html, nomFichier: `${nom}.html` });
  } catch (e) {
    await message(`Erreur impression : ${e}`, { title: "Erreur", kind: "error" });
  }
}

function genererHTMLRapport(titre: string, tableauHTML: string, periode?: string): string {
  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${titre}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:12px; color:#000; padding:15mm; }
    h1 { font-size:18px; margin-bottom:4px; }
    .periode { font-size:11px; color:#555; margin-bottom:16px; }
    table { width:100%; border-collapse:collapse; }
    th { background:#f0f0f0; padding:6px 8px; text-align:left; font-size:11px; border-bottom:2px solid #000; }
    td { padding:5px 8px; border-bottom:1px solid #eee; font-size:11px; }
    tr:nth-child(even) { background:#fafafa; }
    .droite { text-align:right; }
    .total { font-weight:bold; border-top:2px solid #000; }
    @media print { @page { size:A4; margin:10mm; } }
  </style>
</head>
<body>
  <h1>${titre}</h1>
  ${periode ? `<div class="periode">${periode}</div>` : ""}
  ${tableauHTML}
  <div style="margin-top:16px;font-size:10px;color:#aaa;border-top:1px solid #ddd;padding-top:8px;">
    Généré le ${new Date().toLocaleDateString("fr-ML", { day:"2-digit", month:"2-digit", year:"numeric", hour:"2-digit", minute:"2-digit" })}
  </div>
  <script>window.onload = () => { window.focus(); window.print(); }</script>
</body>
</html>`;
}

// =====================================================================
//  Onglets
// =====================================================================

const ONGLETS = [
  { key: "ca",       label: "CA mensuel",   icone: BarChart2  },
  { key: "clients",  label: "Top clients",  icone: Users      },
  { key: "articles", label: "Top articles", icone: Package    },
  { key: "stock",    label: "Stock",        icone: FileText   },
  { key: "creances", label: "Créances",     icone: AlertTriangle },
];

// =====================================================================
//  Page Rapports
// =====================================================================

export function Rapports() {
  const [onglet, setOnglet] = useState("ca");
  const [dateDebut, setDateDebut] = useState(debutMois().slice(0, 10));
  const [dateFin, setDateFin] = useState(aujourd_hui().slice(0, 10));
  const [chargement, setChargement] = useState(false);

  // Données
  const [moisCA, setMoisCA] = useState<MoisCA[]>([]);
  const [topClients, setTopClients] = useState<ClientRapport[]>([]);
  const [topArticles, setTopArticles] = useState<ArticleRapport[]>([]);
  const [stock, setStock] = useState<StockRapport[]>([]);
  const [creances, setCreances] = useState<CreanceRapport | null>(null);

  const charger = useCallback(async () => {
    setChargement(true);
    const dd = dateDebut + "T00:00:00";
    const df = dateFin + "T23:59:59";
    try {
      switch (onglet) {
        case "ca":
          setMoisCA(await invoke<MoisCA[]>("lire_rapport_ca_mensuel", { nbMois: 12 }));
          break;
        case "clients":
          setTopClients(await invoke<ClientRapport[]>("lire_rapport_top_clients", {
            dateDebut: dd, dateFin: df, limite: 30,
          }));
          break;
        case "articles":
          setTopArticles(await invoke<ArticleRapport[]>("lire_rapport_top_articles", {
            dateDebut: dd, dateFin: df, limite: 30,
          }));
          break;
        case "stock":
          setStock(await invoke<StockRapport[]>("lire_rapport_stock"));
          break;
        case "creances":
          setCreances(await invoke<CreanceRapport>("lire_rapport_creances"));
          break;
      }
    } catch (e) { console.error("Erreur rapport :", e); }
    finally { setChargement(false); }
  }, [onglet, dateDebut, dateFin]);

  useEffect(() => { charger(); }, [charger]);

  // ---- Actions export ----

  async function handleImprimerCA() {
    const maxCA = Math.max(...moisCA.map(m => m.ca), 1);
    const lignes = moisCA.map(m => `
      <tr>
        <td>${nomMois(m.mois)}</td>
        <td class="droite">${m.nb_ventes}</td>
        <td class="droite">${fmt(m.ca)}</td>
        <td class="droite">${fmt(m.encaisse)}</td>
        <td class="droite">${fmt(m.ca - m.encaisse)}</td>
      </tr>`).join("");
    const totalCA = moisCA.reduce((s, m) => s + m.ca, 0);
    const totalEnc = moisCA.reduce((s, m) => s + m.encaisse, 0);
    const html = genererHTMLRapport("Rapport CA mensuel", `
      <table>
        <thead><tr>
          <th>Mois</th><th class="droite">Ventes</th>
          <th class="droite">CA (F)</th><th class="droite">Encaissé (F)</th>
          <th class="droite">Créances (F)</th>
        </tr></thead>
        <tbody>${lignes}</tbody>
        <tfoot><tr class="total">
          <td>TOTAL</td><td></td>
          <td class="droite">${fmt(totalCA)}</td>
          <td class="droite">${fmt(totalEnc)}</td>
          <td class="droite">${fmt(totalCA - totalEnc)}</td>
        </tr></tfoot>
      </table>`);
    await imprimerRapport(html, "rapport_ca_mensuel");
  }

  async function handleExcelCA() {
    await exporterExcel([{
      nom: "CA mensuel",
      donnees: moisCA.map(m => ({
        "Mois": nomMois(m.mois),
        "Nb ventes": m.nb_ventes,
        "CA (F)": m.ca,
        "Encaissé (F)": m.encaisse,
        "Créances (F)": m.ca - m.encaisse,
      }))
    }], "rapport_ca_mensuel.xlsx");
  }

  async function handleImprimerClients() {
    const lignes = topClients.map((c, i) => `
      <tr>
        <td>${i+1}</td><td>${c.nom}</td><td>${c.code}</td>
        <td>${c.telephone ?? "—"}</td>
        <td class="droite">${c.nb_ventes}</td>
        <td class="droite">${fmt(c.ca)}</td>
        <td class="droite" style="color:${c.creances > 0 ? "#e65c00" : "inherit"}">
          ${c.creances > 0 ? fmt(c.creances) : "—"}
        </td>
      </tr>`).join("");
    const periode = `Du ${dateDebut} au ${dateFin}`;
    const html = genererHTMLRapport("Top clients", `
      <table>
        <thead><tr>
          <th>#</th><th>Client</th><th>Code</th><th>Téléphone</th>
          <th class="droite">Ventes</th><th class="droite">CA (F)</th>
          <th class="droite">Créances (F)</th>
        </tr></thead>
        <tbody>${lignes}</tbody>
      </table>`, periode);
    await imprimerRapport(html, "rapport_clients");
  }

  async function handleExcelClients() {
    await exporterExcel([{
      nom: "Top clients",
      donnees: topClients.map((c, i) => ({
        "Rang": i+1, "Nom": c.nom, "Code": c.code,
        "Téléphone": c.telephone ?? "",
        "Nb ventes": c.nb_ventes,
        "CA (F)": c.ca, "Créances (F)": c.creances,
      }))
    }], "rapport_clients.xlsx");
  }

  async function handleImprimerArticles() {
    const lignes = topArticles.map((a, i) => `
      <tr>
        <td>${i+1}</td><td>${a.nom}</td>
        <td class="droite">${a.qte_vendue % 1 === 0 ? a.qte_vendue : a.qte_vendue.toFixed(2)} ${a.unite}</td>
        <td class="droite">${a.nb_ventes}</td>
        <td class="droite">${fmt(a.ca)}</td>
        <td class="droite">${fmt(a.marge)}</td>
      </tr>`).join("");
    const html = genererHTMLRapport("Top articles", `
      <table>
        <thead><tr>
          <th>#</th><th>Article</th><th class="droite">Qté vendue</th>
          <th class="droite">Ventes</th><th class="droite">CA (F)</th>
          <th class="droite">Marge (F)</th>
        </tr></thead>
        <tbody>${lignes}</tbody>
      </table>`, `Du ${dateDebut} au ${dateFin}`);
    await imprimerRapport(html, "rapport_articles");
  }

  async function handleExcelArticles() {
    await exporterExcel([{
      nom: "Top articles",
      donnees: topArticles.map((a, i) => ({
        "Rang": i+1, "Article": a.nom, "Unité": a.unite,
        "Qté vendue": a.qte_vendue, "Nb ventes": a.nb_ventes,
        "CA (F)": a.ca, "Marge (F)": a.marge,
      }))
    }], "rapport_articles.xlsx");
  }

  async function handleImprimerStock() {
    const valeurTotale = stock.reduce((s, a) => s + a.valeur_stock, 0);
    const lignes = stock.map(a => `
      <tr>
        <td>${a.nom}</td><td>${a.depot}</td>
        <td class="droite">${a.quantite % 1 === 0 ? a.quantite : a.quantite.toFixed(2)} ${a.unite}</td>
        <td class="droite">${fmt(a.prix_achat)}</td>
        <td class="droite">${fmt(a.valeur_stock)}</td>
        <td style="color:${a.statut==="rupture"?"#dc2626":a.statut==="alerte"?"#e65c00":"#16a34a"}">
          ${a.statut === "rupture" ? "Rupture" : a.statut === "alerte" ? "Alerte" : "OK"}
        </td>
      </tr>`).join("");
    const html = genererHTMLRapport("État du stock", `
      <table>
        <thead><tr>
          <th>Article</th><th>Dépôt</th><th class="droite">Qté</th>
          <th class="droite">P.A. (F)</th><th class="droite">Valeur (F)</th>
          <th>Statut</th>
        </tr></thead>
        <tbody>${lignes}</tbody>
        <tfoot><tr class="total">
          <td colspan="4">VALEUR TOTALE STOCK</td>
          <td class="droite">${fmt(valeurTotale)}</td><td></td>
        </tr></tfoot>
      </table>`);
    await imprimerRapport(html, "rapport_stock");
  }

  async function handleExcelStock() {
    await exporterExcel([{
      nom: "Stock",
      donnees: stock.map(a => ({
        "Article": a.nom, "Dépôt": a.depot, "Quantité": a.quantite,
        "Unité": a.unite, "Prix achat (F)": a.prix_achat,
        "Valeur stock (F)": a.valeur_stock, "Statut": a.statut,
      }))
    }], "rapport_stock.xlsx");
  }

  // ---- Render ----

  const hasPeriode = ["clients", "articles"].includes(onglet);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* En-tête */}
      <div className="px-6 pt-5 pb-4 border-b border-border bg-card shrink-0">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-xl font-semibold">Rapports</h1>
          <Button size="sm" variant="outline" onClick={charger}>
            <RefreshCw className="h-3.5 w-3.5 mr-1" /> Actualiser
          </Button>
        </div>

        {/* Onglets */}
        <div className="flex gap-1 flex-wrap">
          {ONGLETS.map(o => {
            const Icone = o.icone;
            return (
              <button key={o.key} onClick={() => setOnglet(o.key)}
                className={`flex items-center gap-2 px-4 py-2 text-sm font-medium
                            rounded-lg transition-colors ${
                  onglet === o.key
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted"
                }`}>
                <Icone className="h-4 w-4" />{o.label}
              </button>
            );
          })}
        </div>

        {/* Filtre période */}
        {hasPeriode && (
          <div className="flex items-center gap-3 mt-3">
            <Label className="text-xs shrink-0">Période</Label>
            <Input type="date" value={dateDebut}
              onChange={e => setDateDebut(e.target.value)}
              className="h-7 text-xs w-32" />
            <span className="text-xs text-muted-foreground">→</span>
            <Input type="date" value={dateFin}
              onChange={e => setDateFin(e.target.value)}
              className="h-7 text-xs w-32" />
          </div>
        )}
      </div>

      {/* Contenu */}
      <div className="flex-1 overflow-auto p-6">
        {chargement ? (
          <div className="flex justify-center py-16">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : (
          <>

            {/* ---- CA mensuel ---- */}
            {onglet === "ca" && (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={handleImprimerCA}>
                    <Printer className="h-3.5 w-3.5 mr-1.5" /> Imprimer
                  </Button>
                  <Button size="sm" variant="outline" onClick={handleExcelCA}>
                    <Download className="h-3.5 w-3.5 mr-1.5" /> Excel
                  </Button>
                </div>
                {/* Mini graphe barres */}
                {moisCA.length > 0 && (
                  <div className="bg-card border border-border rounded-xl p-4">
                    <div className="flex items-end gap-1 h-32 mb-2">
                      {[...moisCA].reverse().map((m, i) => {
                        const maxCA = Math.max(...moisCA.map(x => x.ca), 1);
                        const h = Math.round((m.ca / maxCA) * 100);
                        return (
                          <div key={i} className="flex-1 flex flex-col items-center gap-1 group">
                            <div className="w-full bg-primary/80 hover:bg-primary rounded-sm transition-all"
                              style={{ height: `${Math.max(h, 2)}%` }}
                              title={`${nomMois(m.mois)}\n${fmt(m.ca)}`} />
                            <span className="text-[9px] text-muted-foreground rotate-45 origin-left">
                              {m.mois.slice(5)}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}
                <div className="border border-border rounded-xl overflow-hidden">
                  <table className="w-full text-sm">
                    <thead className="bg-muted text-xs">
                      <tr>
                        <th className="text-left px-4 py-2.5">Mois</th>
                        <th className="text-right px-4 py-2.5">Ventes</th>
                        <th className="text-right px-4 py-2.5">CA</th>
                        <th className="text-right px-4 py-2.5">Encaissé</th>
                        <th className="text-right px-4 py-2.5">Créances</th>
                      </tr>
                    </thead>
                    <tbody>
                      {moisCA.map((m, i) => (
                        <tr key={i} className={`border-t border-border ${i % 2 === 1 ? "bg-muted/20" : ""}`}>
                          <td className="px-4 py-2 font-medium capitalize">{nomMois(m.mois)}</td>
                          <td className="px-4 py-2 text-right text-muted-foreground">{m.nb_ventes}</td>
                          <td className="px-4 py-2 text-right font-semibold">{fmt(m.ca)}</td>
                          <td className="px-4 py-2 text-right text-green-600">{fmt(m.encaisse)}</td>
                          <td className="px-4 py-2 text-right text-orange-600">
                            {m.ca - m.encaisse > 0 ? fmt(m.ca - m.encaisse) : "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                    <tfoot className="border-t-2 border-border bg-muted/50">
                      <tr>
                        <td className="px-4 py-2 font-bold">Total</td>
                        <td className="px-4 py-2 text-right font-medium">
                          {moisCA.reduce((s, m) => s + m.nb_ventes, 0)}
                        </td>
                        <td className="px-4 py-2 text-right font-bold">
                          {fmt(moisCA.reduce((s, m) => s + m.ca, 0))}
                        </td>
                        <td className="px-4 py-2 text-right font-bold text-green-600">
                          {fmt(moisCA.reduce((s, m) => s + m.encaisse, 0))}
                        </td>
                        <td className="px-4 py-2 text-right font-bold text-orange-600">
                          {fmt(moisCA.reduce((s, m) => s + (m.ca - m.encaisse), 0))}
                        </td>
                      </tr>
                    </tfoot>
                  </table>
                </div>
              </div>
            )}

            {/* ---- Top clients ---- */}
            {onglet === "clients" && (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={handleImprimerClients}>
                    <Printer className="h-3.5 w-3.5 mr-1.5" /> Imprimer
                  </Button>
                  <Button size="sm" variant="outline" onClick={handleExcelClients}>
                    <Download className="h-3.5 w-3.5 mr-1.5" /> Excel
                  </Button>
                </div>
                <div className="border border-border rounded-xl overflow-hidden">
                  <table className="w-full text-sm">
                    <thead className="bg-muted text-xs">
                      <tr>
                        <th className="text-left px-4 py-2.5 w-8">#</th>
                        <th className="text-left px-4 py-2.5">Client</th>
                        <th className="text-left px-3 py-2.5">Téléphone</th>
                        <th className="text-right px-4 py-2.5">Ventes</th>
                        <th className="text-right px-4 py-2.5">CA</th>
                        <th className="text-right px-4 py-2.5">Créances</th>
                      </tr>
                    </thead>
                    <tbody>
                      {topClients.map((c, i) => (
                        <tr key={c.id}
                          className={`border-t border-border ${i % 2 === 1 ? "bg-muted/20" : ""}`}>
                          <td className="px-4 py-2 text-muted-foreground font-mono text-xs">
                            {i+1}
                          </td>
                          <td className="px-4 py-2">
                            <p className="font-medium">{c.nom}</p>
                            <p className="text-xs text-muted-foreground">{c.code}</p>
                          </td>
                          <td className="px-3 py-2 text-xs text-muted-foreground">
                            {c.telephone ?? "—"}
                          </td>
                          <td className="px-4 py-2 text-right">{c.nb_ventes}</td>
                          <td className="px-4 py-2 text-right font-semibold">{fmt(c.ca)}</td>
                          <td className={`px-4 py-2 text-right ${c.creances > 0 ? "text-orange-600 font-medium" : "text-muted-foreground"}`}>
                            {c.creances > 0 ? fmt(c.creances) : "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* ---- Top articles ---- */}
            {onglet === "articles" && (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={handleImprimerArticles}>
                    <Printer className="h-3.5 w-3.5 mr-1.5" /> Imprimer
                  </Button>
                  <Button size="sm" variant="outline" onClick={handleExcelArticles}>
                    <Download className="h-3.5 w-3.5 mr-1.5" /> Excel
                  </Button>
                </div>
                <div className="border border-border rounded-xl overflow-hidden">
                  <table className="w-full text-sm">
                    <thead className="bg-muted text-xs">
                      <tr>
                        <th className="text-left px-4 py-2.5 w-8">#</th>
                        <th className="text-left px-4 py-2.5">Article</th>
                        <th className="text-right px-4 py-2.5">Qté vendue</th>
                        <th className="text-right px-4 py-2.5">Ventes</th>
                        <th className="text-right px-4 py-2.5">CA</th>
                        <th className="text-right px-4 py-2.5">Marge</th>
                      </tr>
                    </thead>
                    <tbody>
                      {topArticles.map((a, i) => (
                        <tr key={a.id}
                          className={`border-t border-border ${i % 2 === 1 ? "bg-muted/20" : ""}`}>
                          <td className="px-4 py-2 text-muted-foreground font-mono text-xs">
                            {i+1}
                          </td>
                          <td className="px-4 py-2 font-medium">{a.nom}</td>
                          <td className="px-4 py-2 text-right text-muted-foreground">
                            {a.qte_vendue % 1 === 0 ? a.qte_vendue : a.qte_vendue.toFixed(2)} {a.unite}
                          </td>
                          <td className="px-4 py-2 text-right">{a.nb_ventes}</td>
                          <td className="px-4 py-2 text-right font-semibold">{fmt(a.ca)}</td>
                          <td className={`px-4 py-2 text-right ${a.marge > 0 ? "text-green-600" : "text-red-500"}`}>
                            {fmt(a.marge)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* ---- Stock ---- */}
            {onglet === "stock" && (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={handleImprimerStock}>
                    <Printer className="h-3.5 w-3.5 mr-1.5" /> Imprimer
                  </Button>
                  <Button size="sm" variant="outline" onClick={handleExcelStock}>
                    <Download className="h-3.5 w-3.5 mr-1.5" /> Excel
                  </Button>
                </div>
                <div className="flex gap-3 text-xs">
                  <span className="text-green-600 font-medium">
                    ● OK : {stock.filter(s => s.statut === "ok").length}
                  </span>
                  <span className="text-orange-600 font-medium">
                    ● Alerte : {stock.filter(s => s.statut === "alerte").length}
                  </span>
                  <span className="text-red-600 font-medium">
                    ● Rupture : {stock.filter(s => s.statut === "rupture").length}
                  </span>
                  <span className="ml-auto font-bold">
                    Valeur totale : {fmt(stock.reduce((s, a) => s + a.valeur_stock, 0))}
                  </span>
                </div>
                <div className="border border-border rounded-xl overflow-hidden">
                  <table className="w-full text-sm">
                    <thead className="bg-muted text-xs">
                      <tr>
                        <th className="text-left px-4 py-2.5">Article</th>
                        <th className="text-left px-3 py-2.5">Dépôt</th>
                        <th className="text-right px-4 py-2.5">Quantité</th>
                        <th className="text-right px-4 py-2.5">P.A. (F)</th>
                        <th className="text-right px-4 py-2.5">Valeur (F)</th>
                        <th className="text-center px-4 py-2.5">Statut</th>
                      </tr>
                    </thead>
                    <tbody>
                      {stock.map((a, i) => (
                        <tr key={i}
                          className={`border-t border-border ${i % 2 === 1 ? "bg-muted/20" : ""} ${
                            a.statut === "rupture" ? "bg-red-50" : ""
                          }`}>
                          <td className="px-4 py-2 font-medium">{a.nom}</td>
                          <td className="px-3 py-2 text-xs text-muted-foreground">{a.depot}</td>
                          <td className="px-4 py-2 text-right">
                            {a.quantite % 1 === 0 ? a.quantite : a.quantite.toFixed(2)} {a.unite}
                          </td>
                          <td className="px-4 py-2 text-right text-muted-foreground">
                            {a.prix_achat > 0 ? fmt(a.prix_achat) : "—"}
                          </td>
                          <td className="px-4 py-2 text-right font-medium">
                            {a.valeur_stock > 0 ? fmt(a.valeur_stock) : "—"}
                          </td>
                          <td className="px-4 py-2 text-center">
                            <span className={`text-xs font-medium ${
                              a.statut === "rupture" ? "text-red-600" :
                              a.statut === "alerte"  ? "text-orange-600" :
                              "text-green-600"
                            }`}>
                              {a.statut === "rupture" ? "Rupture" :
                               a.statut === "alerte"  ? "Alerte" : "OK"}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {/* ---- Créances par ancienneté ---- */}
            {onglet === "creances" && creances && (
              <div className="space-y-4">
                <div className="flex gap-2">
                  <Button size="sm" variant="outline" onClick={async () => {
                    const tranches = [
                      { label: "Moins de 30 jours",   data: creances.moins_30j    },
                      { label: "30 à 60 jours",       data: creances.tranche_30_60 },
                      { label: "60 à 90 jours",       data: creances.tranche_60_90 },
                      { label: "Plus de 90 jours",    data: creances.plus_90j     },
                    ];
                    const lignes = tranches.map(t =>
                      `<tr><td>${t.label}</td><td class="droite">${t.data.nb}</td>
                       <td class="droite">${fmt(t.data.montant)}</td></tr>`
                    ).join("");
                    const html = genererHTMLRapport("Créances par ancienneté", `
                      <table>
                        <thead><tr>
                          <th>Tranche</th>
                          <th class="droite">Nb créances</th>
                          <th class="droite">Montant (F)</th>
                        </tr></thead>
                        <tbody>${lignes}</tbody>
                      </table>`);
                    await imprimerRapport(html, "rapport_creances");
                  }}>
                    <Printer className="h-3.5 w-3.5 mr-1.5" /> Imprimer
                  </Button>
                  <Button size="sm" variant="outline" onClick={async () => {
                    await exporterExcel([{ nom: "Créances", donnees: [
                      { "Tranche": "< 30j",  "Nb": creances.moins_30j.nb,     "Montant (F)": creances.moins_30j.montant },
                      { "Tranche": "30-60j", "Nb": creances.tranche_30_60.nb, "Montant (F)": creances.tranche_30_60.montant },
                      { "Tranche": "60-90j", "Nb": creances.tranche_60_90.nb, "Montant (F)": creances.tranche_60_90.montant },
                      { "Tranche": "> 90j",  "Nb": creances.plus_90j.nb,      "Montant (F)": creances.plus_90j.montant },
                    ]}], "rapport_creances.xlsx");
                  }}>
                    <Download className="h-3.5 w-3.5 mr-1.5" /> Excel
                  </Button>
                </div>

                {[
                  { label: "Moins de 30 jours",  data: creances.moins_30j,     couleur: "bg-gray-50 border-gray-200" },
                  { label: "30 à 60 jours",      data: creances.tranche_30_60, couleur: "bg-yellow-50 border-yellow-200" },
                  { label: "60 à 90 jours",      data: creances.tranche_60_90, couleur: "bg-orange-50 border-orange-200" },
                  { label: "Plus de 90 jours",   data: creances.plus_90j,      couleur: "bg-red-50 border-red-200" },
                ].map(t => (
                  <div key={t.label}
                    className={`border rounded-xl px-5 py-4 ${t.couleur} flex items-center justify-between`}>
                    <div>
                      <p className="text-sm font-semibold">{t.label}</p>
                      <p className="text-xs text-muted-foreground">
                        {t.data.nb} créance{t.data.nb > 1 ? "s" : ""}
                      </p>
                    </div>
                    <p className="text-xl font-bold">{fmt(t.data.montant)}</p>
                  </div>
                ))}

                <div className="border-t border-border pt-4">
                  <div className="flex justify-between text-sm font-bold">
                    <span>Total créances</span>
                    <span>{fmt(
                      creances.moins_30j.montant +
                      creances.tranche_30_60.montant +
                      creances.tranche_60_90.montant +
                      creances.plus_90j.montant
                    )}</span>
                  </div>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}