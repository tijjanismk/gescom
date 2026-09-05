// components/ModalOuvrirCaisse.tsx
//
// Intercepte le refus `CAISSE_FERMEE` et propose d'ouvrir la caisse
// sur place.
//
// Depuis D46, toute opération d'argent est refusée si aucune session
// n'est ouverte. Le refus est juste — sans session, l'argent entre dans
// le tiroir sans `mouvement_caisse`, et la clôture affiche un excédent
// inexplicable. Mais renvoyer le vendeur vers un autre écran alors
// qu'un client attend au comptoir, c'est le meilleur moyen qu'il
// contourne l'application.
//
// Un champ, un bouton, et la vente reprend où elle en était.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { Wallet, Loader2 } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { UTILISATEUR_ACTIF } from "@/App";

/** Le refus serveur porte ce code en tête de message. */
export function estCaisseFermee(e: unknown): boolean {
  return String(e).includes("CAISSE_FERMEE");
}

interface Props {
  ouvert: boolean;
  onFermer: () => void;
  /** Appelé après ouverture réussie — relancer l'opération refusée. */
  onOuverte: () => void;
}

export function ModalOuvrirCaisse({ ouvert, onFermer, onOuverte }: Props) {
  const [fond, setFond] = useState("");
  const [enCours, setEnCours] = useState(false);

  async function ouvrir() {
    setEnCours(true);
    try {
      await invoke("ouvrir_session_caisse", {
        // Zéro est une valeur légitime : le tiroir peut être vide au
        // matin. On ne force donc pas une saisie non nulle.
        fondOuverture: parseMontant(fond) || 0,
        utilisateurRole: UTILISATEUR_ACTIF?.role ?? "employe",
      });
      setFond("");
      onOuverte();
    } catch (e) {
      await message(`${e}`, { title: "Ouverture impossible", kind: "error" });
    } finally {
      setEnCours(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={o => !o && onFermer()}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Wallet className="h-4 w-4" />
            La caisse n'est pas ouverte
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-3 pt-2">
          <p className="text-sm text-muted-foreground">
            Aucune opération d'argent ne peut être enregistrée tant que la
            caisse est fermée. Ouvrez-la ici, l'opération reprendra
            ensuite.
          </p>

          <div>
            <Label>Argent présent dans le tiroir *</Label>
            <MoneyInput value={fond} onChange={setFond}
              placeholder="0" className="mt-1" />
            {/* Compter réellement : c'est ce montant qui sert de
                référence à l'écart du soir. Le laisser à zéro par
                facilité fausse la clôture. */}
            <p className="text-xs text-muted-foreground mt-1">
              Comptez les billets et pièces. Ce montant sert de référence
              à la clôture du soir.
            </p>
          </div>

          <div className="flex gap-2 pt-1">
            <Button variant="outline" className="flex-1" onClick={onFermer}>
              Annuler
            </Button>
            <Button className="flex-1" onClick={ouvrir} disabled={enCours}>
              {enCours
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Ouvrir la caisse"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
