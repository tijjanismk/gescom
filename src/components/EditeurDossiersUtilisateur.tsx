// components/EditeurDossiersUtilisateur.tsx — dans quels dossiers une
// personne entre, et avec quel rôle (v3, C-2 — décision C2).
//
// « Ton frère est patron de sa boutique et n'a rien à faire dans la
// tienne. Ta comptable voit les deux. » Deux façons :
// - comme son rôle : ce qui valait avant la v3 (accès total → tous les
//   dossiers ; sinon le dossier d'origine) ;
// - par dossier : exactement ceux cochés, chacun avec son rôle. Qui n'a
//   que certains dossiers ne gère ni les comptes, ni les postes, ni la
//   sauvegarde, ni les paramètres de la société (communs à tous).
// Le serveur juge ; il relit le rôle à chaque requête.

import { useState, useEffect, useCallback } from "react";
import { Loader2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/** La fenêtre « Dossiers de … » (Paramètres → Utilisateurs). */
export function ModalDossiersUtilisateur({ utilisateur, onFermer }: {
  utilisateur: { id: string; nom: string } | null;
  onFermer: () => void;
}) {
  return (
    <Dialog open={utilisateur !== null} onOpenChange={o => { if (!o) onFermer(); }}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{utilisateur ? `Dossiers de ${utilisateur.nom}` : "Dossiers"}</DialogTitle>
        </DialogHeader>
        {utilisateur && <EditeurDossiersUtilisateur utilisateurId={utilisateur.id} nom={utilisateur.nom} />}
      </DialogContent>
    </Dialog>
  );
}

interface Etat {
  role_global: string;
  acces_total: boolean;
  superadmin: boolean;
  par_dossier: boolean;
  dossiers: { id: string; code: string; societe: string; role: string | null }[];
}

export function EditeurDossiersUtilisateur({ utilisateurId, nom }: { utilisateurId: string; nom: string }) {
  const [etat, setEtat] = useState<Etat | null>(null);
  const [roles, setRoles] = useState<string[]>([]);
  const [parDossier, setParDossier] = useState(false);
  const [choix, setChoix] = useState<Record<string, string>>({});
  const [enCours, setEnCours] = useState(false);
  const [avis, setAvis] = useState<{ texte: string; erreur?: boolean } | null>(null);

  const charger = useCallback(async () => {
    const [e, r] = await Promise.all([
      invoke<Etat>("lire_dossiers_utilisateur", { utilisateurId }),
      invoke<{ nom: string }[]>("lire_roles"),
    ]);
    setEtat(e);
    setRoles(r.map(x => x.nom).filter(n => n !== "superadmin"));
    setParDossier(e.par_dossier);
    setChoix(Object.fromEntries(e.dossiers.map(d => [d.id, d.role ?? ""])));
  }, [utilisateurId]);
  useEffect(() => { charger().catch(e => setAvis({ texte: String(e), erreur: true })); }, [charger]);

  async function enregistrer() {
    setEnCours(true);
    setAvis(null);
    try {
      const dossiers = parDossier
        ? Object.entries(choix).filter(([, r]) => r).map(([dossier_id, role]) => ({ dossier_id, role }))
        : null;
      await invoke("definir_dossiers_utilisateur", { utilisateurId, dossiers });
      setAvis({ texte: parDossier ? `Dossiers de ${nom} enregistrés.` : `${nom} suit de nouveau son rôle.` });
      await charger();
    } catch (e) {
      setAvis({ texte: String(e), erreur: true });
    } finally {
      setEnCours(false);
    }
  }

  // Un seul dossier : rien à régler, la section n'apparaît pas.
  if (!etat || etat.dossiers.length < 2) return null;
  const titre = (
    <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground mb-2">Dossiers</p>
  );
  if (etat.superadmin) {
    return (
      <div className="border border-border rounded-lg p-3">
        {titre}
        <p className="text-sm text-muted-foreground">Compte de secours : il entre dans tous les dossiers.</p>
      </div>
    );
  }
  const regle = etat.acces_total
    ? "tous les dossiers"
    : `le dossier d'origine seulement`;

  return (
    <div className="border border-border rounded-lg p-3 space-y-2 text-sm" data-testid="dossiers-utilisateur">
      {titre}
      <label className="flex items-center gap-2">
        <input type="radio" name={`mode-${utilisateurId}`} checked={!parDossier} onChange={() => setParDossier(false)} />
        Comme son rôle « {etat.role_global} » : {regle}
      </label>
      <label className="flex items-center gap-2">
        <input type="radio" name={`mode-${utilisateurId}`} checked={parDossier} onChange={() => setParDossier(true)} />
        Par dossier
      </label>
      {parDossier && (
        <div className="border border-border rounded-md divide-y divide-border ml-6">
          {etat.dossiers.map(d => (
            <div key={d.id} className="flex items-center justify-between px-3 py-1.5 gap-2">
              <span>{d.societe} <span className="text-xs text-muted-foreground">{d.code}</span></span>
              <select aria-label={`Rôle dans ${d.societe}`} value={choix[d.id] ?? ""}
                onChange={e => setChoix(c => ({ ...c, [d.id]: e.target.value }))}
                className="h-8 px-2 text-sm border border-border rounded-md bg-background">
                <option value="">— pas d'accès</option>
                {roles.map(r => <option key={r} value={r}>{r}</option>)}
              </select>
            </div>
          ))}
        </div>
      )}
      {parDossier && (
        <p className="text-xs text-muted-foreground ml-6">
          Qui n'a que certains dossiers ne gère ni les comptes, ni les postes, ni la sauvegarde, ni les
          paramètres de la société : ils sont communs à tous les dossiers.
        </p>
      )}
      <div className="flex items-center gap-3">
        <Button size="sm" variant="outline" onClick={enregistrer} disabled={enCours}>
          {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer les dossiers"}
        </Button>
        {avis && (
          <span className={avis.erreur ? "text-red-600" : "text-emerald-700"} role="status">{avis.texte}</span>
        )}
      </div>
    </div>
  );
}
