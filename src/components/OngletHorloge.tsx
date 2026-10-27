// components/OngletHorloge.tsx — Paramètres → Horloge (lot C.5).
//
// Le serveur retient l'heure de sa dernière écriture. Si l'ordinateur
// qui garde la base repart en arrière (pile du BIOS, coupure) de plus de
// cinq minutes, plus rien ne s'enregistre : une vente tomberait au
// mauvais jour. Le remède est de corriger l'heure de Windows.
//
// Le seul geste d'ici : l'horloge avait été AVANCÉE par erreur (2030),
// puis corrigée — la dernière écriture est restée dans le futur et tout
// resterait bloqué jusqu'à cette date. Le patron confirme alors que
// l'heure actuelle est juste ; le serveur ramène la référence et le note
// à l'Historique. Le bouton n'existe que quand l'écriture est bloquée.

import { useCallback, useEffect, useState } from "react";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, CheckCircle2, Clock, Loader2 } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";

interface EtatHorloge {
  derniere_ecriture: string | null;
  derniere_ecriture_en_clair: string | null;
  maintenant: string;
  maintenant_en_clair: string;
  recul_secondes: number | null;
  tolerance_minutes: number;
  bloque: boolean;
  refus: string | null;
}

export function OngletHorloge() {
  const [etat, setEtat] = useState<EtatHorloge | null>(null);
  const [erreur, setErreur] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [fait, setFait] = useState("");

  const charger = useCallback(async () => {
    try {
      setEtat(await invoke<EtatHorloge>("lire_horloge"));
      setErreur("");
    } catch (e) {
      setErreur(String(e));
    }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  async function confirmer() {
    if (!etat) return;
    const oui = await ask(
      `L'ordinateur qui garde la base dit qu'il est ${etat.maintenant_en_clair}.\n\n`
      + "Confirmer seulement si CETTE date et CETTE heure sont justes. "
      + "Si elles sont fausses (pile, coupure), corriger d'abord la date et "
      + "l'heure de Windows sur cet ordinateur : confirmer une heure fausse "
      + "ferait enregistrer les ventes au mauvais jour.\n\n"
      + `La dernière écriture (${etat.derniere_ecriture_en_clair ?? "—"}) sera `
      + "ramenée à maintenant, et l'Historique le gardera.",
      { title: "L'heure de Windows est-elle juste ?", kind: "warning" },
    );
    if (!oui) return;
    setEnCours(true);
    try {
      const r = await invoke<{ nouvelle_en_clair: string }>("confirmer_horloge");
      setFait(`Heure confirmée : les écritures reprennent (référence ${r.nouvelle_en_clair}).`);
      await charger();
    } catch (e) {
      await message(String(e), { title: "Horloge", kind: "error" });
      await charger();
    } finally {
      setEnCours(false);
    }
  }

  if (!etat) {
    return erreur
      ? <p className="text-sm text-red-600">{erreur}</p>
      : (
        <div className="flex items-center gap-2 py-8 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Lecture de l'horloge…
        </div>
      );
  }

  return (
    <div className="max-w-2xl space-y-5">
      <p className="text-sm text-muted-foreground">
        L'ordinateur qui garde la base retient l'heure de sa dernière
        écriture. S'il repart en arrière de plus de {etat.tolerance_minutes} minutes
        (pile de l'ordinateur usée, coupure), plus rien ne s'enregistre — une
        vente tomberait au mauvais jour — jusqu'à ce que la date et l'heure de
        Windows soient corrigées. Les consultations continuent.
      </p>

      <div className="rounded-lg border border-border divide-y divide-border text-sm">
        <div className="flex items-center gap-3 px-4 py-2.5">
          <Clock className="h-4 w-4 text-muted-foreground" />
          <span className="text-muted-foreground">Heure de l'ordinateur qui garde la base</span>
          <span className="ml-auto font-medium">{etat.maintenant_en_clair}</span>
        </div>
        <div className="flex items-center gap-3 px-4 py-2.5">
          <Clock className="h-4 w-4 text-muted-foreground" />
          <span className="text-muted-foreground">Dernière écriture</span>
          <span className="ml-auto font-medium">{etat.derniere_ecriture_en_clair ?? "aucune encore"}</span>
        </div>
      </div>

      {etat.bloque ? (
        <div className="rounded-lg border border-red-300 bg-red-50 p-4 space-y-3">
          <p className="flex items-start gap-2 text-sm text-red-800">
            <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
            <span>
              Les écritures sont bloquées : l'heure de cet ordinateur est en
              arrière sur la dernière écriture. D'abord, vérifier la date et
              l'heure de Windows et les corriger si elles sont fausses.
            </span>
          </p>
          <p className="text-sm text-red-800">
            Si elles sont justes — l'horloge avait été avancée par erreur, puis
            remise à l'heure —, confirmer ici.
          </p>
          <Button variant="destructive" size="sm" onClick={confirmer} disabled={enCours}>
            {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "L'heure de Windows est juste"}
          </Button>
        </div>
      ) : (
        <p className="flex items-center gap-2 text-sm text-green-700">
          <CheckCircle2 className="h-4 w-4" />
          L'heure est cohérente : les écritures passent.
        </p>
      )}

      {fait && <p className="text-sm text-green-700">{fait}</p>}

      <Button variant="outline" size="sm" onClick={() => { setFait(""); charger(); }}>
        Relire l'heure
      </Button>
    </div>
  );
}
