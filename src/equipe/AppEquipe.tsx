// equipe/AppEquipe.tsx — Gescom Équipe (PLAN-EQUIPE, D28).
//
// Une seconde fenêtre, qui ne parle qu'au serveur Gescom : aucune base
// à elle. Mêmes comptes, mêmes droits (par dossier, C-2), même choix de
// dossier à la connexion — l'écran de connexion est celui de Gescom.
// Sa session est à elle (clé `gescom_equipe_session`) : se déconnecter
// d'Équipe ne ferme pas la caisse du même poste, et inversement.

import { useState, useEffect } from "react";
import { Loader2, LogOut, ServerOff } from "lucide-react";
import {
  synchroniserConfig, enReseau, sessionUtilisable, surSessionPerdue, deconnecterServeur,
  dossierCourant, etatReseau,
} from "@/lib/pont";
import { poserDroits, peutUne } from "@/lib/droits";
import { PageLogin, type UtilisateurConnecte } from "@/pages/PageLogin";
import { Button } from "@/components/ui/button";
import { ModalChangerMdp } from "@/components/ModalChangerMdp";
import { MODULES } from "./modules";
import { Accueil } from "./Accueil";
import { Bientot } from "./Bientot";
import { Personnel } from "./Personnel";

const CLE_SESSION = "gescom_equipe_session";

function lireSession(): UtilisateurConnecte | null {
  try {
    const s = JSON.parse(localStorage.getItem(CLE_SESSION) ?? "null");
    return s && Array.isArray(s.permissions) ? s : null;
  } catch {
    return null;
  }
}

function ecrireSession(u: UtilisateurConnecte | null) {
  try {
    if (u) localStorage.setItem(CLE_SESSION, JSON.stringify(u));
    else localStorage.removeItem(CLE_SESSION);
  } catch { /* stockage bloqué : on redemandera la connexion */ }
}

export function AppEquipe() {
  const [pontPret, setPontPret] = useState(false);
  const [utilisateur, setUtilisateur] = useState<UtilisateurConnecte | null>(() => {
    const u = lireSession();
    poserDroits(u);
    return u;
  });
  const [page, setPage] = useState("accueil");

  useEffect(() => {
    synchroniserConfig().finally(() => setPontPret(true));
  }, []);

  function quitter() {
    ecrireSession(null);
    poserDroits(null);
    setUtilisateur(null);
    setPage("accueil");
  }

  // Le jeton du serveur est la source (comme dans Gescom) : un
  // redémarrage du serveur ramène à la connexion, pas à un écran muet.
  useEffect(() => {
    if (!pontPret) return;
    if (utilisateur && !sessionUtilisable()) {
      quitter();
      return;
    }
    return surSessionPerdue(quitter);
  }, [pontPret, utilisateur?.id]);

  if (!pontPret) {
    return (
      <div className="h-screen flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // D28 : pas de base locale. Un poste réglé en monoposte n'a pas de
  // serveur à qui parler — le dire, avec quoi faire.
  if (!enReseau()) {
    return (
      <div className="h-screen flex items-center justify-center p-8" data-testid="sans-serveur">
        <div className="max-w-md space-y-3 text-sm">
          <ServerOff className="h-8 w-8 text-muted-foreground" />
          <h1 className="text-xl font-semibold">Gescom Équipe a besoin du serveur Gescom</h1>
          <p className="text-muted-foreground">
            Ce poste est réglé en « une seule caisse », sans serveur. Équipe n'a pas de base à elle :
            elle lit et écrit par le serveur, pour que les droits, les dossiers et l'historique
            restent les mêmes que dans Gescom.
          </p>
          <p className="text-muted-foreground">
            Lancer le serveur Gescom (sur cette machine, il travaille sur la même base), puis régler
            ce poste sur « plusieurs caisses » dans Gescom → Paramètres → Réseau.
          </p>
        </div>
      </div>
    );
  }

  if (!utilisateur) {
    return (
      <PageLogin
        titre="Gescom Équipe"
        sousTitre="Personnel, paie, suivi des clients"
        onConnecte={u => {
          ecrireSession(u);
          poserDroits(u);
          setUtilisateur(u);
          setPage("accueil");
        }}
      />
    );
  }

  const visibles = MODULES.filter(m => m.droits.length === 0 || peutUne(...m.droits));
  const courant = visibles.find(m => m.cle === page) ?? visibles[0];
  const dossier = dossierCourant();

  return (
    <div className="h-screen flex bg-background">
      <aside className="w-60 shrink-0 border-r border-border flex flex-col">
        <div className="px-4 py-4 border-b border-border">
          <div className="font-semibold">Gescom Équipe</div>
          <div className="text-xs text-muted-foreground truncate" data-testid="dossier-ouvert">
            {dossier?.societe ?? etatReseau().serveur}
          </div>
        </div>
        <nav className="flex-1 p-2 space-y-1" aria-label="Menu Équipe">
          {visibles.map(m => {
            const Icone = m.icone;
            return (
              <button key={m.cle} onClick={() => setPage(m.cle)}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                  courant.cle === m.cle ? "bg-primary text-primary-foreground" : "hover:bg-muted"
                }`}>
                <Icone className="h-4 w-4" />
                <span className="flex-1 text-left">{m.libelle}</span>
                {m.aVenir && <span className="text-[10px] opacity-70">bientôt</span>}
              </button>
            );
          })}
        </nav>
        <div className="p-3 border-t border-border flex items-center gap-2">
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium truncate">{utilisateur.nom}</div>
            <div className="text-xs text-muted-foreground truncate">{utilisateur.role}</div>
          </div>
          <Button size="sm" variant="ghost" aria-label="Se déconnecter"
            onClick={async () => { await deconnecterServeur(); quitter(); }}>
            <LogOut className="h-4 w-4" />
          </Button>
        </div>
      </aside>
      <main className="flex-1 overflow-auto p-8">
        {courant.cle === "accueil"
          ? <Accueil nom={utilisateur.nom} modules={visibles.filter(m => m.cle !== "accueil")} onOuvrir={setPage} />
          : courant.cle === "personnel" ? <Personnel />
          : <Bientot module={courant} />}
      </main>

      {/* Un mot de passe d'usine ou donné par le patron se change avant
          tout, ici comme dans Gescom. */}
      <ModalChangerMdp
        ouvert={utilisateur.doit_changer_mdp}
        utilisateurId={utilisateur.id}
        obligatoire
        onFermer={() => undefined}
        onChange={() => {
          const u = { ...utilisateur, doit_changer_mdp: false };
          ecrireSession(u);
          setUtilisateur(u);
        }}
      />
    </div>
  );
}
