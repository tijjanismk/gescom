// lib/homonyme.ts
//
// D44 — un fournisseur du même nom qu'un autre : le serveur refuse avec
// le code FOURNISSEUR_HOMONYME en tête de message. Le même, ou un autre
// « Moussa Traoré » ? Seul le commerçant le sait : on lui demande, et on
// rejoue avec `forcer` s'il confirme. Un NIF déjà connu, lui, est refusé
// sans question (même entreprise).

import { ask } from "@tauri-apps/plugin-dialog";

const CODE = "FOURNISSEUR_HOMONYME";

/**
 * Appelle `appel(false)` ; sur un refus d'homonyme, demande, puis
 * rappelle `appel(true)`. Rend `null` si le commerçant renonce.
 */
export async function avecHomonymeConfirme<T>(appel: (forcer: boolean) => Promise<T>): Promise<T | null> {
  try {
    return await appel(false);
  } catch (e) {
    const texte = String(e);
    if (!texte.includes(CODE)) throw e;
    const question = texte.replace(new RegExp(`^.*?${CODE}\\s*:\\s*`), "");
    const oui = await ask(`${question}\n\nCréer quand même un second fournisseur de ce nom ?`, {
      title: "Fournisseur du même nom",
      kind: "warning",
      okLabel: "Oui, un autre fournisseur",
      cancelLabel: "Non, c'est le même",
    });
    return oui ? appel(true) : null;
  }
}
