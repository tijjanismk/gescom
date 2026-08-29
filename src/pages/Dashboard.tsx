import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  TrendingUp, ShoppingCart, Users, Wallet,
  AlertTriangle, FileText, Clock, CheckCircle2,
  Package, ArrowUpRight, ArrowDownRight,
  Receipt, Gift, Loader2, RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { UTILISATEUR_ACTIF, DEPOT_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface ResumeDashboard {
  ca_jour: number;
  ca_semaine: number;
  ca_mois: number;
  ca_mois_precedent: number;
  nb_ventes_jour: number;
  nb_ventes_mois: number;
  total_creances: number;
  nb_creances_ouvertes: number;
  nb_creances_en_retard: number;
  total_avoirs_ouverts: number;
  stock_ruptures: number;
  stock_alertes: number;
  caisse_solde: number;
  caisse_session_ouverte: boolean;
  factures_brouillon: number;
  commandes_en_attente: number;
}

interface VenteJour {
  heure: string;
  montant: number;
  nb: number;
}

interface TopClient {
  nom: string; code: string; ca: number; nb_ventes: number;
}

interface TopArticle {
  nom: string; qte_vendue: number; ca: number; unite: string;
}

// =====================================================================
//  Utilitaires
// =====================================================================

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
// Montants TOUJOURS en entier, jamais abreges.
//
// "1,2M F" cache la difference entre 1 150 000 et 1 249 999 — sur des
// FCFA, l'ecart depasse le CA d'une journee. Un commercant qui compare
// son tableau de bord a sa caisse doit voir le meme nombre.
function fmtCompact(n: number) {
  return fmt(n);
}
function pct(a: number, b: number) {
  if (b === 0) return a > 0 ? 100 : 0;
  return Math.round(((a - b) / b) * 100);
}

// =====================================================================
//  KPI Card
// =====================================================================

function KpiCard({
  titre, valeur, sous, icone: Icone, couleur, tendance, onClick,
}: {
  titre: string; valeur: string; sous?: string;
  icone: React.ElementType; couleur: string;
  tendance?: number; onClick?: () => void;
}) {
  return (
    <div
      onClick={onClick}
      className={`bg-card border border-border rounded-xl p-4 space-y-3
                  ${onClick ? "cursor-pointer hover:shadow-md hover:border-primary/30 transition-all" : ""}`}>
      <div className="flex items-start justify-between">
        <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${couleur}`}>
          <Icone className="h-5 w-5" />
        </div>
        {tendance !== undefined && (
          <div className={`flex items-center gap-1 text-xs font-medium ${
            tendance >= 0 ? "text-green-600" : "text-red-500"
          }`}>
            {tendance >= 0
              ? <ArrowUpRight className="h-3.5 w-3.5" />
              : <ArrowDownRight className="h-3.5 w-3.5" />
            }
            {Math.abs(tendance)}%
          </div>
        )}
      </div>
      <div>
        <p className="text-2xl font-bold tracking-tight">{valeur}</p>
        <p className="text-xs text-muted-foreground mt-0.5">{titre}</p>
        {sous && <p className="text-xs text-muted-foreground">{sous}</p>}
      </div>
    </div>
  );
}

// =====================================================================
//  Mini barre de graphe
// =====================================================================

function MiniBar({ valeur, max, couleur = "bg-primary" }: {
  valeur: number; max: number; couleur?: string;
}) {
  const pct = max > 0 ? Math.round((valeur / max) * 100) : 0;
  return (
    <div className="flex-1 bg-muted rounded-full h-1.5">
      <div
        className={`${couleur} h-1.5 rounded-full transition-all`}
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

// =====================================================================
//  Dashboard
// =====================================================================

export function Dashboard() {
  const [resume, setResume] = useState<ResumeDashboard | null>(null);
  const [ventesJour, setVentesJour] = useState<VenteJour[]>([]);
  const [topClients, setTopClients] = useState<TopClient[]>([]);
  const [topArticles, setTopArticles] = useState<TopArticle[]>([]);
  const [chargement, setChargement] = useState(true);
  const [derniereActu, setDerniereActu] = useState<Date>(new Date());

  const estPatron = UTILISATEUR_ACTIF?.role === "patron";

  async function charger() {
    setChargement(true);
    try {
      const [res, vj, tc, ta] = await Promise.all([
        invoke<ResumeDashboard>("lire_resume_dashboard", { depotId: DEPOT_ACTIF }),
        invoke<VenteJour[]>("lire_ventes_du_jour"),
        estPatron ? invoke<TopClient[]>("lire_top_clients") : Promise.resolve([]),
        estPatron ? invoke<TopArticle[]>("lire_top_articles") : Promise.resolve([]),
      ]);
      setResume(res);
      setVentesJour(vj);
      setTopClients(tc);
      setTopArticles(ta);
      setDerniereActu(new Date());
    } catch (e) {
      console.error("Erreur dashboard :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  if (chargement && !resume) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // Le `resume!` d'origine partait du principe qu'un chargement termine
  // signifie des donnees presentes. Si la commande echoue, `resume`
  // reste null et l'ecran plante sur `r.ca_mois` — le vrai probleme
  // (l'erreur de chargement) devient alors invisible.
  if (!resume) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 p-6">
        <AlertTriangle className="h-8 w-8 text-muted-foreground" />
        <p className="text-sm text-muted-foreground text-center max-w-sm">
          Impossible de charger le tableau de bord.
          Voir la console pour le détail.
        </p>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Réessayer
        </Button>
      </div>
    );
  }

  const r = resume;
  const tendanceMois = pct(r.ca_mois, r.ca_mois_precedent);
  const maxVente = Math.max(...ventesJour.map(v => v.montant), 1);
  const maxClient = Math.max(...topClients.map(c => c.ca), 1);
  const maxArticle = Math.max(...topArticles.map(a => a.ca), 1);

  return (
    <div className="flex-1 overflow-auto">
      <div className="p-6 space-y-6 max-w-7xl mx-auto">

        {/* ── En-tête ── */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold">
              Bonjour, {UTILISATEUR_ACTIF?.nom?.split(" ")[0] ?? "..."} 👋
            </h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              {new Date().toLocaleDateString("fr-ML", {
                weekday: "long", day: "numeric",
                month: "long", year: "numeric",
              })}
            </p>
          </div>
          <button onClick={charger}
            className="flex items-center gap-2 text-xs text-muted-foreground
                       hover:text-foreground transition-colors">
            <RefreshCw className="h-3.5 w-3.5" />
            <span>
              Mis à jour à {derniereActu.toLocaleTimeString("fr-ML", {
                hour: "2-digit", minute: "2-digit"
              })}
            </span>
          </button>
        </div>

        {/* ── Alertes ── */}
        {(r.nb_creances_en_retard > 0 || r.stock_ruptures > 0 ||
          r.factures_brouillon > 0) && (
          <div className="flex gap-2 flex-wrap">
            {r.nb_creances_en_retard > 0 && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg
                              bg-red-50 border border-red-200 text-sm text-red-700">
                <AlertTriangle className="h-4 w-4" />
                <span><strong>{r.nb_creances_en_retard}</strong> créance{r.nb_creances_en_retard > 1 ? "s" : ""} en retard</span>
              </div>
            )}
            {r.factures_brouillon > 0 && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg
                              bg-orange-50 border border-orange-200 text-sm text-orange-700">
                <FileText className="h-4 w-4" />
                <span><strong>{r.factures_brouillon}</strong> facture{r.factures_brouillon > 1 ? "s" : ""} à valider</span>
              </div>
            )}
            {r.commandes_en_attente > 0 && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg
                              bg-blue-50 border border-blue-200 text-sm text-blue-700">
                <Clock className="h-4 w-4" />
                <span><strong>{r.commandes_en_attente}</strong> commande{r.commandes_en_attente > 1 ? "s" : ""} en attente</span>
              </div>
            )}
            {r.stock_ruptures > 0 && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg
                              bg-yellow-50 border border-yellow-200 text-sm text-yellow-700">
                <Package className="h-4 w-4" />
                <span><strong>{r.stock_ruptures}</strong> article{r.stock_ruptures > 1 ? "s" : ""} en rupture</span>
              </div>
            )}
          </div>
        )}

        {/* ── KPIs principaux ── */}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <KpiCard
            titre="CA aujourd'hui"
            valeur={fmtCompact(r.ca_jour)}
            sous={`${r.nb_ventes_jour} vente${r.nb_ventes_jour > 1 ? "s" : ""}`}
            icone={ShoppingCart}
            couleur="bg-primary/10 text-primary"
          />
          <KpiCard
            titre="CA ce mois"
            valeur={fmtCompact(r.ca_mois)}
            sous={`${r.nb_ventes_mois} ventes`}
            icone={TrendingUp}
            couleur="bg-green-100 text-green-700"
            tendance={tendanceMois}
          />
          {estPatron && (
            <KpiCard
              titre="Créances ouvertes"
              valeur={fmtCompact(r.total_creances)}
              sous={`${r.nb_creances_ouvertes} client${r.nb_creances_ouvertes > 1 ? "s" : ""}`}
              icone={r.nb_creances_en_retard > 0 ? AlertTriangle : Users}
              couleur={r.nb_creances_en_retard > 0
                ? "bg-red-100 text-red-600"
                : "bg-orange-100 text-orange-700"
              }
            />
          )}
          <KpiCard
            titre="Caisse"
            valeur={fmtCompact(r.caisse_solde)}
            sous={r.caisse_session_ouverte ? "Session ouverte" : "Session fermée"}
            icone={Wallet}
            couleur={r.caisse_session_ouverte
              ? "bg-green-100 text-green-700"
              : "bg-gray-100 text-gray-500"
            }
          />
        </div>

        {/* ── KPIs secondaires ── */}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <div className="bg-card border border-border rounded-xl p-4">
            <div className="flex items-center gap-2 mb-2">
              <Receipt className="h-4 w-4 text-purple-600" />
              <p className="text-xs font-medium text-muted-foreground">Factures brouillon</p>
            </div>
            <p className="text-2xl font-bold">{r.factures_brouillon}</p>
            <p className="text-xs text-muted-foreground">à valider</p>
          </div>
          <div className="bg-card border border-border rounded-xl p-4">
            <div className="flex items-center gap-2 mb-2">
              <FileText className="h-4 w-4 text-blue-600" />
              <p className="text-xs font-medium text-muted-foreground">Commandes</p>
            </div>
            <p className="text-2xl font-bold">{r.commandes_en_attente}</p>
            <p className="text-xs text-muted-foreground">en attente de transfert</p>
          </div>
          {estPatron && (
            <div className="bg-card border border-border rounded-xl p-4">
              <div className="flex items-center gap-2 mb-2">
                <Gift className="h-4 w-4 text-green-600" />
                <p className="text-xs font-medium text-muted-foreground">Avoirs disponibles</p>
              </div>
              <p className="text-2xl font-bold">{fmtCompact(r.total_avoirs_ouverts)}</p>
              <p className="text-xs text-muted-foreground">à appliquer</p>
            </div>
          )}
          <div className="bg-card border border-border rounded-xl p-4">
            <div className="flex items-center gap-2 mb-2">
              <Package className={`h-4 w-4 ${r.stock_ruptures > 0 ? "text-red-500" : "text-gray-400"}`} />
              <p className="text-xs font-medium text-muted-foreground">Stock</p>
            </div>
            <p className={`text-2xl font-bold ${r.stock_ruptures > 0 ? "text-red-500" : ""}`}>
              {r.stock_ruptures}
            </p>
            <p className="text-xs text-muted-foreground">
              rupture{r.stock_ruptures > 1 ? "s" : ""}
              {r.stock_alertes > 0 ? ` · ${r.stock_alertes} alerte${r.stock_alertes > 1 ? "s" : ""}` : ""}
            </p>
          </div>
        </div>

        {/* ── Graphe ventes du jour ── */}
        {ventesJour.length > 0 && (
          <div className="bg-card border border-border rounded-xl p-5">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-semibold">Ventes aujourd'hui par heure</h2>
              <span className="text-xs text-muted-foreground">
                Total : {fmt(r.ca_jour)}
              </span>
            </div>
            <div className="flex items-end gap-1 h-24 bg-transparent">
              {ventesJour.map((v, i) => {
                const h = Math.round((v.montant / maxVente) * 100);
                return (
                  <div key={i} className="flex-1 flex flex-col items-center gap-1 group">
                    <div className="relative w-full">
                      {v.montant > 0 && (
                        <div className="absolute -top-5 left-1/2 -translate-x-1/2
                                        text-[9px] text-muted-foreground whitespace-nowrap
                                        opacity-0 group-hover:opacity-100 transition-opacity">
                          {fmtCompact(v.montant)}
                        </div>
                      )}
                      <div
                        className={`w-full rounded-sm transition-all ${
                          v.montant > 0 ? "bg-primary/80 hover:bg-primary" : "bg-muted"
                        }`}
                        style={{ height: `${Math.max(h, v.montant > 0 ? 4 : 2)}px` }}
                      />
                    </div>
                    <span className="text-[9px] text-muted-foreground">{v.heure}h</span>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ── Top clients + Top articles ── */}
        {estPatron && (topClients.length > 0 || topArticles.length > 0) && (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">

            {/* Top clients */}
            {topClients.length > 0 && (
              <div className="bg-card border border-border rounded-xl p-5">
                <h2 className="text-sm font-semibold mb-4">Top clients — ce mois</h2>
                <div className="space-y-3">
                  {topClients.slice(0, 5).map((c, i) => (
                    <div key={i} className="space-y-1">
                      <div className="flex items-center justify-between text-sm">
                        <div className="flex items-center gap-2 min-w-0">
                          <span className="text-xs font-bold text-muted-foreground w-4">
                            {i + 1}
                          </span>
                          <span className="truncate font-medium">{c.nom}</span>
                        </div>
                        <span className="font-semibold shrink-0 ml-2">
                          {fmtCompact(c.ca)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="w-4" />
                        <MiniBar valeur={c.ca} max={maxClient} />
                        <span className="text-xs text-muted-foreground shrink-0">
                          {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Top articles */}
            {topArticles.length > 0 && (
              <div className="bg-card border border-border rounded-xl p-5">
                <h2 className="text-sm font-semibold mb-4">Top articles — ce mois</h2>
                <div className="space-y-3">
                  {topArticles.slice(0, 5).map((a, i) => (
                    <div key={i} className="space-y-1">
                      <div className="flex items-center justify-between text-sm">
                        <div className="flex items-center gap-2 min-w-0">
                          <span className="text-xs font-bold text-muted-foreground w-4">
                            {i + 1}
                          </span>
                          <span className="truncate font-medium">{a.nom}</span>
                        </div>
                        <span className="font-semibold shrink-0 ml-2">
                          {fmtCompact(a.ca)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="w-4" />
                        <MiniBar valeur={a.ca} max={maxArticle} couleur="bg-green-500" />
                        <span className="text-xs text-muted-foreground shrink-0">
                          {a.qte_vendue % 1 === 0 ? a.qte_vendue : a.qte_vendue.toFixed(1)} {a.unite}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* ── CA semaine ── */}
        {estPatron && (
          <div className="bg-card border border-border rounded-xl p-5">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold">Résumé de la semaine</h2>
                <p className="text-xs text-muted-foreground mt-0.5">
                  CA : <strong>{fmt(r.ca_semaine)}</strong>
                </p>
              </div>
              <div className="text-right">
                <p className="text-xs text-muted-foreground">Mois dernier</p>
                <p className="text-sm font-medium">{fmtCompact(r.ca_mois_precedent)}</p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}