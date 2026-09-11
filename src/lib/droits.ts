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

import { UTILISATEUR_ACTIF } from "@/App";

/** Cette personne peut-elle faire cela ? */
export function peut(permission: string): boolean {
  return UTILISATEUR_ACTIF?.permissions?.includes(permission) ?? false;
}

/** Au moins une des permissions données. */
export function peutUne(...permissions: string[]): boolean {
  return permissions.some(peut);
}
