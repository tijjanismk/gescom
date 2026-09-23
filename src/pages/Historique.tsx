// pages/Historique.tsx — le journal métier, enfin lu (v3, B-1).
//
// « Qui a annulé le règlement de Coulibaly mardi ? » : la réponse est
// dans la base depuis la v1. Ici, chaque ligne dit QUAND, QUI, QUOI,
// SUR QUOI, et ce qui a changé (avant → après).
//
// On arrive ici par le menu, ou depuis une fiche (client, fournisseur,
// pièce, article) : l'écran s'ouvre alors filtré sur elle, et le filtre
// se retire d'un clic.
//
// Le serveur refuse sans `journal:lire` ; le menu ne fait que cacher.

import { useState, useEffect, useCallback, useRef } from "react";
import { appeler as invoke } from "@/lib/pont";
import { History, RefreshCw, Loader2, X, ArrowLeft, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import { Pagination } from "@/components/Pagination";

/** Ce sur quoi l'écran s'ouvre depuis une fiche. */
export interface FiltreHistorique {
  tiers_id?: string;
  piece_id?: string;
  article_id?: string;
  type_evenement?: string;
  du?: string;
  au?: string;
  /** Seulement les anomalies que personne n'a marquées vues (B-4). */
  a_verifier?: boolean;
  /** Ce que dit la puce du filtre : « Client : Coulibaly ». */
  libelle?: string;
}

interface Ligne {
  id: string;
  date: string;
  type: string;
  libelle_type: string;
  entite_type: string;
  entite_id: string;
  auteur_id: string | null;
  auteur_nom: string | null;
  tiers_id: string | null;
  tiers_nom: string | null;
  piece_id: string | null;
  piece_numero: string | null;
  article_id: string | null;
  article_nom: string | null;
  ancien: unknown;
  nouveau: unknown;
  /** Une anomalie marquée vue : par qui, quand. */
  vue: { le: string; par_nom: string | null } | null;
}

interface Page {
  total: number;
  page: number;
  par_page: number;
  lignes: Ligne[];
}

interface Filtres {
  types: { type: string; libelle: string }[];
  auteurs: { id: string; nom: string }[];
}

const PAR_PAGE = 50;

const TH = "text-left text-xs font-semibold text-muted-foreground px-3 py-2";
const TD = "px-3 py-2 text-sm align-top";

function quand(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("fr-ML", { day: "2-digit", month: "2-digit", year: "numeric" }) +
    " " + d.toLocaleTimeString("fr-ML", { hour: "2-digit", minute: "2-digit" });
}

const ARGENT = /montant|total|prix|reste|solde|rendu|verse|encaisse/i;

function valeur(cle: string, v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "boolean") return v ? "oui" : "non";
  if (typeof v === "number") {
    if (cle.endsWith("_pct")) return `${v} %`;
    if (ARGENT.test(cle) && Number.isInteger(v)) return new Intl.NumberFormat("fr-ML").format(v) + " F";
    return String(v);
  }
  if (typeof v === "object") return JSON.stringify(v);
  return String(v).replace(/_/g, " ");
}

// Un identifiant interne ne dit rien au commerçant : la pièce ou le
// client qu'il désigne est déjà nommé dans « Sur quoi ».
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Le détail enregistré, en une ligne lisible. */
function resume(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v !== "object" || Array.isArray(v)) return valeur("", v);
  return Object.entries(v as Record<string, unknown>)
    .filter(([, x]) => !(typeof x === "string" && UUID.test(x)))
    .map(([k, x]) => `${k.replace(/_/g, " ")} : ${valeur(k, x)}`)
    .join(" · ");
}

function Detail({ ancien, nouveau }: { ancien: unknown; nouveau: unknown }) {
  const a = resume(ancien);
  const n = resume(nouveau);
  if (a && n) {
    return (
      <span>
        <span className="text-muted-foreground">{a}</span>
        <span className="mx-1.5">→</span>
        <span>{n}</span>
      </span>
    );
  }
  return <span>{n || a || "—"}</span>;
}

function SurQuoi({ l }: { l: Ligne }) {
  const morceaux = [l.tiers_nom, l.piece_numero, l.article_nom].filter(Boolean);
  if (morceaux.length === 0) {
    return <span className="text-muted-foreground">{l.entite_type.replace(/_/g, " ")}</span>;
  }
  return <span>{morceaux.join(" · ")}</span>;
}

