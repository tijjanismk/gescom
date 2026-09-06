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

type Periode = "jour" | "semaine" | "mois" | "annee";

/** Une barre du graphe, quelle que soit l'échelle. */
interface PointVente {
  // Le libellé est fabriqué par le backend : « 14h », « Mar », « S2 »,
  // « Avr ». Le composer ici aurait demandé de connaître l'échelle à
  // l'affichage, et de refaire les noms de jours et de mois en français.
  label: string;
  montant: number;
  nb: number;
}

interface VentesPeriode {
  points: PointVente[];
  total: number;
  nb: number;
  periode: string;
}

const PERIODES: { cle: Periode; label: string }[] = [
  { cle: "jour",    label: "Jour" },
  { cle: "semaine", label: "Semaine" },
  { cle: "mois",    label: "Mois" },
  { cle: "annee",   label: "Année" },
];

const TITRE_PERIODE: Record<Periode, string> = {
  jour:    "Ventes aujourd'hui, par heure",
  semaine: "Ventes des 7 derniers jours",
  mois:    "Ventes du mois, par semaine",
  annee:   "Ventes de l'année, par mois",
};

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
  // Graphe : échelle choisie par l'utilisateur, rechargée seule quand
  // elle change — inutile de refaire tout le tableau de bord pour
  // passer de la journée à la semaine.
  const [periode, setPeriode] = useState<Periode>("jour");
  const [graphe, setGraphe] = useState<VentesPeriode | null>(null);
  const [chargeGraphe, setChargeGraphe] = useState(true);
  const [topClients, setTopClients] = useState<TopClient[]>([]);
  const [topArticles, setTopArticles] = useState<TopArticle[]>([]);
  const [chargement, setChargement] = useState(true);
  const [derniereActu, setDerniereActu] = useState<Date>(new Date());

  const estPatron = UTILISATEUR_ACTIF?.role === "patron";

  async function charger() {
    setChargement(true);
    try {
      const auj = new Date().toISOString().slice(0, 10);
      const [res, tc, ta, dec] = await Promise.all([
        invoke<ResumeDashboard>("lire_resume_dashboard", { depotId: DEPOT_ACTIF }),
        estPatron ? invoke<TopClient[]>("lire_top_clients") : Promise.resolve([]),
        estPatron ? invoke<TopArticle[]>("lire_top_articles") : Promise.resolve([]),
        invoke<{ nb: number }>("lire_ventes_a_decouvert", {
          dateDebut: auj, dateFin: auj,
        }).catch(() => ({ nb: 0 })),
      ]);
      setResume(res);
      setNbDecouverts(dec?.nb ?? 0);
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

  // Le graphe se recharge seul quand l'échelle change : refaire tout le
  // tableau de bord pour passer de la journée à la semaine ferait
  // clignoter des chiffres qui, eux, n'ont pas bougé.
  useEffect(() => {
    let annule = false;
    setChargeGraphe(true);
    invoke<VentesPeriode>("lire_ventes_periode", {
      periode, depotId: DEPOT_ACTIF,
    })
      .then(g => { if (!annule) setGraphe(g); })
      .catch(e => console.error("Erreur graphe :", e))
      .finally(() => { if (!annule) setChargeGraphe(false); });
    return () => { annule = true; };
  }, [periode, derniereActu]);

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
  const points = graphe?.points ?? [];
  const totalPeriode = graphe?.total ?? 0;
  // `, 1` garde la division sûre quand aucune vente n'a encore eu lieu.
  const maxPoint = Math.max(...points.map(p => p.montant), 1);
  // Le pic, ou `null` si la période n'a rien encaissé — sans ce cas,
  // `max` valant 1 par défaut désignerait une barre au hasard.
  const pic = points.some(p => p.montant > 0)
    ? points.find(p => p.montant === maxPoint)!
    : null;
  const maxClient = Math.max(...topClients.map(c => c.ca), 1);
  const maxArticle = Math.max(...topArticles.map(a => a.ca), 1);

  return (
    // `overflow-x-hidden` et non `overflow-auto` : les halos de fond
    // sont posés en `inset:-10%`, donc ils débordent de 10 % à droite du
    // conteneur et y créaient une barre de défilement horizontale. Le
    // débordement est voulu — c'est ce qui donne au flou de ne pas
    // s'arrêter net au bord — il ne doit juste pas être scrollable.
    <div className="flex-1 overflow-y-auto overflow-x-hidden relative"
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

        {/* ── Graphe des ventes, par période ── */}
        <div className={`${CARTE} p-5`}>
          <div className="flex items-baseline justify-between gap-3 flex-wrap">
            <h2 className="text-sm font-semibold">{TITRE_PERIODE[periode]}</h2>
            <span className="text-xs text-muted-foreground shrink-0">
              Total : {fmt(totalPeriode)}
            </span>
          </div>

          {/* Le pic en clair : c'est l'information qu'on cherche dans ce
              graphe — quand ça se joue. La lire en survolant les barres
              une à une serait absurde. */}
          <p className="text-xs text-muted-foreground mt-1">
            {pic === null
              ? "Aucune vente sur cette période"
              : `Meilleur${periode === "jour" ? "e heure" : " moment"} : `
                + `${pic.label} · ${fmt(pic.montant)}`}
          </p>

          {/* Sélecteur d'échelle. Le filtre se pose au-dessus du graphe,
              là où l'œil arrive avant de lire les barres. */}
          <div className="flex gap-1 mt-3 mb-4">
            {PERIODES.map(p => (
              <button key={p.cle} onClick={() => setPeriode(p.cle)}
                className={`px-2.5 py-1 rounded-md text-xs font-medium
                            transition-colors ${
                  periode === p.cle
                    ? "bg-sky-100 text-sky-800 border border-sky-300"
                    : "text-muted-foreground border border-transparent hover:bg-muted"
                }`}>
                {p.label}
              </button>
            ))}
          </div>

          {chargeGraphe ? (
            <div className="h-48 flex items-center justify-center">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          ) : (
            <>
              {/* Hauteur fixe, barres en POURCENTAGE de ce conteneur.
                  Un pourcentage écrit en `px` faisait déborder la barre
                  du pic hors de la carte. */}
              <div className="flex items-end gap-[3px] h-48">
                {points.map((v, i) => {
                  const pct = (v.montant / maxPoint) * 100;
                  const estPic = v.montant > 0 && v.montant === maxPoint;
                  return (
                    <div key={i}
                      className="flex-1 h-full flex items-end justify-center group
                                 relative min-w-0">
                      {/* Infobulle : montant ET nombre de ventes. Le seul
                          montant ne dit pas si la période a fait une
                          grosse vente ou dix petites. */}
                      {v.montant > 0 && (
                        <div className="absolute bottom-full mb-1 left-1/2 -translate-x-1/2
                                        px-2 py-1 rounded-md bg-foreground text-background
                                        text-[10px] leading-tight whitespace-nowrap
                                        opacity-0 group-hover:opacity-100 pointer-events-none
                                        transition-opacity z-10 shadow-sm">
                          <span className="font-semibold">{fmt(v.montant)}</span>
                          <span className="opacity-70">
                            {" · "}{v.nb} vente{v.nb > 1 ? "s" : ""}
                          </span>
                        </div>
                      )}
                      <div
                        className={`w-full rounded-t-[4px] transition-colors ${
                          v.montant > 0
                            ? estPic
                              ? "bg-sky-600"
                              : "bg-sky-500/55 group-hover:bg-sky-600"
                            : "bg-muted/60"
                        }`}
                        // Plancher de 3px : une heure à 200 F doit rester
                        // visible à côté d'une heure à 200 000 F, sinon
                        // elle se confond avec une période sans vente.
                        style={{
                          height: v.montant > 0 ? `max(3px, ${pct}%)` : "2px",
                        }}
                      />
                    </div>
                  );
                })}
              </div>

              {/* Axe. Sur la journée, une étiquette sur trois : dix-huit
                  nombres à 9px collés ne se lisent pas. Sur les autres
                  échelles il y a peu de barres, on les nomme toutes. */}
              <div className="flex gap-[3px] mt-1.5 border-t border-border/60 pt-1.5">
                {points.map((v, i) => {
                  const montrer = periode !== "jour"
                    || i % 3 === 0 || v.label === pic?.label;
                  return (
                    <span key={i}
                      className={`flex-1 text-center text-[9px] tabular-nums min-w-0 ${
                        v.label === pic?.label
                          ? "text-foreground font-medium"
                          : "text-muted-foreground"
                      }`}>
                      {montrer ? v.label : ""}
                    </span>
                  );
                })}
              </div>
            </>
          )}
        </div>

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