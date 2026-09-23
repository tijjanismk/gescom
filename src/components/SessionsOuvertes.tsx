// components/SessionsOuvertes.tsx — qui est connecté, et le déconnecter (v3, C-4).
//
// Paramètres → Utilisateurs. Qui, sur quel poste, depuis quand, sa
// dernière commande ; « Déconnecter » ferme la session (le poste
// revient à l'écran de connexion à son prochain appel). Déconnecter
// demande `postes:gerer`, comme dans la console du serveur.

import { useState, useEffect, useCallback } from "react";
import { appeler as invoke, posteCourant } from "@/lib/pont";
import { Loader2, LogOut, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { peut } from "@/lib/droits";

export interface SessionOuverte {
  id: string;
  poste_id: string;
  poste_nom: string;
  utilisateur_id: string;
  utilisateur_nom: string;
  role: string;
  ouvert_le: string;
  expire_le: string;
  derniere_vue: string | null;
  derniere_commande: string | null;
}

function quand(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  const auj = new Date().toDateString() === d.toDateString();
  const heure = d.toLocaleTimeString("fr-ML", { hour: "2-digit", minute: "2-digit" });
  return auj ? heure : `${d.toLocaleDateString("fr-ML", { day: "2-digit", month: "2-digit" })} ${heure}`;
}

/** `creer_vente` → « creer vente » : le nom de la commande, lisible. */
function commande(c: string | null): string {
  return c ? c.replace(/_/g, " ") : "—";
}

export function SessionsOuvertes({ revision = 0 }: { revision?: number }) {
  const [sessions, setSessions] = useState<SessionOuverte[] | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [enCours, setEnCours] = useState<string | null>(null);
  const moi = posteCourant();
  const peutDeconnecter = peut("postes:gerer");

  const charger = useCallback(async () => {
    try {
      setSessions(await invoke<SessionOuverte[]>("lire_sessions_reseau"));
      setErreur(null);
    } catch (e) {
      setErreur(String(e));
    }
  }, []);

  useEffect(() => { charger(); }, [charger, revision]);

  async function deconnecter(s: SessionOuverte) {
    setEnCours(s.id);
    try {
      await invoke("revoquer_session_reseau", { session_id: s.id });
      await charger();
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(null);
    }
  }

  return (
    <div className="space-y-2" data-testid="sessions-ouvertes">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold">Sessions ouvertes</h3>
        <Button variant="ghost" size="sm" onClick={charger} aria-label="Actualiser les sessions">
          <RefreshCw className="h-3.5 w-3.5" />
        </Button>
      </div>
      {erreur && <p className="text-xs text-red-600" role="alert">{erreur}</p>}
      {!sessions ? (
        <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
      ) : sessions.length === 0 ? (
        <p className="text-sm text-muted-foreground">Personne n'est connecté.</p>
      ) : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {sessions.map(s => (
            <div key={s.id} className="flex items-center justify-between gap-3 px-4 py-2.5"
              data-testid="session">
              <div className="min-w-0">
                <p className="text-sm font-medium">
                  {s.utilisateur_nom} <span className="text-xs text-muted-foreground">({s.role})</span>
                </p>
                <p className="text-xs text-muted-foreground truncate">
                  {s.poste_nom} · depuis {quand(s.ouvert_le)} · dernière action {quand(s.derniere_vue)}
                  {s.derniere_commande ? ` (${commande(s.derniere_commande)})` : ""}
                </p>
              </div>
              {s.poste_id === moi ? (
                <span className="text-xs text-muted-foreground shrink-0">ce poste</span>
              ) : peutDeconnecter ? (
                <Button variant="outline" size="sm" className="shrink-0"
                  disabled={enCours === s.id} onClick={() => deconnecter(s)}
                  aria-label={`Déconnecter ${s.utilisateur_nom} sur ${s.poste_nom}`}>
                  {enCours === s.id
                    ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    : <><LogOut className="h-3.5 w-3.5 mr-1" /> Déconnecter</>}
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
