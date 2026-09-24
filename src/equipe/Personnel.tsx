// equipe/Personnel.tsx — les fiches du personnel (PLAN-EQUIPE, F-2, D30).
//
// Une personne, pas un contrat : le nom, ce qu'elle fait, comment elle
// est payée (au mois, à la journée, à la commission, à la tâche, ou
// rien de fixe). Le reste s'ajoute quand on l'a. Ce qu'elle gagne ne
// s'affiche qu'à qui prépare ou valide la paie — le serveur le masque.

import { useState, useEffect, useCallback } from "react";
import { Loader2, Plus, UserMinus, UserCheck, ChevronDown, ChevronRight } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { JoursTravailles } from "./JoursTravailles";

export interface Employe {
  id: string;
  nom: string;
  fonction: string;
  telephone: string | null;
  date_entree: string | null;
  piece_identite: string | null;
  contrat_ecrit: boolean;
  contrat_date: string | null;
  declare: boolean;
  numero_inps: string | null;
  salaire_mensuel: number | null;
  tarif_journalier: number | null;
  commission_pct: number | null;
  a_la_tache: boolean;
  modes: string[];
  remuneration_dite: string | null;
  utilisateur_id: string | null;
  utilisateur_nom: string | null;
  depot_id: string | null;
  depot_nom: string | null;
  statut: "actif" | "partie";
  date_depart: string | null;
  motif_depart: string | null;
  note: string | null;
}

interface Saisie {
  nom: string; fonction: string;
  auMois: boolean; salaire: string;
  aLaJournee: boolean; tarif: string;
  aLaCommission: boolean; commission: string;
  aLaTache: boolean;
  telephone: string; dateEntree: string; piece: string;
  contrat: boolean; contratDate: string;
  declare: boolean; inps: string;
  utilisateurId: string; depotId: string; note: string;
}

const VIDE: Saisie = {
  nom: "", fonction: "", auMois: false, salaire: "", aLaJournee: false, tarif: "",
  aLaCommission: false, commission: "", aLaTache: false, telephone: "", dateEntree: "", piece: "",
  contrat: false, contratDate: "", declare: false, inps: "", utilisateurId: "", depotId: "", note: "",
};

function depuis(e: Employe): Saisie {
  return {
    nom: e.nom, fonction: e.fonction,
    auMois: e.modes.includes("au mois"), salaire: e.salaire_mensuel?.toString() ?? "",
    aLaJournee: e.modes.includes("à la journée"), tarif: e.tarif_journalier?.toString() ?? "",
    aLaCommission: e.modes.includes("à la commission"), commission: e.commission_pct?.toString() ?? "",
    aLaTache: e.a_la_tache,
    telephone: e.telephone ?? "", dateEntree: e.date_entree ?? "", piece: e.piece_identite ?? "",
    contrat: e.contrat_ecrit, contratDate: e.contrat_date ?? "",
    declare: e.declare, inps: e.numero_inps ?? "",
    utilisateurId: e.utilisateur_id ?? "", depotId: e.depot_id ?? "", note: e.note ?? "",
  };
}

const nombre = (s: string) => {
  const n = Number(s.replace(/\s/g, "").replace(",", "."));
  return s.trim() === "" || !isFinite(n) ? null : n;
};

function versFiche(s: Saisie) {
  return {
    nom: s.nom, fonction: s.fonction,
    salaire_mensuel: s.auMois ? nombre(s.salaire) : null,
    tarif_journalier: s.aLaJournee ? nombre(s.tarif) : null,
    commission_pct: s.aLaCommission ? nombre(s.commission) : null,
    a_la_tache: s.aLaTache,
    telephone: s.telephone || null, date_entree: s.dateEntree || null, piece_identite: s.piece || null,
    contrat_ecrit: s.contrat, contrat_date: s.contrat ? s.contratDate || null : null,
    declare: s.declare, numero_inps: s.inps || null,
    utilisateur_id: s.utilisateurId || null, depot_id: s.depotId || null, note: s.note || null,
  };
}

