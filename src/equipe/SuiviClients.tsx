// equipe/SuiviClients.tsx — le suivi client (PLAN-EQUIPE, H-1/H-2 — D34).
//
// Pas de seconde fiche client : la recherche s'appuie sur les clients
// de Gescom (lire_clients_pagines) ; un prospect EST un client
// (statut « prospect »), qui redevient client tout seul a sa premiere
// vente. Les relances de creance existantes se retrouvent dans le
// meme fil que les echanges (D34) : pas une liste de plus.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Search, ArrowLeft, PhoneCall, Users2, Store, StickyNote, BadgeCheck, MessageCircle } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { KpiPetit, GRILLE } from "@/components/ui/KpiVerre";
import { ClientsARelancer } from "./ClientsARelancer";
import { ETAPES_PROSPECT, RAISONS_ABANDON, RAISONS_DEPART, libelle } from "./raisonsCrm";

interface Utilisateur { id: string; nom: string; role: string; actif: boolean; }
interface ClientLigne {
  id: string; code: string; nom: string; telephone: string | null; statut?: string;
  // D50 (10-13), lus par la fiche seulement.
  etape_prospect?: string | null; raison_sortie?: string | null; parti_le?: string | null;
}
interface Echange {
  id: string; genre: string; quoi: string; suite_prevue: string | null;
  date: string; auteur_nom: string | null; source: string; canal?: string;
}
interface Rappel {
  id: string; client_id: string; client_nom: string; pour_utilisateur_id: string; pour_nom: string | null;
  quand: string; quoi: string; fait: boolean; fait_le: string | null;
}
interface Prospect {
  id: string; code: string; nom: string; telephone: string | null; origine_prospect: string | null; cree_le: string;
  etape: string; raison_abandon: string | null;
}
interface CompteProspects {
  mois: string; crees: number; devenus_clients: number;
  par_origine: { origine: string | null; crees: number; devenus_clients: number }[];
  abandons: { raison: string | null; nombre: number }[];
}

const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const dateFr = (d: string) => d.slice(0, 10).split("-").reverse().join("/");
const GENRES: [string, string][] = [["appel", "Appel"], ["visite", "Visite"], ["whatsapp", "WhatsApp"], ["note", "Note"]];

function IconeGenre({ genre }: { genre: string }) {
  const cls = "h-4 w-4";
  if (genre === "appel") return <PhoneCall className={cls} />;
  if (genre === "visite") return <Store className={cls} />;
  if (genre === "whatsapp" || genre === "relance") return <MessageCircle className={cls} />;
  return <StickyNote className={cls} />;
}

// =====================================================================
//  Fiche 360
// =====================================================================

