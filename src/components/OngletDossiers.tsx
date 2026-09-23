// components/OngletDossiers.tsx — les dossiers (sociétés), v3 D-3.
//
// Un dossier = une société : ses clients, son stock, sa caisse, ses
// numéros. Il naît avec ses DATES DE TRAVAIL (D21), proposées à l'année
// civile — son premier exercice —, son magasin et son client de passage.
// On y travaille en le choisissant à la connexion : changer de dossier,
// c'est se déconnecter (plan multi-société, décision 3).

import { useState, useEffect, useCallback } from "react";
import { appeler as invoke, dossierCourant } from "@/lib/pont";
import { Loader2, Plus, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { jourEnLettres } from "@/lib/utils";
import { ExercicesDossier } from "@/components/ExercicesDossier";

interface Dossier {
  id: string;
  code: string;
  societe: string;
  clos: boolean;
  exercices_ouverts: number;
}

export function OngletDossiers() {
  const annee = new Date().getFullYear();
  const [dossiers, setDossiers] = useState<Dossier[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [societe, setSociete] = useState("");
  const [du, setDu] = useState(`${annee}-01-01`);
  const [au, setAu] = useState(`${annee}-12-31`);
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);
  const courant = dossierCourant();

  const charger = useCallback(async () => {
    try {
      setDossiers(await invoke<Dossier[]>("lire_dossiers"));
    } catch (e) {
      setErreur(String(e));
    }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  async function creer() {
    setEnCours(true);
    setAvis(null);
    try {
      const r = await invoke<{ code: string; societe: string; date_debut: string; date_fin: string }>(
        "creer_dossier", { code, societe, dateDebut: du || null, dateFin: au || null });
      setAvis({
        texte: `Dossier ${r.code} créé — dates de travail du ${jourEnLettres(r.date_debut)} au ${jourEnLettres(r.date_fin)}. ` +
          "Pour y travailler : se déconnecter, puis le choisir à la connexion.",
      });
      setCode(""); setSociete("");
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  if (erreur) return <p className="text-sm text-red-600 py-4">{erreur}</p>;
  if (!dossiers) return <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />;

  return (
    <div className="space-y-6 max-w-3xl">
      <div className="space-y-2">
        <p className="text-sm text-muted-foreground">
          Un dossier est une société : ses clients, son stock, sa caisse, ses
          numéros de pièces. Les articles et leurs prix sont communs à tous.
        </p>
        <div className="border border-border rounded-lg divide-y divide-border" data-testid="liste-dossiers">
          {dossiers.map(d => (
            <div key={d.id} className="flex items-center justify-between px-4 py-2.5" data-testid="dossier">
              <div className="flex items-center gap-2">
                <FolderOpen className="h-4 w-4 text-muted-foreground" />
                <span className="text-sm font-medium">{d.societe}</span>
                <span className="text-xs text-muted-foreground">{d.code}</span>
                {courant?.id === d.id && <Badge variant="secondary">ouvert ici</Badge>}
                {d.clos && <Badge variant="outline">clos</Badge>}
              </div>
              <span className="text-xs text-muted-foreground">
                {d.exercices_ouverts} exercice{d.exercices_ouverts > 1 ? "s" : ""} ouvert{d.exercices_ouverts > 1 ? "s" : ""}
              </span>
            </div>
          ))}
        </div>
      </div>

      <ExercicesDossier />

      <div className="border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-semibold">Nouveau dossier</h3>
        <div className="grid grid-cols-2 gap-3">
          <label className="text-xs text-muted-foreground">
            Code (lettres, chiffres)
            <Input value={code} onChange={e => setCode(e.target.value)} aria-label="Code du dossier"
              placeholder="QUINC" className="mt-1 h-9" />
          </label>
          <label className="text-xs text-muted-foreground">
            Société
            <Input value={societe} onChange={e => setSociete(e.target.value)} aria-label="Société"
              placeholder="Quincaillerie du Fleuve" className="mt-1 h-9" />
          </label>
          <label className="text-xs text-muted-foreground">
            Je travaille dedans du
            <input type="date" value={du} onChange={e => setDu(e.target.value)} aria-label="Début des dates de travail"
              className="mt-1 h-9 w-full px-2 text-sm border border-border rounded-md bg-background block" />
          </label>
          <label className="text-xs text-muted-foreground">
            au
            <input type="date" value={au} onChange={e => setAu(e.target.value)} aria-label="Fin des dates de travail"
              className="mt-1 h-9 w-full px-2 text-sm border border-border rounded-md bg-background block" />
          </label>
        </div>
        <p className="text-xs text-muted-foreground">
          Les dates de travail sont le premier exercice du dossier. Une vente
          ou une pièce hors de ces dates sera refusée ; elles se prolongent.
          Le dossier naît avec son magasin principal et son client de passage.
        </p>
        <Button size="sm" onClick={creer} disabled={enCours || !code.trim() || !societe.trim()}>
          {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Plus className="h-4 w-4 mr-1" /> Créer le dossier</>}
        </Button>
        {avis && (
          <p className={`text-sm ${avis.erreur ? "text-red-600" : "text-emerald-700"}`} role="status">{avis.texte}</p>
        )}
      </div>
    </div>
  );
}
