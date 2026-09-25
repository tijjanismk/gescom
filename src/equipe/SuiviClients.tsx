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

interface Utilisateur { id: string; nom: string; role: string; actif: boolean; }
interface ClientLigne { id: string; code: string; nom: string; telephone: string | null; statut?: string; }
interface Echange {
  id: string; genre: string; quoi: string; suite_prevue: string | null;
  date: string; auteur_nom: string | null; source: string; canal?: string;
}
interface Rappel {
  id: string; client_id: string; client_nom: string; pour_utilisateur_id: string; pour_nom: string | null;
  quand: string; quoi: string; fait: boolean; fait_le: string | null;
}
interface Prospect { id: string; code: string; nom: string; telephone: string | null; origine_prospect: string | null; cree_le: string; }

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

  if (!fiche) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  const { client, stats } = fiche;
  const estProspect = client.statut === "prospect";

  return (
    <div className="space-y-4 max-w-3xl" data-testid="fiche-360">
      <Button size="sm" variant="ghost" onClick={onRetour}><ArrowLeft className="h-4 w-4 mr-1" /> Retour</Button>

      <div className="flex items-center gap-2 flex-wrap">
        <h1 className="text-xl font-semibold">{client.nom}</h1>
        <span className="text-sm text-muted-foreground">{client.code}</span>
        {estProspect ? <Badge variant="secondary">prospect</Badge> : null}
      </div>
      {client.telephone && <p className="text-sm text-muted-foreground">{client.telephone}</p>}

      <div className="flex gap-4 flex-wrap text-sm">
        <div className="border border-border rounded-lg px-3 py-2"><div className="text-muted-foreground text-xs">Chiffre d'affaires</div><div className="font-semibold">{f(stats.ca_total)}</div></div>
        <div className="border border-border rounded-lg px-3 py-2"><div className="text-muted-foreground text-xs">Ventes</div><div className="font-semibold">{stats.nb_ventes}</div></div>
        {stats.encours != null && <div className="border border-border rounded-lg px-3 py-2"><div className="text-muted-foreground text-xs">Encours</div><div className="font-semibold">{f(stats.encours)}</div></div>}
      </div>

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

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
    </div>
  );
}

// =====================================================================
//  Recherche client
// =====================================================================

function Clients({ onOuvrir }: { onOuvrir: (id: string) => void }) {
  const [recherche, setRecherche] = useState("");
  const [resultats, setResultats] = useState<ClientLigne[] | null>(null);

  useEffect(() => {
    let annule = false;
    invoke<{ donnees: ClientLigne[] }>("lire_clients_pagines", {
      page: 0, limite: 20, recherche: recherche || null,
      avecCreancesSeulement: false, ventesFiltre: null, tri: null,
    }).then(r => { if (!annule) setResultats(r.donnees); }).catch(() => setResultats([]));
    return () => { annule = true; };
  }, [recherche]);

  return (
    <div className="space-y-3 max-w-xl">
      <label className="flex items-center gap-2 border border-border rounded-lg px-3 h-9">
        <Search className="h-4 w-4 text-muted-foreground" />
        <input value={recherche} onChange={e => setRecherche(e.target.value)} placeholder="Nom, code, téléphone…"
          aria-label="Rechercher un client" className="flex-1 bg-transparent text-sm outline-none" />
      </label>
      {!resultats ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {resultats.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun client.</p>}
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

  const charger = useCallback(async () => setListe(await invoke<Prospect[]>("lire_prospects")), []);
  useEffect(() => { charger().catch(e => setAvis({ texte: String(e), erreur: true })); }, [charger]);

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
    <div className="space-y-4 max-w-2xl">
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
      {!liste ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-prospects">
          {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucun prospect.</p>}
          {liste.map(p => (
            <button key={p.id} onClick={() => onOuvrir(p.id)} data-testid="prospect"
              className="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-muted">
              <div className="flex-1">
                <div className="font-medium">{p.nom}</div>
                <div className="text-xs text-muted-foreground">
                  {p.telephone ? `${p.telephone} · ` : ""}depuis le {dateFr(p.cree_le)}{p.origine_prospect ? ` · ${p.origine_prospect}` : ""}
                </div>
              </div>
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

  const charger = useCallback(async () => setListe(await invoke<Rappel[]>("lire_rappels", { pourUtilisateurId: moi.id, clientId: null, inclureFaits: false })), [moi.id]);
  useEffect(() => { charger().catch(() => setListe([])); }, [charger]);

  async function marquerFait(id: string) {
    setEnCours(true);
    try { await invoke("marquer_rappel_fait", { rappelId: id }); await charger(); } finally { setEnCours(false); }
  }

  if (!liste) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  return (
    <div className="border border-border rounded-lg divide-y divide-border max-w-2xl" data-testid="mes-rappels">
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
  );
}

// =====================================================================
//  Écran
// =====================================================================

export function SuiviClients({ moi }: { moi: { id: string; nom: string } }) {
  const [vue, setVue] = useState<"clients" | "prospects" | "rappels">("clients");
  const [clientOuvert, setClientOuvert] = useState<string | null>(null);

  if (clientOuvert) {
    return <Fiche360 clientId={clientOuvert} moi={moi} onRetour={() => setClientOuvert(null)} />;
  }

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-semibold">Suivi clients</h1>
      <div className="flex gap-1" role="tablist">
        {([["clients", "Clients"], ["prospects", "Prospects"], ["rappels", "Mes rappels"]] as const).map(([cle, libelle]) => (
          <button key={cle} role="tab" aria-selected={vue === cle} onClick={() => setVue(cle)}
            className={`px-3 py-1.5 text-sm rounded-lg ${vue === cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
            {libelle}
          </button>
        ))}
      </div>
      {vue === "clients" ? <Clients onOuvrir={setClientOuvert} />
        : vue === "prospects" ? <Prospects onOuvrir={setClientOuvert} />
        : <MesRappels moi={moi} onOuvrir={setClientOuvert} />}
    </div>
  );
}
