// equipe/ClientsARelancer.tsx — les clients a relancer (PLAN-EQUIPE,
// EQ-4 — D50). Une liste tiree de la base, sans rien saisir : dormants,
// creances en retard, meilleurs clients qui ralentissent.
//
// Le clic passe par les commandes du CRM (creer_echange, creer_rappel) :
// le client sort alors de la liste pour 14 jours, ou tant que le rappel
// n'est pas fait. Les montants arrivent a `null` pour qui n'a pas le
// droit de les lire (coeur::lecture) ; les creances n'arrivent pas du
// tout sans `tiers:lire_solde`.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Users2, BellPlus } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { RAISONS_DEPART, libelle } from "./raisonsCrm";

interface Ligne {
  client_id: string; code: string; nom: string; telephone: string | null;
  parti: boolean; motifs: string[];
  dernier_achat: string | null; jours_sans_achat: number | null;
  nb_achats_reference: number; ca_reference: number | null; ca_recent: number | null;
  reste_en_retard: number | null; nb_creances_en_retard: number; jours_plus_vieille_creance: number | null;
  texte_propose: string;
}
// D50, point 16 : les partis, sans aucun montant.
interface Partis {
  nombre: number; par_raison: { raison: string; nombre: number }[]; reconquis_mois: number;
  liste: { client_id: string; code: string; nom: string; parti_le: string; raison: string }[];
}
interface Liste { jours_dormant: number; avec_creances: boolean; lignes: Ligne[]; mis_de_cote: number; partis: Partis; }
const PARTIS_VIDE: Partis = { nombre: 0, par_raison: [], reconquis_mois: 0, liste: [] };

const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const dateFr = (d: string) => d.slice(0, 10).split("-").reverse().join("/");
const GENRES: [string, string][] = [["appel", "Appel"], ["visite", "Visite"], ["whatsapp", "WhatsApp"], ["note", "Note"]];
const LIBELLE_MOTIF: Record<string, string> = { creance: "créance en retard", ralentit: "ralentit", dormant: "n'achète plus" };

