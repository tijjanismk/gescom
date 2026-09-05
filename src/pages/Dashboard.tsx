import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  TrendingUp, ShoppingCart, Users, Wallet,
  AlertTriangle, FileText, Clock,
  Package, ArrowUpRight, ArrowDownRight,
  Receipt, Gift, Loader2, RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { GlassHalos } from "@/components/ui/GlassIcon";
import { KpiCard, KpiPetit, CARTE, GRILLE } from "@/components/ui/KpiVerre";
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
  // Ventes à découvert : marchandise sortie au-delà du stock connu.
  // Chacune signale soit un stock faux, soit une entrée non saisie.
  const [nbDecouverts, setNbDecouverts] = useState(0);
  const [ventesJour, setVentesJour] = useState<VenteJour[]>([]);
  const [topClients, setTopClients] = useState<TopClient[]>([]);
  const [topArticles, setTopArticles] = useState<TopArticle[]>([]);
  const [chargement, setChargement] = useState(true);
  const [derniereActu, setDerniereActu] = useState<Date>(new Date());

  const estPatron = UTILISATEUR_ACTIF?.role === "patron";

  async function charger() {
    setChargement(true);
    try {
      const auj = new Date().toISOString().slice(0, 10);
      const [res, vj, tc, ta, dec] = await Promise.all([
        invoke<ResumeDashboard>("lire_resume_dashboard", { depotId: DEPOT_ACTIF }),
        invoke<VenteJour[]>("lire_ventes_du_jour"),
        estPatron ? invoke<TopClient[]>("lire_top_clients") : Promise.resolve([]),
        estPatron ? invoke<TopArticle[]>("lire_top_articles") : Promise.resolve([]),
        invoke<{ nb: number }>("lire_ventes_a_decouvert", {
          dateDebut: auj, dateFin: auj,
        }).catch(() => ({ nb: 0 })),
      ]);
      setResume(res);
      setNbDecouverts(dec?.nb ?? 0);
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
    <div className="flex-1 overflow-auto relative"
         style={{ fontFamily: '"Archivo Variable", Archivo, system-ui, sans-serif' }}>
      {/* Le verre ne se lit pas sur du blanc plat. */}
      <GlassHalos />
      <div className="relative z-[1] p-6 space-y-6 max-w-7xl mx-auto">

        {/* ── En-tête ── */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold">
              Bonjour, {UTILISATEUR_ACTIF?.nom?.split(" ")[0] ?? "..."} 
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
          r.factures_brouillon > 0 || nbDecouverts > 0) && (
          <div className="flex gap-2 flex-wrap">
            {r.nb_creances_en_retard > 0 && (
              <div className="flex items-center gap-2 px-3 py-2
                              bg-red-50 border border-red-200 text-sm text-red-700">
                <AlertTriangle className="h-4 w-4" />
                <span><strong>{r.nb_creances_en_retard}</strong> créance{r.nb_creances_en_retard > 1 ? "s" : ""} en retard</span>
              </div>
            )}
            {r.factures_brouillon > 0 && (
              <div className="flex items-center gap-2 px-3 py-2
                              bg-orange-50 border border-orange-200 text-sm text-orange-700">
                <FileText className="h-4 w-4" />
                <span><strong>{r.factures_brouillon}</strong> facture{r.factures_brouillon > 1 ? "s" : ""} à valider</span>
              </div>
            )}
            {r.commandes_en_attente > 0 && (
              <div className="flex items-center gap-2 px-3 py-2
                              bg-blue-50 border border-blue-200 text-sm text-blue-700">
                <Clock className="h-4 w-4" />
                <span><strong>{r.commandes_en_attente}</strong> commande{r.commandes_en_attente > 1 ? "s" : ""} en attente</span>
              </div>
            )}
            {/* Juste avant les ruptures : les deux disent la même
                chose du stock, mais le découvert est plus grave — la
                marchandise est déjà partie. */}
            {nbDecouverts > 0 && (
              <div className="flex items-center gap-2 px-3 py-2
                              bg-orange-50 border border-orange-300 text-sm text-orange-800"
                title="Vendu au-delà du stock connu : régulariser par une entrée, un achat ou un ajustement">
                <AlertTriangle className="h-4 w-4" />
                <span>
                  <strong>{nbDecouverts}</strong> vente{nbDecouverts > 1 ? "s" : ""} à découvert
                </span>
              </div>
            )}
            {r.stock_ruptures > 0 && (
              <div className="flex items-center gap-2 px-3 py-2
                              bg-yellow-50 border border-yellow-200 text-sm text-yellow-700">
                <Package className="h-4 w-4" />
                <span><strong>{r.stock_ruptures}</strong> article{r.stock_ruptures > 1 ? "s" : ""} en rupture</span>
              </div>
            )}
          </div>
        )}

        {/* ── KPIs principaux ── */}
        <div style={GRILLE}>
          <KpiCard
            titre="CA aujourd'hui"
            valeur={fmtCompact(r.ca_jour)}
            sous={`${r.nb_ventes_jour} vente${r.nb_ventes_jour > 1 ? "s" : ""}`}
            icone={ShoppingCart}
            variante="tinted"
          />
          <KpiCard
            titre="CA ce mois"
            valeur={fmtCompact(r.ca_mois)}
            sous={`${r.nb_ventes_mois} ventes`}
            icone={TrendingUp}
            variante="neutral"
            tendance={tendanceMois}
            tendanceIcones={[ArrowUpRight, ArrowDownRight]}
          />
          {estPatron && (
            <KpiCard
              titre="Créances ouvertes"
              valeur={fmtCompact(r.total_creances)}
              sous={`${r.nb_creances_ouvertes} client${r.nb_creances_ouvertes > 1 ? "s" : ""}`}
              icone={r.nb_creances_en_retard > 0 ? AlertTriangle : Users}
              variante={r.nb_creances_en_retard > 0 ? "tinted" : "clear"}
            />
          )}
          {/* Session fermee = tuile inactive : D46, aucune operation
              d'argent n'est acceptee dans cet etat. */}
          <KpiCard
            titre="Caisse"
            valeur={fmtCompact(r.caisse_solde)}
            sous={r.caisse_session_ouverte ? "Session ouverte" : "Session fermée"}
            icone={Wallet}
            variante="neutral"
            inactif={!r.caisse_session_ouverte}
          />
        </div>

        {/* ── KPIs secondaires ── */}
        <div style={GRILLE}>
          <KpiPetit
            titre="Factures brouillon"
            valeur={String(r.factures_brouillon)}
            sous="à valider"
            icone={Receipt}
          />
          <KpiPetit
            titre="Commandes"
            valeur={String(r.commandes_en_attente)}
            sous="en attente de transfert"
            icone={FileText}
          />
          {estPatron && (
            <KpiPetit
              titre="Avoirs disponibles"
              valeur={fmtCompact(r.total_avoirs_ouverts)}
              sous="à appliquer"
              icone={Gift}
            />
          )}
          <KpiPetit
            titre="Stock"
            valeur={String(r.stock_ruptures)}
            sous={`rupture${r.stock_ruptures > 1 ? "s" : ""}` +
                  (r.stock_alertes > 0
                    ? ` · ${r.stock_alertes} alerte${r.stock_alertes > 1 ? "s" : ""}`
                    : "")}
            icone={Package}
            variante={r.stock_ruptures > 0 ? "tinted" : "clear"}
            alerte={r.stock_ruptures > 0}
          />
        </div>

        {/* ── Graphe ventes du jour ── */}
        {ventesJour.length > 0 && (
          <div className={`${CARTE} p-5`}>
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
              <div className={`${CARTE} p-5`}>
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
              <div className={`${CARTE} p-5`}>
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
          <div className={`${CARTE} p-5`}>
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