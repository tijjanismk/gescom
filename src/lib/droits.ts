// lib/droits.ts — ce que l'utilisateur a le droit de faire.
//
// L'écran décidait d'après le NOM du rôle : `role === "patron"`. Cela
// marchait tant qu'il n'y avait que deux rôles. Depuis que le
// commerçant crée les siens, un « caissier » tombait dans la liste de
// l'employé, et un rôle fabriqué à la main n'avait droit à rien.
//
// Le serveur calcule les permissions à la connexion et les renvoie avec
// l'identité. L'écran ne fait plus que les lire.
//
// ⚠️ Ceci ne SÉCURISE rien : c'est du confort. Cacher un bouton évite
// qu'on clique dessus pour rien, mais le refus qui compte est celui du
// noyau, à chaque appel. Un écran périmé ne peut pas ouvrir une porte
// que le serveur ferme.

import { appeler, enReseau } from "@/lib/pont";

// Les droits de la personne connectée, posés par la fenêtre qui l'a
// connectée (Gescom ou Équipe) : ce fichier ne dépend d'aucune des deux.
let actif: { permissions?: string[] } | null = null;

/** La personne connectée change (connexion, dossier choisi, déconnexion). */
export function poserDroits(u: { permissions?: string[] } | null): void {
  actif = u;
}

/** Cette personne peut-elle faire cela ? */
export function peut(permission: string): boolean {
  return actif?.permissions?.includes(permission) ?? false;
}

/**
 * Relit les droits de la session. La session gardée par le navigateur
 * (8 h) porte les permissions de la connexion : une permission ajoutée
 * depuis — par une mise à jour ou par le patron — restait invisible
 * jusqu'à la reconnexion (« les chiffres ne sont pas ouverts à votre
 * compte » chez le patron). Rend l'utilisateur à jour s'il a changé,
 * sinon null ; une erreur ne change rien (le serveur juge de toute façon).
 */
export async function relireDroits<U extends { id: string; role?: string; permissions?: string[] }>(u: U): Promise<U | null> {
  try {
    let role: string;
    let permissions: string[];
    if (enReseau()) {
      ({ role, permissions } = await appeler<{ role: string; permissions: string[] }>("lire_mes_droits"));
    } else {
      const r = await appeler<{ role: string; effectives: string[] }>("lire_permissions_utilisateur", { utilisateurId: u.id });
      ({ role, effectives: permissions } = r);
    }
    const avant = [...(u.permissions ?? [])].sort().join(",");
    if (avant === [...permissions].sort().join(",") && role === u.role) return null;
    return { ...u, role, permissions };
  } catch {
    return null;
  }
}

/** Au moins une des permissions données. */
export function peutUne(...permissions: string[]): boolean {
  return permissions.some(peut);
}