// D50 (10-13) : l'etape d'un prospect, ou le depart d'un client. Rien
// ici ne bloque une vente : la prochaine fait revenir un parti.
function SuiviEtat({ client, agir }: { client: ClientLigne; agir: (geste: () => Promise<unknown>, fait: string) => Promise<void> }) {
  const prospect = client.statut === "prospect";
  const [etape, setEtape] = useState(client.etape_prospect || "nouveau");
  const [raison, setRaison] = useState("");
  useEffect(() => { setEtape(client.etape_prospect || "nouveau"); setRaison(""); }, [client.etape_prospect, client.parti_le]);
  const choix = "h-9 px-2 text-sm border border-border rounded-md bg-background min-w-40";

  if (prospect) {
    const actuelle = client.etape_prospect || "nouveau";
    const abandon = etape === "abandonne";
    return (
      <div className="flex gap-2 flex-wrap items-end text-sm" data-testid="etape-prospect">
        <label className="text-xs text-muted-foreground flex flex-col gap-1">Étape
          <select value={etape} onChange={e => setEtape(e.target.value)} aria-label="Étape du prospect" className={choix}>
            {ETAPES_PROSPECT.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
          </select></label>
        {abandon && (
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Pourquoi ?
            <select value={raison} onChange={e => setRaison(e.target.value)} aria-label="Raison de l'abandon" className={choix}>
              <option value="">— choisir —</option>
              {RAISONS_ABANDON.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select></label>
        )}
        <Button size="sm" variant="outline" disabled={etape === actuelle && !abandon || (abandon && !raison)}
          onClick={() => agir(() => invoke("changer_etape_prospect", { clientId: client.id, etape, raison: abandon ? raison : null }), `Étape : ${libelle(etape)}.`)}>
          Changer
        </Button>
        {actuelle === "abandonne" && client.raison_sortie && <span className="text-xs text-muted-foreground">abandonné : {libelle(client.raison_sortie)}</span>}
      </div>
    );
  }

  if (client.parti_le) {
    return (
      <div className="flex gap-2 flex-wrap items-center text-sm" data-testid="client-parti">
        <Badge variant="outline">parti</Badge>
        <span className="text-muted-foreground">le {dateFr(client.parti_le)} · {libelle(client.raison_sortie)} — sa prochaine vente le fera revenir.</span>
        <Button size="sm" variant="ghost" onClick={() => agir(() => invoke("annuler_client_parti", { clientId: client.id }), "N'est plus marqué parti.")}>Il est revenu</Button>
      </div>
    );
  }

  return (
    <div className="flex gap-2 flex-wrap items-end text-sm" data-testid="marquer-parti">
      <label className="text-xs text-muted-foreground flex flex-col gap-1">Client parti ?
        <select value={raison} onChange={e => setRaison(e.target.value)} aria-label="Raison du départ" className={choix}>
          <option value="">— pourquoi —</option>
          {RAISONS_DEPART.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
        </select></label>
      <Button size="sm" variant="outline" disabled={!raison}
        onClick={() => agir(() => invoke("marquer_client_parti", { clientId: client.id, raison }), "Marqué parti.")}>Marquer parti</Button>
    </div>
  );
}

function Fiche360({ clientId, moi, onRetour }: { clientId: string; moi: { id: string; nom: string }; onRetour: () => void }) {
  const [fiche, setFiche] = useState<{ client: ClientLigne & { code: string }; stats: { ca_total: number; nb_ventes: number; encours: number | null } } | null>(null);
  const [fil, setFil] = useState<Echange[] | null>(null);
  const [rappels, setRappels] = useState<Rappel[] | null>(null);
  const [utilisateurs, setUtilisateurs] = useState<Utilisateur[]>([]);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  const ecrit = peut("crm:suivre");

  const [genre, setGenre] = useState("appel");
  const [quoi, setQuoi] = useState("");
  const [suite, setSuite] = useState("");

  const [rQuand, setRQuand] = useState("");
  const [rQuoi, setRQuoi] = useState("");
  const [rPour, setRPour] = useState("");

  const charger = useCallback(async () => {
    const [f2, e, r] = await Promise.all([
      invoke<{ client: ClientLigne; stats: { ca_total: number; nb_ventes: number; encours: number | null } }>("lire_fiche_client", { clientId }),
      invoke<Echange[]>("lire_echanges_client", { clientId }),
      invoke<Rappel[]>("lire_rappels", { pourUtilisateurId: null, clientId, inclureFaits: true }),
    ]);
    setFiche(f2 as any);
    setFil(e);
    setRappels(r);
  }, [clientId]);

  useEffect(() => { charger().catch(err => setAvis({ texte: String(err), erreur: true })); }, [charger]);
  useEffect(() => {
    invoke<Utilisateur[]>("lire_utilisateurs").then(u => {
      const actifs = u.filter(x => x.actif);
      setUtilisateurs(actifs);
      setRPour(moi.id);
    }).catch(() => setUtilisateurs([]));
  }, [moi.id]);

  async function ajouterEchange() {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke("creer_echange", { clientId, genre, quoi, suitePrevue: suite || null });
      setQuoi(""); setSuite("");
      await charger();
      setAvis({ texte: "Échange noté." });
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  async function ajouterRappel() {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke("creer_rappel", { clientId, pourUtilisateurId: rPour, quand: rQuand, quoi: rQuoi });
      setRQuand(""); setRQuoi("");
      await charger();
      setAvis({ texte: "Rappel noté." });
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  async function marquerFait(id: string) {
    setEnCours(true);
    try {
      await invoke("marquer_rappel_fait", { rappelId: id });
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  async function agirEtat(geste: () => Promise<unknown>, fait: string) {
    setEnCours(true);
    setAvis(null);
    try {
      await geste();
      await charger();
      setAvis({ texte: fait });
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  // Le bouton Retour et l'avis restent visibles meme quand la fiche ne
  // charge pas : sinon un echec initial laisse un tourniquet sans issue.
  const client = fiche?.client;
  const stats = fiche?.stats;
  const estProspect = client?.statut === "prospect";

  return (
    // Toute la largeur (le proprietaire, 29/10/2026) : la fiche tenait
    // dans 768 px ; ses chiffres sont des tuiles comme au tableau de bord.
    <div className="space-y-4" data-testid="fiche-360">
      <Button size="sm" variant="ghost" onClick={onRetour}><ArrowLeft className="h-4 w-4 mr-1" /> Retour</Button>

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      {!client || !stats ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
      <>
      <div className="flex items-center gap-2 flex-wrap">
        <h1 className="text-xl font-semibold">{client.nom}</h1>
        <span className="text-sm text-muted-foreground">{client.code}</span>
        {estProspect ? <Badge variant="secondary">prospect</Badge> : null}
      </div>
      {client.telephone && <p className="text-sm text-muted-foreground">{client.telephone}</p>}
      {ecrit && <SuiviEtat client={client} agir={agirEtat} />}

      <div style={GRILLE}>
        <KpiPetit titre="Chiffre d'affaires" valeur={f(stats.ca_total)} />
        <KpiPetit titre="Ventes" valeur={String(stats.nb_ventes)} />
        {stats.encours != null && <KpiPetit titre="Encours" valeur={f(stats.encours)} />}
      </div>

      {/* Rappels */}
      <div className="border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-semibold">Rappels</h3>
        {ecrit && (
          <div className="flex gap-2 flex-wrap items-end" data-testid="nouveau-rappel">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Pour qui
              <select value={rPour} onChange={e => setRPour(e.target.value)} aria-label="Pour qui"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background min-w-40">
                {utilisateurs.map(u => <option key={u.id} value={u.id}>{u.nom}{u.id === moi.id ? " (moi)" : ""}</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Quand
              <Input type="date" value={rQuand} onChange={e => setRQuand(e.target.value)} aria-label="Quand" className="h-9" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Quoi
              <Input value={rQuoi} onChange={e => setRQuoi(e.target.value)} aria-label="Quoi" className="h-9" /></label>
            <Button size="sm" disabled={enCours || !rQuand || !rQuoi.trim() || !rPour} onClick={ajouterRappel}>Rappeler</Button>
          </div>
        )}
        <div className="divide-y divide-border">
          {(rappels ?? []).length === 0 && <p className="text-sm text-muted-foreground">Aucun rappel.</p>}
          {(rappels ?? []).map(r => (
            <div key={r.id} className="flex items-center gap-3 py-2 text-sm" data-testid="rappel">
              <div className="flex-1">
                <div className={r.fait ? "line-through text-muted-foreground" : ""}>{r.quoi}</div>
                <div className="text-xs text-muted-foreground">le {dateFr(r.quand)} · pour {r.pour_nom ?? "—"}</div>
              </div>
              {r.fait ? <Badge variant="outline">fait</Badge>
                : ecrit && <Button size="sm" variant="ghost" aria-label={`Marquer fait : ${r.quoi}`} disabled={enCours} onClick={() => marquerFait(r.id)}>
                    <BadgeCheck className="h-4 w-4" />
                  </Button>}
            </div>
          ))}
        </div>
      </div>

      {/* Échanges */}
      <div className="border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-semibold">Échanges</h3>
        {ecrit && (
          <div className="flex gap-2 flex-wrap items-end" data-testid="nouvel-echange">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Genre
              <select value={genre} onChange={e => setGenre(e.target.value)} aria-label="Genre"
                className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                {GENRES.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Ce qui s'est passé
              <Input value={quoi} onChange={e => setQuoi(e.target.value)} aria-label="Ce qui s'est passé" className="h-9" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-40">Suite prévue (facultatif)
              <Input value={suite} onChange={e => setSuite(e.target.value)} aria-label="Suite prévue" className="h-9" /></label>
            <Button size="sm" disabled={enCours || !quoi.trim()} onClick={ajouterEchange}>Noter</Button>
          </div>
        )}
        <div className="divide-y divide-border">
          {(fil ?? []).length === 0 && <p className="text-sm text-muted-foreground">Aucun échange.</p>}
          {(fil ?? []).map(e => (
            <div key={e.id} className="flex items-start gap-3 py-2 text-sm" data-testid="echange">
              <IconeGenre genre={e.genre} />
              <div className="flex-1">
                <div>{e.quoi}</div>
                <div className="text-xs text-muted-foreground">
                  le {dateFr(e.date)}{e.auteur_nom ? ` · ${e.auteur_nom}` : ""}
                  {e.suite_prevue ? ` · à suivre : ${e.suite_prevue}` : ""}
                  {e.source === "relance_creance" ? " · relance de créance" : ""}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
      </>
      )}
    </div>
  );
}

// =====================================================================
//  Recherche client
// =====================================================================

function Clients({ onOuvrir }: { onOuvrir: (id: string) => void }) {
  const [recherche, setRecherche] = useState("");
  const [resultats, setResultats] = useState<ClientLigne[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  // La recherche part au serveur : on attend que la frappe se pose
  // (200 ms), et on jette une reponse — ou une erreur — arrivee apres
  // une frappe plus recente.
  useEffect(() => {
    let annule = false;
    const t = setTimeout(() => {
      invoke<{ donnees: ClientLigne[] }>("lire_clients_pagines", {
        page: 0, limite: 20, recherche: recherche || null,
        avecCreancesSeulement: false, ventesFiltre: null, tri: null,
      })
        .then(r => { if (annule) return; setErreur(null); setResultats(r.donnees); })
        .catch(e => { if (annule) return; setErreur(String(e)); setResultats([]); });
    }, 200);
    return () => { annule = true; clearTimeout(t); };
  }, [recherche]);

  return (
    <div className="space-y-3">
      <label className="flex items-center gap-2 border border-border rounded-lg px-3 h-9">
        <Search className="h-4 w-4 text-muted-foreground" />
        <input value={recherche} onChange={e => setRecherche(e.target.value)} placeholder="Nom, code, téléphone…"
          aria-label="Rechercher un client" className="flex-1 bg-transparent text-sm outline-none" />
      </label>
      {!resultats ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {erreur ? <p className="px-4 py-3 text-sm text-red-600" role="status">{erreur}</p>
            : resultats.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun client.</p>}
          {resultats.map(c => (
            <button key={c.id} onClick={() => onOuvrir(c.id)} data-testid="resultat-client"
              className="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-muted">
              <Users2 className="h-4 w-4 text-muted-foreground" />
              <div className="flex-1">
                <div className="font-medium">{c.nom}</div>
                <div className="text-xs text-muted-foreground">{c.code}{c.telephone ? ` · ${c.telephone}` : ""}</div>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// =====================================================================
//  Prospects
// =====================================================================

function Prospects({ onOuvrir }: { onOuvrir: (id: string) => void }) {
  const [liste, setListe] = useState<Prospect[] | null>(null);
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [origine, setOrigine] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const ecrit = peut("crm:suivre");
  // D50, point 12 : le compte du mois choisi, les abandonnes caches d'office.
  const [mois, setMois] = useState(() => new Date().toISOString().slice(0, 7));
  const [compte, setCompte] = useState<CompteProspects | null>(null);
  const [voirAbandonnes, setVoirAbandonnes] = useState(false);

  const charger = useCallback(async () => setListe(await invoke<Prospect[]>("lire_prospects")), []);
  useEffect(() => { charger().catch(e => setAvis({ texte: String(e), erreur: true })); }, [charger]);
  useEffect(() => {
    if (!/^\d{4}-\d{2}$/.test(mois)) return;
    invoke<CompteProspects>("lire_compte_prospects", { mois }).then(setCompte).catch(e => setAvis({ texte: String(e), erreur: true }));
  }, [mois, liste]);
  const visibles = (liste ?? []).filter(p => voirAbandonnes || p.etape !== "abandonne");
  const nbAbandonnes = (liste ?? []).length - (liste ?? []).filter(p => p.etape !== "abandonne").length;

  async function ajouter() {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke("creer_prospect", { nom, telephone: telephone || null, origine: origine || null });
      setNom(""); setTelephone(""); setOrigine("");
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="space-y-2" data-testid="compte-prospects">
        <label className="text-xs text-muted-foreground flex items-center gap-2">Mois
          <Input type="month" value={mois} onChange={e => setMois(e.target.value)} aria-label="Mois du compte" className="h-9 w-44" /></label>
        {compte && (
          <div style={GRILLE}>
            <KpiPetit titre="Prospects créés" valeur={String(compte.crees)} />
            <KpiPetit titre="Devenus clients" valeur={String(compte.devenus_clients)} />
            {compte.par_origine.map(o => (
              <KpiPetit key={o.origine ?? ""} titre={o.origine ?? "Origine non dite"} valeur={`${o.crees} créés · ${o.devenus_clients} clients`} />
            ))}
            <KpiPetit titre="Abandons en cours"
              valeur={compte.abandons.length === 0 ? "aucun" : compte.abandons.map(a => `${libelle(a.raison)} ${a.nombre}`).join(" · ")} />
          </div>
        )}
      </div>
      {ecrit && (
        <div className="border border-border rounded-lg p-4 space-y-3" data-testid="nouveau-prospect">
          <h3 className="text-sm font-semibold">Nouveau prospect</h3>
          <p className="text-xs text-muted-foreground">Un client sans vente encore — il devient client tout seul à sa première vente.</p>
          <div className="flex gap-2 flex-wrap items-end">
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Nom
              <Input value={nom} onChange={e => setNom(e.target.value)} aria-label="Nom du prospect" className="h-9" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Téléphone
              <Input value={telephone} onChange={e => setTelephone(e.target.value)} aria-label="Téléphone du prospect" className="h-9" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">D'où il vient
              <Input value={origine} onChange={e => setOrigine(e.target.value)} placeholder="salon, recommandation…" aria-label="Origine du prospect" className="h-9" /></label>
            <Button size="sm" disabled={enCours || !nom.trim()} onClick={ajouter}>Ajouter</Button>
          </div>
        </div>
      )}
      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}
      {nbAbandonnes > 0 && (
        <label className="text-sm flex items-center gap-2">
          <input type="checkbox" checked={voirAbandonnes} onChange={e => setVoirAbandonnes(e.target.checked)} />
          Voir les abandonnés ({nbAbandonnes})
        </label>
      )}
      {!liste ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-prospects">
          {visibles.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun prospect.</p>}
          {visibles.map(p => (
            <button key={p.id} onClick={() => onOuvrir(p.id)} data-testid="prospect"
              className="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-muted">
              <div className="flex-1">
                <div className="font-medium">{p.nom}</div>
                <div className="text-xs text-muted-foreground">
                  {p.telephone ? `${p.telephone} · ` : ""}depuis le {dateFr(p.cree_le)}{p.origine_prospect ? ` · ${p.origine_prospect}` : ""}
                </div>
              </div>
              <Badge variant={p.etape === "abandonne" ? "outline" : "secondary"}>
                {libelle(p.etape)}{p.etape === "abandonne" && p.raison_abandon ? ` · ${libelle(p.raison_abandon)}` : ""}
              </Badge>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// =====================================================================
//  Mes rappels
// =====================================================================

function MesRappels({ moi, onOuvrir }: { moi: { id: string }; onOuvrir: (id: string) => void }) {
  const [liste, setListe] = useState<Rappel[] | null>(null);
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);

  const charger = useCallback(async () => setListe(await invoke<Rappel[]>("lire_rappels", { pourUtilisateurId: moi.id, clientId: null, inclureFaits: false })), [moi.id]);
  useEffect(() => { charger().catch(e => { setAvis({ texte: String(e), erreur: true }); setListe([]); }); }, [charger]);

  async function marquerFait(id: string) {
    setEnCours(true);
    setAvis(null);
    try {
      await invoke("marquer_rappel_fait", { rappelId: id });
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
      // On recharge quand meme : un rappel deja marque par un collegue
      // doit disparaitre de la liste.
      await charger().catch(() => {});
    } finally {
      setEnCours(false);
    }
  }

  if (!liste) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  return (
    <>
    {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}
    <div className="border border-border rounded-lg divide-y divide-border" data-testid="mes-rappels">
      {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun rappel en attente.</p>}
      {liste.map(r => (
        <div key={r.id} className="flex items-center gap-3 px-4 py-2 text-sm" data-testid="rappel">
          <button className="flex-1 text-left hover:underline" onClick={() => onOuvrir(r.client_id)}>
            <div className="font-medium">{r.client_nom} — {r.quoi}</div>
            <div className="text-xs text-muted-foreground">le {dateFr(r.quand)}</div>
          </button>
          <Button size="sm" variant="ghost" aria-label={`Marquer fait : ${r.quoi}`} disabled={enCours} onClick={() => marquerFait(r.id)}>
            <BadgeCheck className="h-4 w-4" />
          </Button>
        </div>
      ))}
    </div>
    </>
  );
}

// =====================================================================
//  Écran
// =====================================================================

// D49 : `vue`, l'onglet ou ouvrir (une tuile de l'accueil).
export function SuiviClients({ moi, vue: depart }: { moi: { id: string; nom: string }; vue?: "rappels" | "relancer" }) {
  const [vue, setVue] = useState<"clients" | "prospects" | "rappels" | "relancer">(depart ?? "clients");
  const [clientOuvert, setClientOuvert] = useState<string | null>(null);

  if (clientOuvert) {
    return <Fiche360 clientId={clientOuvert} moi={moi} onRetour={() => setClientOuvert(null)} />;
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">Suivi clients</h1>
      <div className="flex gap-1" role="tablist">
        {([["clients", "Clients"], ["prospects", "Prospects"], ["rappels", "Mes rappels"], ["relancer", "À relancer"]] as const).map(([cle, libelle]) => (
          <button key={cle} role="tab" aria-selected={vue === cle} onClick={() => setVue(cle)}
            className={`px-3 py-1.5 text-sm rounded-lg ${vue === cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
            {libelle}
          </button>
        ))}
      </div>
      {vue === "clients" ? <Clients onOuvrir={setClientOuvert} />
        : vue === "prospects" ? <Prospects onOuvrir={setClientOuvert} />
        : vue === "relancer" ? <ClientsARelancer moi={moi} onOuvrir={setClientOuvert} />
        : <MesRappels moi={moi} onOuvrir={setClientOuvert} />}
    </div>
  );
}