const champ = "h-9 px-2 text-sm border border-border rounded-md bg-background w-full";

function Formulaire({ initial, voitMontants, onEnregistrer, onAnnuler }: {
  initial: Saisie;
  voitMontants: boolean;
  onEnregistrer: (s: Saisie) => Promise<void>;
  onAnnuler: () => void;
}) {
  const [s, setS] = useState(initial);
  const [plus, setPlus] = useState(false);
  const [enCours, setEnCours] = useState(false);
  const [comptes, setComptes] = useState<{ id: string; nom: string; actif: boolean }[]>([]);
  const [depots, setDepots] = useState<{ id: string; nom: string }[]>([]);
  const maj = (p: Partial<Saisie>) => setS(x => ({ ...x, ...p }));

  useEffect(() => {
    invoke<{ id: string; nom: string; actif: boolean }[]>("lire_utilisateurs").then(setComptes).catch(() => setComptes([]));
    invoke<{ id: string; nom: string }[]>("lire_depots").then(setDepots).catch(() => setDepots([]));
  }, []);

  const mode = (cle: "auMois" | "aLaJournee" | "aLaCommission", libelle: string, valeur: "salaire" | "tarif" | "commission", unite: string) => (
    <div className="flex items-center gap-2">
      <label className="flex items-center gap-2 text-sm w-40">
        <input type="checkbox" checked={s[cle]} onChange={e => maj({ [cle]: e.target.checked } as Partial<Saisie>)} />
        {libelle}
      </label>
      {s[cle] && voitMontants && (
        <>
          <Input value={s[valeur]} onChange={e => maj({ [valeur]: e.target.value } as Partial<Saisie>)}
            aria-label={libelle} inputMode="decimal" className="h-8 w-32" />
          <span className="text-xs text-muted-foreground">{unite}</span>
        </>
      )}
    </div>
  );

  return (
    <div className="border border-border rounded-lg p-4 space-y-4" data-testid="formulaire-employe">
      <div className="grid sm:grid-cols-2 gap-3">
        <label className="text-xs text-muted-foreground flex flex-col gap-1">
          Nom
          <Input value={s.nom} onChange={e => maj({ nom: e.target.value })} aria-label="Nom" className="h-9" />
        </label>
        <label className="text-xs text-muted-foreground flex flex-col gap-1">
          Ce qu'elle fait
          <Input value={s.fonction} onChange={e => maj({ fonction: e.target.value })} aria-label="Ce qu'elle fait"
            placeholder="vendeuse, magasinier, livreur…" className="h-9" />
        </label>
      </div>

      <div className="space-y-2">
        <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Comment elle est payée</p>
        {mode("auMois", "Au mois", "salaire", "F par mois")}
        {mode("aLaJournee", "À la journée", "tarif", "F par jour travaillé")}
        {mode("aLaCommission", "À la commission", "commission", "% des ventes de son compte")}
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={s.aLaTache} onChange={e => maj({ aLaTache: e.target.checked })} />
          À la tâche (des lignes sur la fiche de paie)
        </label>
        {!s.auMois && !s.aLaJournee && !s.aLaCommission && !s.aLaTache && (
          <p className="text-xs text-muted-foreground">Rien de coché : rien de fixe, des primes à la paie.</p>
        )}
        {!voitMontants && (
          <p className="text-xs text-muted-foreground">Les montants se règlent par qui prépare la paie.</p>
        )}
      </div>

      <button type="button" onClick={() => setPlus(p => !p)} className="flex items-center gap-1 text-sm text-muted-foreground">
        {plus ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />} Plus d'informations (facultatif)
      </button>
      {plus && (
        <div className="grid sm:grid-cols-2 gap-3">
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Téléphone
            <Input value={s.telephone} onChange={e => maj({ telephone: e.target.value })} aria-label="Téléphone" className="h-9" /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Entrée le
            <input type="date" value={s.dateEntree} onChange={e => maj({ dateEntree: e.target.value })} aria-label="Entrée le" className={champ} /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Pièce d'identité
            <Input value={s.piece} onChange={e => maj({ piece: e.target.value })} aria-label="Pièce d'identité" className="h-9" /></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Compte Gescom (pour la commission)
            <select value={s.utilisateurId} onChange={e => maj({ utilisateurId: e.target.value })} aria-label="Compte Gescom" className={champ}>
              <option value="">— aucun</option>
              {comptes.filter(c => c.actif || c.id === s.utilisateurId).map(c => <option key={c.id} value={c.id}>{c.nom}</option>)}
            </select></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Magasin
            <select value={s.depotId} onChange={e => maj({ depotId: e.target.value })} aria-label="Magasin" className={champ}>
              <option value="">— aucun</option>
              {depots.map(d => <option key={d.id} value={d.id}>{d.nom}</option>)}
            </select></label>
          <div className="space-y-2">
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={s.contrat} onChange={e => maj({ contrat: e.target.checked })} /> Contrat écrit
            </label>
            {s.contrat && <input type="date" value={s.contratDate} onChange={e => maj({ contratDate: e.target.value })} aria-label="Date du contrat" className={champ} />}
          </div>
          <div className="space-y-2">
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={s.declare} onChange={e => maj({ declare: e.target.checked })} /> Déclarée (INPS)
            </label>
            {s.declare && <Input value={s.inps} onChange={e => maj({ inps: e.target.value })} aria-label="Numéro INPS" placeholder="Numéro INPS" className="h-9" />}
          </div>
          <label className="text-xs text-muted-foreground flex flex-col gap-1 sm:col-span-2">Note
            <Input value={s.note} onChange={e => maj({ note: e.target.value })} aria-label="Note" className="h-9" /></label>
        </div>
      )}

      <div className="flex gap-2">
        <Button size="sm" disabled={enCours || !s.nom.trim() || !s.fonction.trim()}
          onClick={async () => { setEnCours(true); try { await onEnregistrer(s); } finally { setEnCours(false); } }}>
          {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer"}
        </Button>
        <Button size="sm" variant="ghost" onClick={onAnnuler}>Annuler</Button>
      </div>
    </div>
  );
}

export function Personnel() {
  const [vue, setVue] = useState<"fiches" | "jours">("fiches");
  return (
    <div className="space-y-4">
      <div className="flex gap-1" role="tablist">
        {([["fiches", "Fiches"], ["jours", "Jours travaillés"]] as const).map(([cle, libelle]) => (
          <button key={cle} role="tab" aria-selected={vue === cle} onClick={() => setVue(cle)}
            className={`px-3 py-1.5 text-sm rounded-lg ${vue === cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"}`}>
            {libelle}
          </button>
        ))}
      </div>
      {vue === "fiches" ? <Fiches /> : <JoursTravailles />}
    </div>
  );
}

function Fiches() {
  const [liste, setListe] = useState<Employe[] | null>(null);
  const [avecPartis, setAvecPartis] = useState(false);
  const [edition, setEdition] = useState<Employe | "nouveau" | null>(null);
  const [depart, setDepart] = useState<{ id: string; date: string; motif: string } | null>(null);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const gere = peut("personnel:gerer");
  const voitMontants = peut("paie:preparer") || peut("paie:valider");

  const charger = useCallback(async () => {
    try {
      setListe(await invoke<Employe[]>("lire_personnel", { avecPartis }));
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }, [avecPartis]);
  useEffect(() => { charger(); }, [charger]);

  async function agir(fn: () => Promise<{ nom: string }>, texte: (nom: string) => string) {
    setAvis(null);
    try {
      const r = await fn();
      setAvis({ texte: texte(r.nom) });
      setEdition(null);
      setDepart(null);
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    }
  }

  const enregistrer = async (s: Saisie) => {
    const fiche = versFiche(s);
    // Qui gère sans voir les montants ne les efface pas en enregistrant.
    if (!voitMontants && edition && edition !== "nouveau") {
      fiche.salaire_mensuel = s.auMois ? edition.salaire_mensuel : null;
      fiche.tarif_journalier = s.aLaJournee ? edition.tarif_journalier : null;
      fiche.commission_pct = s.aLaCommission ? edition.commission_pct : null;
    }
    await agir(
      () => edition === "nouveau"
        ? invoke<Employe>("creer_employe", { fiche })
        : invoke<Employe>("modifier_employe", { employeId: (edition as Employe).id, fiche }),
      nom => edition === "nouveau" ? `${nom} ajouté(e) au personnel.` : `Fiche de ${nom} enregistrée.`,
    );
  };

  if (!liste) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  return (
    <div className="max-w-4xl space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Personnel</h1>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-sm text-muted-foreground">
            <input type="checkbox" checked={avecPartis} onChange={e => setAvecPartis(e.target.checked)} /> Voir les départs
          </label>
          {gere && (
            <Button size="sm" onClick={() => { setEdition("nouveau"); setAvis(null); }}>
              <Plus className="h-4 w-4 mr-1" /> Nouvelle personne
            </Button>
          )}
        </div>
      </div>

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      {edition === "nouveau" && (
        <Formulaire initial={VIDE} voitMontants={voitMontants} onEnregistrer={enregistrer} onAnnuler={() => setEdition(null)} />
      )}

      <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-personnel">
        {liste.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Personne pour l'instant.</p>}
        {liste.map(e => (
          <div key={e.id} data-testid="employe">
            <div className="flex items-center gap-3 px-4 py-2.5">
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium flex items-center gap-2">
                  {e.nom}
                  {e.statut === "partie" && <Badge variant="outline">partie le {e.date_depart?.split("-").reverse().join("/")}</Badge>}
                  {e.utilisateur_nom && <Badge variant="secondary">compte {e.utilisateur_nom}</Badge>}
                </div>
                <div className="text-xs text-muted-foreground">
                  {e.fonction} · {e.remuneration_dite ?? e.modes.join(" + ")}
                  {e.telephone ? ` · ${e.telephone}` : ""}
                  {!e.contrat_ecrit ? " · sans contrat écrit" : ""}
                </div>
              </div>
              {gere && e.statut === "actif" && (
                <>
                  <Button size="sm" variant="outline" onClick={() => { setEdition(e); setDepart(null); }}>Modifier</Button>
                  <Button size="sm" variant="ghost" aria-label={`Départ de ${e.nom}`}
                    onClick={() => { setDepart({ id: e.id, date: new Date().toISOString().slice(0, 10), motif: "" }); setEdition(null); }}>
                    <UserMinus className="h-4 w-4" />
                  </Button>
                </>
              )}
              {gere && e.statut === "partie" && (
                <Button size="sm" variant="ghost" aria-label={`Retour de ${e.nom}`}
                  onClick={() => agir(() => invoke<Employe>("faire_revenir_employe", { employeId: e.id }), n => `${n} est de retour.`)}>
                  <UserCheck className="h-4 w-4 mr-1" /> Revient
                </Button>
              )}
            </div>
            {depart?.id === e.id && (
              <div className="flex items-center gap-2 px-4 pb-3 text-sm flex-wrap">
                <span className="text-muted-foreground">Part le</span>
                <input type="date" value={depart.date} onChange={x => setDepart({ ...depart, date: x.target.value })}
                  aria-label="Date de départ" className="h-8 px-2 border border-border rounded-md bg-background" />
                <Input value={depart.motif} onChange={x => setDepart({ ...depart, motif: x.target.value })}
                  aria-label="Motif du départ" placeholder="motif (facultatif)" className="h-8 w-56" />
                <Button size="sm" variant="destructive"
                  onClick={() => agir(() => invoke<Employe>("faire_partir_employe", { employeId: e.id, date: depart.date, motif: depart.motif }),
                    n => `${n} est partie : sa fiche reste.`)}>
                  Confirmer le départ
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setDepart(null)}>Annuler</Button>
              </div>
            )}
            {edition !== "nouveau" && edition?.id === e.id && (
              <div className="px-4 pb-4">
                <Formulaire initial={depuis(e)} voitMontants={voitMontants} onEnregistrer={enregistrer} onAnnuler={() => setEdition(null)} />
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