function demain(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function Details({ l }: { l: Ligne }) {
  const parties: string[] = [];
  if (l.motifs.includes("creance")) {
    const n = l.nb_creances_en_retard;
    parties.push(`${n} facture${n > 1 ? "s" : ""} impayée${n > 1 ? "s" : ""}, la plus vieille de ${l.jours_plus_vieille_creance} jours`
      + (l.reste_en_retard != null ? ` · reste ${f(l.reste_en_retard)}` : ""));
  }
  if (l.motifs.includes("ralentit")) {
    parties.push(l.ca_reference != null && l.ca_recent != null
      ? `d'habitude ${f(Math.round(l.ca_reference / 3))} par trimestre, ${f(l.ca_recent)} ces 90 jours`
      : `${l.nb_achats_reference} achats l'an dernier, moins de la moitié ces 90 jours`);
  }
  if (l.dernier_achat) parties.push(`dernier achat le ${dateFr(l.dernier_achat)} (${l.jours_sans_achat} jours)`);
  return <div className="text-xs text-muted-foreground">{parties.join(" · ")}</div>;
}

export function ClientsARelancer({ moi, onOuvrir }: { moi: { id: string }; onOuvrir: (id: string) => void }) {
  const [liste, setListe] = useState<Liste | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  const [ouvert, setOuvert] = useState<string | null>(null);
  // Le formulaire ouvert sur une ligne : noter un echange, ou dire
  // pourquoi le client est parti.
  const [geste, setGeste] = useState<"noter" | "parti">("noter");
  const [raison, setRaison] = useState("");
  const [voirPartis, setVoirPartis] = useState(false);
  const [genre, setGenre] = useState("appel");
  const [quoi, setQuoi] = useState("");
  const [jours, setJours] = useState("");
  const ecrit = peut("crm:suivre");
  const regle = peut("parametres:modifier");

  const charger = useCallback(async () => {
    const l = await invoke<Liste>("lire_clients_a_relancer", { avecCreances: true });
    setListe(l);
    setJours(String(l.jours_dormant));
  }, []);
  useEffect(() => { charger().catch(e => { setAvis({ texte: String(e), erreur: true }); setListe({ jours_dormant: 60, avec_creances: false, lignes: [], mis_de_cote: 0, partis: PARTIS_VIDE }); }); }, [charger]);

  async function agir(geste: () => Promise<unknown>, fait: string) {
    setEnCours(true);
    setAvis(null);
    try {
      await geste();
      setOuvert(null);
      await charger();
      setAvis({ texte: fait });
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  function ouvrir(l: Ligne, quel: "noter" | "parti") {
    setOuvert(l.client_id);
    setGeste(quel);
    setGenre("appel");
    setQuoi(l.texte_propose);
    setRaison("");
  }

  if (!liste) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;
  const partis = liste.partis ?? PARTIS_VIDE;

  return (
    <div className="space-y-3" data-testid="clients-a-relancer">
      <div className="flex gap-2 flex-wrap items-end text-sm">
        <p className="flex-1 text-muted-foreground">
          Sans achat depuis {liste.jours_dormant} jours, créance de plus de 30 jours, meilleurs clients qui ralentissent.
          {liste.mis_de_cote > 0 && ` ${liste.mis_de_cote} client${liste.mis_de_cote > 1 ? "s" : ""} mis de côté : contacté${liste.mis_de_cote > 1 ? "s" : ""} il y a moins de 14 jours, ou un rappel l'attend.`}
          {!liste.avec_creances && " Les créances ne sont pas montrées à votre compte."}
        </p>
        {regle && (
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Jours sans achat
            <span className="flex gap-1">
              <Input type="number" min={15} max={365} value={jours} onChange={e => setJours(e.target.value)} aria-label="Jours sans achat" className="h-9 w-20" />
              <Button size="sm" variant="outline" disabled={enCours || Number(jours) === liste.jours_dormant}
                onClick={() => agir(() => invoke("definir_jours_dormant", { jours: Number(jours) }), "Réglage enregistré.")}>Changer</Button>
            </span></label>
        )}
      </div>

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      {(partis.nombre > 0 || partis.reconquis_mois > 0) && (
        <div className="border border-border rounded-lg text-sm" data-testid="clients-partis">
          <button className="w-full text-left px-4 py-2 hover:bg-muted rounded-lg" aria-expanded={voirPartis} onClick={() => setVoirPartis(v => !v)}>
            Partis : {partis.nombre}
            {partis.par_raison.length > 0 && ` (${partis.par_raison.map(r => `${libelle(r.raison).toLowerCase()} ${r.nombre}`).join(", ")})`}
            {` · reconquis ce mois : ${partis.reconquis_mois}`}
            <span className="text-xs text-muted-foreground"> — {voirPartis ? "fermer" : "voir la liste"}</span>
          </button>
          {voirPartis && (
            <div className="divide-y divide-border border-t border-border">
              {partis.liste.length === 0 && <p className="px-4 py-2 text-muted-foreground">Personne n'est parti.</p>}
              {partis.liste.map(p => (
                <div key={p.client_id} className="flex items-center gap-3 px-4 py-2" data-testid="client-parti">
                  <button className="flex-1 text-left hover:underline" onClick={() => onOuvrir(p.client_id)}>
                    <span className="font-medium">{p.nom}</span> <span className="text-xs text-muted-foreground">{p.code} · parti le {dateFr(p.parti_le)} · {libelle(p.raison)}</span>
                  </button>
                  {ecrit && (
                    <Button size="sm" variant="ghost" disabled={enCours} aria-label={`${p.nom} est revenu`}
                      onClick={() => agir(() => invoke("annuler_client_parti", { clientId: p.client_id }), `${p.nom} n'est plus marqué parti.`)}>
                      Il est revenu
                    </Button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="border border-border rounded-lg divide-y divide-border">
        {liste.lignes.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Personne à relancer.</p>}
        {liste.lignes.map(l => (
          <div key={l.client_id} className="px-4 py-2 text-sm space-y-2" data-testid="client-a-relancer">
            <div className="flex items-center gap-3 flex-wrap">
              <Users2 className="h-4 w-4 text-muted-foreground" />
              <button className="flex-1 text-left hover:underline min-w-48" onClick={() => onOuvrir(l.client_id)}>
                <div className="font-medium">{l.nom} <span className="text-xs text-muted-foreground font-normal">{l.code}{l.telephone ? ` · ${l.telephone}` : ""}</span></div>
                <Details l={l} />
              </button>
              <div className="flex gap-1 flex-wrap">
                {l.motifs.map(m => <Badge key={m} variant={m === "creance" ? "outline" : "secondary"}>{LIBELLE_MOTIF[m] ?? m}</Badge>)}
                {l.parti && <Badge variant="outline">parti</Badge>}
              </div>
              {ecrit && ouvert !== l.client_id && (
                <div className="flex gap-1">
                  <Button size="sm" variant="outline" disabled={enCours} onClick={() => ouvrir(l, "noter")}>Noter…</Button>
                  <Button size="sm" variant="ghost" disabled={enCours} aria-label={`Me rappeler ${l.nom} demain`}
                    onClick={() => agir(() => invoke("creer_rappel", { clientId: l.client_id, pourUtilisateurId: moi.id, quand: demain(), quoi: l.texte_propose }), `Rappel posé pour demain : ${l.nom}.`)}>
                    <BellPlus className="h-4 w-4 mr-1" /> Me le rappeler
                  </Button>
                  {!l.parti && <Button size="sm" variant="ghost" disabled={enCours} aria-label={`${l.nom} est parti`} onClick={() => ouvrir(l, "parti")}>Parti…</Button>}
                </div>
              )}
            </div>
            {ouvert === l.client_id && geste === "parti" && (
              <div className="flex gap-2 flex-wrap items-end pl-7" data-testid="marquer-parti">
                <label className="text-xs text-muted-foreground flex flex-col gap-1">Pourquoi est-il parti ?
                  <select value={raison} onChange={e => setRaison(e.target.value)} aria-label="Raison du départ"
                    className="h-9 px-2 text-sm border border-border rounded-md bg-background min-w-40">
                    <option value="">— choisir —</option>
                    {RAISONS_DEPART.map(([v, lib]) => <option key={v} value={v}>{lib}</option>)}
                  </select></label>
                <Button size="sm" disabled={enCours || !raison}
                  onClick={() => agir(() => invoke("marquer_client_parti", { clientId: l.client_id, raison }), `${l.nom} marqué parti. Sa prochaine vente le fera revenir.`)}>Marquer parti</Button>
                <Button size="sm" variant="ghost" disabled={enCours} onClick={() => setOuvert(null)}>Annuler</Button>
              </div>
            )}
            {ouvert === l.client_id && geste === "noter" && (
              <div className="flex gap-2 flex-wrap items-end pl-7" data-testid="noter-relance">
                <label className="text-xs text-muted-foreground flex flex-col gap-1">Genre
                  <select value={genre} onChange={e => setGenre(e.target.value)} aria-label="Genre"
                    className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                    {GENRES.map(([v, lib]) => <option key={v} value={v}>{lib}</option>)}
                  </select></label>
                <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-48">Ce qui s'est passé
                  <Input value={quoi} onChange={e => setQuoi(e.target.value)} aria-label="Ce qui s'est passé" className="h-9" /></label>
                <Button size="sm" disabled={enCours || !quoi.trim()}
                  onClick={() => agir(() => invoke("creer_echange", { clientId: l.client_id, genre, quoi, suitePrevue: null }), `Échange noté : ${l.nom}.`)}>Noter</Button>
                <Button size="sm" variant="ghost" disabled={enCours} onClick={() => setOuvert(null)}>Annuler</Button>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
