// equipe/FichesPaie.tsx — les fiches de paie du mois (PLAN-EQUIPE, G-2 — D31).
//
// Préparer (un brouillon calculé depuis la fiche de la personne, ses
// jours, ses ventes, ses avances), corriger (prime, tâche, retenue),
// valider (numéroté, figé), rectifier (une nouvelle fiche qui remplace).
// Tous les calculs sont faits par le serveur ; l'écran ne fait qu'afficher.

import { useState, useEffect, useCallback } from "react";
import { Loader2, FileCheck2, RefreshCw, Trash2, X, Plus, FilePen, Banknote, Printer } from "lucide-react";
import { ApercuBulletin } from "./ApercuBulletin";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

interface LignePaie {
  id: string; genre: string; libelle: string; quantite: number | null; prix: number | null;
  montant: number; source: string | null; saisie: boolean;
}
interface FichePaie {
  id: string; employe_id: string; nom: string; fonction: string; du: string; au: string; prorata: boolean;
  statut: "brouillon" | "validee" | "remplacee"; numero: string | null; brut: number; retenues: number; net: number;
  reporte: number; rectifie_numero: string | null; remplacee_par: string | null; valide_le: string | null;
  valide_par_nom: string | null; lignes?: LignePaie[];
  verse: number; reste: number; versements?: { id: string; montant: number; moyen: string; date: string; par: string | null }[];
}
interface Mois { fiches: FichePaie[]; a_preparer: { employe_id: string; nom: string; fonction: string; au_mois: boolean }[] }

const f = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const jj = (iso: string) => iso.slice(0, 10).split("-").reverse().join("/");
const MOYENS: [string, string][] = [["especes", "Espèces"], ["orange_money", "Orange Money"], ["moov_money", "Moov Money"], ["virement", "Virement"], ["cheque", "Chèque"]];
const GENRES: [string, string][] = [["prime", "Prime"], ["tache", "À la tâche"], ["retenue", "Retenue (casse, absence…)"]];

function bornes(mois: string): [string, string] {
  const [a, m] = mois.split("-").map(Number);
  const fin = new Date(a, m, 0).getDate();
  return [`${mois}-01`, `${mois}-${String(fin).padStart(2, "0")}`];
}

function Statut({ fiche }: { fiche: FichePaie }) {
  if (fiche.statut === "brouillon") return <Badge variant="outline">brouillon</Badge>;
  if (fiche.statut === "remplacee") return <Badge variant="outline" className="line-through">{fiche.numero}</Badge>;
  return <>
    <Badge variant="secondary">{fiche.numero}</Badge>
    {fiche.reste <= 0 ? <Badge className="bg-emerald-600">payée</Badge> : fiche.verse > 0 ? <Badge variant="outline">reste {f(fiche.reste)}</Badge> : null}
  </>;
}

