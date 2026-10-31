import { useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { Lock, Loader2, Eye, EyeOff } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { message } from "@tauri-apps/plugin-dialog";

interface ModalChangerMdpProps {
  ouvert: boolean;
  utilisateurId: string;
  obligatoire?: boolean; // true = première connexion, pas de bouton Annuler
  onFermer: () => void;
  onChange: () => void;
}

export function ModalChangerMdp({
  ouvert, utilisateurId, obligatoire = false, onFermer, onChange,
}: ModalChangerMdpProps) {
  const [ancienMdp, setAncienMdp] = useState("");
  const [nouveauMdp, setNouveauMdp] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [visible, setVisible] = useState(false);
  const [chargement, setChargement] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);

  const valide = nouveauMdp.length >= 6 && nouveauMdp === confirmation;

  async function handleChanger() {
    if (!valide || !ancienMdp) return;
    setChargement(true);
    setErreur(null);
    let change = false;
    try {
      await invoke("changer_mot_de_passe", {
        utilisateurId,
        ancienMdp,
        nouveauMdp,
      });
      change = true;
    } catch (err) {
      setErreur(typeof err === "string" ? err : err instanceof Error ? err.message : "Erreur lors du changement");
    } finally {
      setChargement(false);
    }
    if (!change) return;
    // Le changement est fait : la boîte se ferme D'ABORD. La confirmation
    // est une boîte native — dans un navigateur (poste multiposte), elle
    // n'existe pas et `message` échoue : la boîte restait alors ouverte,
    // le mot de passe déjà changé, l'écran bloqué (vu le 31/10).
    setAncienMdp(""); setNouveauMdp(""); setConfirmation("");
    onChange();
    message("Mot de passe changé", { title: "Succès", kind: "info" }).catch(() => {});
  }

  return (
    <Dialog open={ouvert} onOpenChange={obligatoire ? undefined : onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lock className="h-4 w-4" />
            {obligatoire ? "Changer le mot de passe par défaut" : "Changer le mot de passe"}
          </DialogTitle>
        </DialogHeader>

        {obligatoire && (
          <p className="text-xs text-muted-foreground bg-muted rounded-md px-3 py-2">
            Pour des raisons de sécurité, vous devez changer le mot de passe par défaut avant de continuer.
          </p>
        )}

        <div className="space-y-3 pt-2">
          <div>
            <Label>Mot de passe actuel</Label>
            <div className="relative mt-1">
              <Input
                type={visible ? "text" : "password"}
                value={ancienMdp}
                onChange={e => setAncienMdp(e.target.value)}
                placeholder="••••••••"
                autoFocus
              />
            </div>
          </div>

          <div>
            <Label>Nouveau mot de passe <span className="text-xs text-muted-foreground">(min. 6 caractères)</span></Label>
            <div className="relative mt-1">
              <Input
                type={visible ? "text" : "password"}
                value={nouveauMdp}
                onChange={e => setNouveauMdp(e.target.value)}
                placeholder="••••••••"
              />
              <button type="button" onClick={() => setVisible(!visible)}
                className="absolute right-3 top-2.5 text-muted-foreground">
                {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>

          <div>
            <Label>Confirmer le mot de passe</Label>
            <Input
              type={visible ? "text" : "password"}
              value={confirmation}
              onChange={e => setConfirmation(e.target.value)}
              placeholder="••••••••"
              className={`mt-1 ${confirmation && !valide ? "border-destructive" : ""}`}
              onKeyDown={e => e.key === "Enter" && handleChanger()}
            />
            {confirmation && nouveauMdp !== confirmation && (
              <p className="text-xs text-destructive mt-1">Les mots de passe ne correspondent pas</p>
            )}
          </div>

          {/* Le refus se lit ici, dans la boîte : il ne dépend pas d'une boîte native. */}
          {erreur && <p className="text-sm text-destructive" role="alert">{erreur}</p>}

          <div className="flex gap-2 pt-1">
            {!obligatoire && (
              <Button variant="outline" onClick={onFermer} className="flex-1">
                Annuler
              </Button>
            )}
            <Button
              onClick={handleChanger}
              disabled={!valide || !ancienMdp || chargement}
              className="flex-1"
            >
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Changer"
              }
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
