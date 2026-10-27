// components/AvisFinExercice.tsx — prévenir avant la fin des dates de
// travail (trouvé le 27/10/2026).
//
// Le lendemain de la fin, si personne n'a prolongé l'exercice ni ouvert
// le suivant, toute vente est refusée (D21) — et seul le patron
// (`dossiers:gerer`) peut y remédier. Le serveur dit, sur SON horloge,
// celle qui refusera, combien de jours restent
// (`lire_avis_fin_exercice`). Cet avis le répète au tableau de bord un
// mois avant, en rouge la dernière semaine. Au patron : le faire, avec
// le chemin. Aux autres : le prévenir.

import { useEffect, useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { jourEnLettres, lendemain } from "@/lib/utils";
import { CalendarClock } from "lucide-react";

interface AvisFin {
  aujourd_hui: string;
  /** Le dernier jour où l'on écrit encore ; `null` : c'est déjà fini. */
  fin: string | null;
  jours_restants: number | null;
  avis: "rien" | "proche" | "urgent" | "bloque";
  preavis_jours: number;
  /** Le refus que le serveur oppose aujourd'hui, mot pour mot. */
  refus: string | null;
}

export function AvisFinExercice({ actu, onExercices }: {
  /** Relit l'avis quand le tableau de bord se recharge. */
  actu?: unknown;
  /** Ouvre Paramètres → Dossiers. Absent sans `dossiers:gerer`. */
  onExercices?: () => void;
}) {
  const [avis, setAvis] = useState<AvisFin | null>(null);

  useEffect(() => {
    let annule = false;
    // Un serveur plus ancien ne connaît pas la commande : l'avis se tait
    // plutôt que de casser l'accueil.
    invoke<AvisFin>("lire_avis_fin_exercice")
      .then(a => { if (!annule) setAvis(a); })
      .catch(() => { if (!annule) setAvis(null); });
    return () => { annule = true; };
  }, [actu]);

  if (!avis || avis.avis === "rien") return null;

  const patron = peut("dossiers:gerer");
  const rouge = avis.avis !== "proche";
  const prevenir = "Prévenir le patron : lui seul peut les prolonger.";

  let texte: string;
  if (avis.avis === "bloque" || !avis.fin) {
    texte = patron
      ? (avis.refus ?? "Aujourd'hui ne tombe dans aucune date de travail ouverte.")
      : `Les ventes sont refusées aujourd'hui : les dates de travail du dossier sont finies. ${prevenir}`;
  } else {
    const j = avis.jours_restants ?? 0;
    const quand = j === 0 ? "aujourd'hui" : j === 1 ? "demain" : `dans ${j} jours`;
    texte = `Les dates de travail finissent le ${jourEnLettres(avis.fin)} (${quand}) : `
      + `dès le ${jourEnLettres(lendemain(avis.fin))}, plus aucune vente ne s'enregistre. `
      + (patron ? "Prolonger l'exercice ou ouvrir le suivant." : prevenir);
  }

  return (
    <div data-testid="avis-fin-exercice" data-avis={avis.avis}
      role={rouge ? "alert" : "status"}
      className={`flex items-center gap-3 px-3 py-2 text-sm border ${rouge
        ? "bg-red-600 border-red-700 text-white"
        : "bg-orange-50 border-orange-300 text-orange-900"}`}>
      <CalendarClock className="h-4 w-4 shrink-0" />
      <span className="flex-1">{texte}</span>
      {patron && onExercices && (
        <button onClick={onExercices} data-testid="avis-fin-exercice-ouvrir"
          className={`shrink-0 px-2.5 py-1 text-xs font-medium border transition-colors ${rouge
            ? "bg-white text-red-700 border-white hover:bg-red-50"
            : "bg-orange-600 text-white border-orange-700 hover:bg-orange-700"}`}>
          Dates de travail
        </button>
      )}
    </div>
  );
}