function Detail({ id, fermer, ouvrir, changee, dire }: {
  id: string; fermer: () => void; ouvrir: (id: string) => void; changee: () => void; dire: (t: string, erreur?: boolean) => void;
}) {
  const [fiche, setFiche] = useState<FichePaie | null>(null);
  const [enCours, setEnCours] = useState(false);
  const [genre, setGenre] = useState("prime");
  const [libelle, setLibelle] = useState("");
  const [quantite, setQuantite] = useState("");
  const [prix, setPrix] = useState("");
  const [montant, setMontant] = useState("");
  const [aVerser, setAVerser] = useState("");
  const [moyen, setMoyen] = useState("especes");
  const [bulletin, setBulletin] = useState(false);
  const prepare = peut("paie:preparer");
  const valide = peut("paie:valider");

  useEffect(() => {
    invoke<FichePaie>("lire_fiche_paie", { ficheId: id }).then(setFiche).catch(e => dire(String(e), true));
  }, [id, dire]);

  async function faire(cmd: string, args: Record<string, unknown>, ok?: (r: FichePaie) => string) {
    setEnCours(true);
    try {
      const r = await invoke<FichePaie>(cmd, args);
      if (r && (r as FichePaie).lignes) setFiche(r);
      if (ok) dire(ok(r));
      changee();
      return r;
    } catch (e) {
      dire(String(e), true);
    } finally {
      setEnCours(false);
    }
  }

  if (!fiche) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;
  const brouillon = fiche.statut === "brouillon";
  const nombre = (s: string) => Number(s.replace(/\s/g, "").replace(",", "."));

  return (
    <div className="border border-border rounded-lg p-4 space-y-3" data-testid="fiche-paie">
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <h3 className="font-semibold">{fiche.nom} <span className="text-muted-foreground font-normal">— {fiche.fonction}</span></h3>
          <p className="text-xs text-muted-foreground">
            du {jj(fiche.du)} au {jj(fiche.au)}
            {fiche.rectifie_numero && <> · rectifie {fiche.rectifie_numero}</>}
            {fiche.statut === "validee" && fiche.valide_le && <> · validée le {jj(fiche.valide_le)}{fiche.valide_par_nom ? ` par ${fiche.valide_par_nom}` : ""}</>}
            {fiche.remplacee_par && <> · remplacée par {fiche.remplacee_par}</>}
          </p>
        </div>
        <Statut fiche={fiche} />
        <Button size="sm" variant="ghost" aria-label="Fermer la fiche" onClick={fermer}><X className="h-4 w-4" /></Button>
      </div>

      <table className="w-full text-sm" data-testid="lignes-paie">
        <tbody className="divide-y divide-border">
          {(fiche.lignes ?? []).map(l => (
            <tr key={l.id} data-testid="ligne-paie">
              <td className="py-1.5">{l.libelle}{l.saisie && <span className="text-xs text-muted-foreground"> · saisie</span>}</td>
              <td className={`py-1.5 text-right tabular-nums ${l.montant < 0 ? "text-red-700" : ""}`}>{l.montant < 0 ? "− " : ""}{f(Math.abs(l.montant))}</td>
              <td className="w-8 text-right">
                {brouillon && prepare && l.saisie && (
                  <Button size="sm" variant="ghost" aria-label={`Retirer « ${l.libelle} »`} disabled={enCours}
                    onClick={() => faire("retirer_ligne_paie", { ficheId: fiche.id, ligneId: l.id })}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
        <tfoot className="border-t-2 border-border">
          <tr><td className="pt-2 text-muted-foreground">Brut</td><td className="pt-2 text-right tabular-nums">{f(fiche.brut)}</td><td /></tr>
          <tr><td className="text-muted-foreground">Retenues</td><td className="text-right tabular-nums">{f(fiche.retenues)}</td><td /></tr>
          <tr className="font-semibold"><td>Net à payer</td><td className="text-right tabular-nums" data-testid="net-a-payer">{f(fiche.net)}</td><td /></tr>
        </tfoot>
      </table>
      {fiche.statut !== "brouillon" && (
        <div className="border-t border-border pt-3 space-y-2" data-testid="versements">
          <p className="text-sm">
            Versé <strong className="tabular-nums">{f(fiche.verse)}</strong>
            {fiche.statut === "validee" && (fiche.reste > 0
              ? <> · reste à verser <strong className="tabular-nums" data-testid="reste-a-verser">{f(fiche.reste)}</strong></>
              : fiche.reste < 0 ? <> · <span className="text-amber-700">versé en trop : {f(-fiche.reste)}</span></>
              : <> · <span className="text-emerald-700">payée</span></>)}
          </p>
          {(fiche.versements ?? []).map(v => (
            <p key={v.id} className="text-xs text-muted-foreground" data-testid="versement">
              {f(v.montant)} le {jj(v.date)} · {MOYENS.find(m => m[0] === v.moyen)?.[1] ?? v.moyen}{v.par ? ` · par ${v.par}` : ""}
            </p>
          ))}
          {fiche.statut === "validee" && valide && fiche.reste > 0 && (
            <div className="flex gap-2 items-end flex-wrap">
              <label className="text-xs text-muted-foreground flex flex-col gap-1">Montant
                <Input value={aVerser} onChange={e => setAVerser(e.target.value)} aria-label="Montant à verser"
                  placeholder={String(fiche.reste)} inputMode="numeric" className="h-9 w-32" /></label>
              <label className="text-xs text-muted-foreground flex flex-col gap-1">Par
                <select value={moyen} onChange={e => setMoyen(e.target.value)} aria-label="Versé par"
                  className="h-9 px-2 text-sm border border-border rounded-md bg-background">
                  {MOYENS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                </select></label>
              <Button size="sm" disabled={enCours}
                onClick={async () => {
                  const m = aVerser.trim() ? nombre(aVerser) : fiche.reste;
                  const r = await faire("verser_paie", { ficheId: fiche.id, montant: m, moyen },
                    r => `${f(m)} versés à ${r.nom}, sortis de la caisse.${r.reste > 0 ? ` Reste ${f(r.reste)}.` : " Fiche payée."}`);
                  if (r) setAVerser("");
                }}>
                <Banknote className="h-4 w-4 mr-1" /> Verser
              </Button>
              <p className="text-xs text-muted-foreground w-full">Vide : tout ce qui reste. La caisse doit être ouverte.</p>
            </div>
          )}
        </div>
      )}

      {fiche.reporte > 0 && (
        <p className="text-xs text-amber-700" data-testid="reporte">{f(fiche.reporte)} d'avances n'ont pas pu être retenus : ils le seront sur la prochaine fiche.</p>
      )}

      {brouillon && prepare && (
        <div className="flex gap-2 flex-wrap items-end border-t border-border pt-3">
          <label className="text-xs text-muted-foreground flex flex-col gap-1">Ajouter
            <select value={genre} onChange={e => setGenre(e.target.value)} aria-label="Genre de ligne"
              className="h-9 px-2 text-sm border border-border rounded-md bg-background">
              {GENRES.map(([v, t]) => <option key={v} value={v}>{t}</option>)}
            </select></label>
          <label className="text-xs text-muted-foreground flex flex-col gap-1 flex-1 min-w-36">Libellé
            <Input value={libelle} onChange={e => setLibelle(e.target.value)} aria-label="Libellé de la ligne" className="h-9"
              placeholder={genre === "tache" ? "Livraisons" : genre === "retenue" ? "Casse d'un carton" : "Prime"} /></label>
          {genre === "tache" ? (<>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Combien
              <Input value={quantite} onChange={e => setQuantite(e.target.value)} aria-label="Quantité" inputMode="decimal" className="h-9 w-20" /></label>
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Prix de chacune
              <Input value={prix} onChange={e => setPrix(e.target.value)} aria-label="Prix unitaire" inputMode="numeric" className="h-9 w-28" /></label>
          </>) : (
            <label className="text-xs text-muted-foreground flex flex-col gap-1">Montant
              <Input value={montant} onChange={e => setMontant(e.target.value)} aria-label="Montant de la ligne" inputMode="numeric" className="h-9 w-28" /></label>
          )}
          <Button size="sm" variant="outline" disabled={enCours}
            onClick={async () => {
              const r = await faire("ajouter_ligne_paie", {
                ficheId: fiche.id, genre, libelle,
                quantite: genre === "tache" && quantite.trim() ? nombre(quantite) : null,
                prix: genre === "tache" && prix.trim() ? nombre(prix) : null,
                montant: genre !== "tache" && montant.trim() ? nombre(montant) : null,
              });
              if (r) { setLibelle(""); setQuantite(""); setPrix(""); setMontant(""); }
            }}>
            <Plus className="h-4 w-4 mr-1" /> Ajouter la ligne
          </Button>
        </div>
      )}

      <div className="flex gap-2 flex-wrap items-center border-t border-border pt-3">
        {brouillon && prepare && (<>
          {fiche.lignes?.some(l => l.genre === "base") && (
            <label className="flex items-center gap-2 text-sm text-muted-foreground mr-2">
              <input type="checkbox" checked={fiche.prorata} disabled={enCours}
                onChange={e => faire("recalculer_fiche_paie", { ficheId: fiche.id, prorata: e.target.checked })} />
              Salaire au prorata des jours travaillés
            </label>
          )}
          <Button size="sm" variant="outline" disabled={enCours}
            onClick={() => faire("recalculer_fiche_paie", { ficheId: fiche.id }, () => "Fiche recalculée.")}>
            <RefreshCw className="h-4 w-4 mr-1" /> Recalculer
          </Button>
          <Button size="sm" variant="ghost" disabled={enCours}
            onClick={async () => { if (await faire("supprimer_fiche_paie", { ficheId: fiche.id }, () => `Brouillon de ${fiche.nom} jeté.`)) fermer(); }}>
            <Trash2 className="h-4 w-4 mr-1" /> Jeter le brouillon
          </Button>
        </>)}
        {brouillon && valide && (
          <Button size="sm" disabled={enCours} className="ml-auto"
            onClick={() => faire("valider_fiche_paie", { ficheId: fiche.id }, r => `Fiche ${r.numero} validée : ${f(r.net)} à payer à ${r.nom}. Elle ne changera plus.`)}>
            <FileCheck2 className="h-4 w-4 mr-1" /> Valider la fiche
          </Button>
        )}
        {brouillon && !valide && <p className="text-xs text-muted-foreground ml-auto">La validation revient à qui a le droit de valider la paie.</p>}
        {fiche.statut !== "brouillon" && (
          <Button size="sm" variant="outline" onClick={() => setBulletin(true)}>
            <Printer className="h-4 w-4 mr-1" /> Bulletin
          </Button>
        )}
        {fiche.statut === "validee" && valide && (
          <Button size="sm" variant="outline" disabled={enCours}
            onClick={async () => {
              const r = await faire("rectifier_fiche_paie", { ficheId: fiche.id }, () => `Rectificative ouverte : ${fiche.numero} reste valable jusqu'à sa validation.`);
              if (r) ouvrir(r.id);
            }}>
            <FilePen className="h-4 w-4 mr-1" /> Faire une rectificative
          </Button>
        )}
      </div>
      {bulletin && <ApercuBulletin ficheId={fiche.id} fermer={() => setBulletin(false)} />}
    </div>
  );
}

export function FichesPaie() {
  const [mois, setMois] = useState(() => new Date().toISOString().slice(0, 7));
  const [donnees, setDonnees] = useState<Mois | null>(null);
  const [ouverte, setOuverte] = useState<string | null>(null);
  const [prorata, setProrata] = useState<Record<string, boolean>>({});
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const [enCours, setEnCours] = useState(false);
  const prepare = peut("paie:preparer");
  const [du, au] = bornes(mois);

  const dire = useCallback((texte: string, erreur?: boolean) => setAvis({ texte, erreur }), []);
  const charger = useCallback(async () => {
    try {
      setDonnees(await invoke<Mois>("lire_fiches_paie", { du, au }));
    } catch (e) {
      dire(String(e), true);
    }
  }, [du, au, dire]);
  useEffect(() => { setDonnees(null); setOuverte(null); charger(); }, [charger]);

  async function preparer(p: Mois["a_preparer"][number]) {
    setEnCours(true);
    setAvis(null);
    try {
      const r = await invoke<FichePaie>("preparer_fiche_paie", { employeId: p.employe_id, du, au, prorata: !!prorata[p.employe_id] });
      dire(`Brouillon de ${r.nom} préparé : ${f(r.net)} net. À relire avant de valider.`);
      await charger();
      setOuverte(r.id);
    } catch (e) {
      dire(String(e), true);
    } finally {
      setEnCours(false);
    }
  }

  return (
    <div className="space-y-4 max-w-3xl">
      <div className="flex items-center gap-3">
        <label className="text-sm text-muted-foreground flex items-center gap-2">Mois
          <Input type="month" value={mois} onChange={e => e.target.value && setMois(e.target.value)} aria-label="Mois de paie" className="h-9 w-44" />
        </label>
        <span className="text-xs text-muted-foreground">du {jj(du)} au {jj(au)}</span>
      </div>

      {avis && <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>}

      {!donnees ? <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" /> : (<>
        {donnees.a_preparer.length > 0 && (
          <div className="border border-border rounded-lg divide-y divide-border" data-testid="a-preparer">
            <h3 className="px-4 py-2 text-sm font-semibold">À préparer</h3>
            {donnees.a_preparer.map(p => (
              <div key={p.employe_id} className="flex items-center gap-3 px-4 py-2 text-sm">
                <div className="flex-1">{p.nom} <span className="text-muted-foreground">({p.fonction})</span></div>
                {p.au_mois && (
                  <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <input type="checkbox" checked={!!prorata[p.employe_id]} aria-label={`Au prorata pour ${p.nom}`}
                      onChange={e => setProrata({ ...prorata, [p.employe_id]: e.target.checked })} /> au prorata
                  </label>
                )}
                {prepare && (
                  <Button size="sm" variant="outline" disabled={enCours} onClick={() => preparer(p)}>
                    Préparer la fiche de {p.nom}
                  </Button>
                )}
              </div>
            ))}
          </div>
        )}

        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-fiches">
          {donnees.fiches.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">Aucune fiche pour ce mois.</p>}
          {donnees.fiches.map(fi => (
            <button key={fi.id} data-testid="fiche" onClick={() => setOuverte(fi.id)}
              className={`w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-muted ${ouverte === fi.id ? "bg-muted" : ""}`}>
              <div className="flex-1">
                <span className={fi.statut === "remplacee" ? "text-muted-foreground" : "font-medium"}>{fi.nom}</span>
                <span className="text-xs text-muted-foreground"> · du {jj(fi.du)} au {jj(fi.au)}{fi.rectifie_numero ? ` · rectifie ${fi.rectifie_numero}` : ""}</span>
              </div>
              <Statut fiche={fi} />
              <span className="tabular-nums font-semibold w-28 text-right">{f(fi.net)}</span>
            </button>
          ))}
        </div>

        {ouverte && <Detail key={ouverte} id={ouverte} fermer={() => setOuverte(null)} ouvrir={setOuverte} changee={charger} dire={dire} />}
      </>)}
    </div>
  );
}