interface HistoriqueProps {
  filtreInitial?: FiltreHistorique;
  /** Présent quand on vient d'une fiche : y retourner. */
  onRetour?: () => void;
}

export function Historique({ filtreInitial, onRetour }: HistoriqueProps) {
  const [contexte, setContexte] = useState<FiltreHistorique | undefined>(filtreInitial);
  const [du, setDu] = useState(filtreInitial?.du ?? "");
  const [au, setAu] = useState(filtreInitial?.au ?? "");
  const [auteur, setAuteur] = useState("");
  const [type, setType] = useState(filtreInitial?.type_evenement ?? "");
  const [saisie, setSaisie] = useState("");
  const [recherche, setRecherche] = useState("");
  const [page, setPage] = useState(0);
  const [donnees, setDonnees] = useState<Page | null>(null);
  const [filtres, setFiltres] = useState<Filtres>({ types: [], auteurs: [] });
  const [chargement, setChargement] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  // Une fiche ouverte depuis une autre fiche : on repart de son filtre.
  useEffect(() => {
    setContexte(filtreInitial);
    setType(filtreInitial?.type_evenement ?? "");
    setDu(filtreInitial?.du ?? "");
    setAu(filtreInitial?.au ?? "");
    setPage(0);
  }, [filtreInitial]);

  useEffect(() => {
    invoke<Filtres>("lire_filtres_historique")
      .then(setFiltres)
      .catch(() => { /* les listes restent vides ; la page dira l'erreur */ });
  }, []);

  // La recherche part quand on s'arrête de taper, pas à chaque touche.
  const minuteur = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    clearTimeout(minuteur.current);
    minuteur.current = setTimeout(() => { setRecherche(saisie.trim()); setPage(0); }, 300);
    return () => clearTimeout(minuteur.current);
  }, [saisie]);

  // Deux filtres changés coup sur coup lancent deux lectures ; seule la
  // DERNIÈRE a le droit d'afficher. Sans ce numéro, la réponse lente
  // de la première arrivait après et remplaçait la bonne.
  const derniere = useRef(0);

  const charger = useCallback(async () => {
    const numero = ++derniere.current;
    setChargement(true);
    setErreur(null);
    try {
      const r = await invoke<Page>("lire_historique", {
        filtre: {
          du: du || null,
          au: au || null,
          auteur_id: auteur || null,
          type_evenement: type || null,
          tiers_id: contexte?.tiers_id ?? null,
          piece_id: contexte?.piece_id ?? null,
          article_id: contexte?.article_id ?? null,
          a_verifier: contexte?.a_verifier ?? false,
          recherche: recherche || null,
          page,
          par_page: PAR_PAGE,
        },
      });
      if (numero !== derniere.current) return;
      setDonnees(r);
    } catch (e) {
      if (numero !== derniere.current) return;
      setErreur(String(e));
      setDonnees(null);
    } finally {
      if (numero === derniere.current) setChargement(false);
    }
  }, [du, au, auteur, type, contexte, recherche, page]);

  useEffect(() => { charger(); }, [charger]);

  async function marquerVue(id: string) {
    try {
      await invoke("marquer_anomalie_vue", { journalId: id });
      await charger();
    } catch (e) {
      setErreur(String(e));
    }
  }

  const aDesFiltres = !!(du || au || auteur || type || saisie || contexte?.libelle);

  function toutEffacer() {
    setDu(""); setAu(""); setAuteur(""); setType(""); setSaisie(""); setRecherche("");
    setContexte(undefined); setPage(0);
  }

  const champ = "h-9 px-2 text-sm border border-border rounded-md bg-background";

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
        <div className="flex items-center gap-2">
          {onRetour && (
            <button onClick={onRetour} aria-label="Retour"
              className="p-1.5 rounded-md hover:bg-accent transition-colors">
              <ArrowLeft className="h-4 w-4" />
            </button>
          )}
          <History className="h-5 w-5" />
          <h1 className="text-2xl font-semibold">Historique</h1>
        </div>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      <div className="flex flex-wrap items-end gap-3 mb-3">
        <label className="text-xs text-muted-foreground">
          Du
          <input type="date" value={du} aria-label="Du"
            onChange={e => { setDu(e.target.value); setPage(0); }}
            className={`${champ} block mt-1 w-40`} />
        </label>
        <label className="text-xs text-muted-foreground">
          Au
          <input type="date" value={au} aria-label="Au"
            onChange={e => { setAu(e.target.value); setPage(0); }}
            className={`${champ} block mt-1 w-40`} />
        </label>
        <label className="text-xs text-muted-foreground">
          Personne
          <select value={auteur} aria-label="Personne"
            onChange={e => { setAuteur(e.target.value); setPage(0); }}
            className={`${champ} block mt-1 w-44`}>
            <option value="">Tout le monde</option>
            {filtres.auteurs.map(a => <option key={a.id} value={a.id}>{a.nom}</option>)}
          </select>
        </label>
        <label className="text-xs text-muted-foreground">
          Type
          <select value={type} aria-label="Type"
            onChange={e => { setType(e.target.value); setPage(0); }}
            className={`${champ} block mt-1 w-56`}>
            <option value="">Tous les événements</option>
            {filtres.types.map(t => <option key={t.type} value={t.type}>{t.libelle}</option>)}
          </select>
        </label>
        <label className="text-xs text-muted-foreground flex-1 min-w-52">
          Recherche
          <div className="relative mt-1">
            <Search className="h-4 w-4 absolute left-2.5 top-2.5 text-muted-foreground" />
            <Input value={saisie} onChange={e => setSaisie(e.target.value)}
              placeholder="Nom de client, n° de pièce, article, motif…"
              aria-label="Recherche" className="h-9 pl-8" />
          </div>
        </label>
        {aDesFiltres && (
          <Button variant="ghost" size="sm" onClick={toutEffacer}>
            Tout effacer
          </Button>
        )}
      </div>

      {contexte?.libelle && (
        <div className="mb-3">
          <span className="inline-flex items-center gap-1.5 rounded-full bg-primary/10 text-primary
                           text-xs font-medium pl-3 pr-1 py-1" data-testid="filtre-contexte">
            {contexte.libelle}
            <button aria-label="Retirer ce filtre"
              onClick={() => { setContexte(undefined); setPage(0); }}
              className="rounded-full p-0.5 hover:bg-primary/20">
              <X className="h-3.5 w-3.5" />
            </button>
          </span>
        </div>
      )}

      {erreur && (
        <div className="mb-3 rounded-md border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700" role="alert">
          {erreur}
        </div>
      )}

      <Card className="overflow-hidden">
        <CardContent className="p-0">
          <table className="w-full">
            <thead className="border-b border-border bg-muted/40">
              <tr>
                <th className={`${TH} w-36`}>Quand</th>
                <th className={`${TH} w-32`}>Qui</th>
                <th className={`${TH} w-48`}>Quoi</th>
                <th className={`${TH} w-56`}>Sur quoi</th>
                <th className={TH}>Avant → après</th>
              </tr>
            </thead>
            <tbody>
              {donnees?.lignes.map(l => (
                <tr key={l.id} className="border-b border-border/50 hover:bg-muted/30" data-testid="ligne-historique">
                  <td className={`${TD} whitespace-nowrap tabular-nums`}>{quand(l.date)}</td>
                  <td className={TD}>{l.auteur_nom ?? <span className="text-muted-foreground">—</span>}</td>
                  <td className={`${TD} font-medium ${l.type === "anomalie" ? "text-red-700" : ""}`}>
                    {l.libelle_type}
                  </td>
                  <td className={TD}><SurQuoi l={l} /></td>
                  <td className={`${TD} text-xs`}>
                    <Detail ancien={l.ancien} nouveau={l.nouveau} />
                    {l.type === "anomalie" && (l.vue ? (
                      <div className="mt-1 text-muted-foreground" data-testid="anomalie-vue">
                        Vue par {l.vue.par_nom ?? "?"} le {quand(l.vue.le)}
                      </div>
                    ) : (
                      <div className="mt-1">
                        <Button size="sm" variant="outline" className="h-7 text-xs"
                          onClick={() => marquerVue(l.id)}>
                          Marquer vue
                        </Button>
                      </div>
                    ))}
                  </td>
                </tr>
              ))}
              {donnees && donnees.lignes.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-3 py-10 text-center text-sm text-muted-foreground">
                    Rien ne correspond à ces filtres.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {chargement && !donnees && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
              <Loader2 className="h-4 w-4 animate-spin" /> Chargement…
            </div>
          )}
          {donnees && (
            <Pagination page={donnees.page} total={donnees.total} limite={donnees.par_page}
              onChanger={setPage} />
          )}
        </CardContent>
      </Card>
    </div>
  );
}
