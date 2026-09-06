// components/ModalModifierTiers.tsx — Édition d'un client ou fournisseur
//
// Un seul modal pour les deux : les champs sont les mêmes (nom,
// téléphone, adresse, NIF, e-mail). Les différences tiennent en deux
// points, portés par `cote` :
//   - le code client est affiché mais NON modifiable ; il est imprimé
//     sur les pièces déjà remises et sert de référence au comptoir ;
//   - `est_voisin` n'existe que côté fournisseur.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { Loader2, Pencil } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export interface TiersModifiable {
  id: string;
  nom: string;
  code?: string;
  telephone?: string | null;
  adresse?: string | null;
  email?: string | null;
  nif?: string | null;
  est_voisin?: boolean;
}

interface Props {
  tiers: TiersModifiable | null;
  cote: "client" | "fournisseur";
  onFermer: () => void;
  onModifie: () => void;
}

export function ModalModifierTiers({ tiers, cote, onFermer, onModifie }: Props) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [adresse, setAdresse] = useState("");
  const [email, setEmail] = useState("");
  const [nif, setNif] = useState("");
  const [estVoisin, setEstVoisin] = useState(false);
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (!tiers) return;
    setNom(tiers.nom ?? "");
    setTelephone(tiers.telephone ?? "");
    setAdresse(tiers.adresse ?? "");
    setEmail(tiers.email ?? "");
    setNif(tiers.nif ?? "");
    setEstVoisin(!!tiers.est_voisin);
  }, [tiers]);

  async function handleEnregistrer() {
    if (!tiers || !nom.trim()) return;
    setChargement(true);
    try {
      if (cote === "client") {
        await invoke("modifier_client", {
          clientId: tiers.id,
          nom: nom.trim(),
          telephone: telephone.trim() || null,
          adresse: adresse.trim() || null,
          email: email.trim() || null,
          nif: nif.trim() || null,
        });
      } else {
        await invoke("modifier_fournisseur", {
          fournisseurId: tiers.id,
          nom: nom.trim(),
          telephone: telephone.trim() || null,
          adresse: adresse.trim() || null,
          nif: nif.trim() || null,
          email: email.trim() || null,
          estVoisin: estVoisin,
        });
      }
      onModifie();
      onFermer();
    } catch (e) {
      await message(`${e}`, { title: "Modification impossible", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={tiers !== null} onOpenChange={o => { if (!o) onFermer(); }}>
      <DialogContent style={{ width: "440px", maxWidth: "94vw" }}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Pencil className="h-4 w-4" />
            Modifier {cote === "client" ? "le client" : "le fournisseur"}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-3 pt-1">
          {/* Le code est une référence figée : il est déjà imprimé sur
              des pièces remises. Affiché pour repérage, jamais éditable. */}
          {cote === "client" && tiers?.code && (
            <div className="bg-muted rounded-md px-3 py-2 text-sm flex
                            items-center justify-between">
              <span className="text-muted-foreground">Code</span>
              <span className="font-mono">{tiers.code}</span>
            </div>
          )}

          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              className="mt-1" autoFocus
              onKeyDown={e => e.key === "Enter" && handleEnregistrer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" className="mt-1" />
          </div>
          <div>
            <Label>Adresse</Label>
            <Input value={adresse} onChange={e => setAdresse(e.target.value)}
              className="mt-1" />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label>NIF</Label>
              <Input value={nif} onChange={e => setNif(e.target.value)}
                className="mt-1" />
            </div>
            <div>
              <Label>E-mail</Label>
              <Input value={email} onChange={e => setEmail(e.target.value)}
                className="mt-1" />
            </div>
          </div>

          {cote === "fournisseur" && (
            <label className="flex items-center gap-2.5 px-3 py-2 rounded-lg
                              border border-border cursor-pointer
                              hover:bg-muted/40 transition-colors">
              <input type="checkbox" checked={estVoisin}
                onChange={e => setEstVoisin(e.target.checked)}
                className="w-4 h-4 rounded accent-primary" />
              <div>
                <p className="text-sm font-medium">Fournisseur de dépannage</p>
                <p className="text-xs text-muted-foreground">
                  Voisin ou confrère chez qui on se dépanne ponctuellement
                </p>
              </div>
            </label>
          )}

          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Annuler
            </Button>
            <Button onClick={handleEnregistrer}
              disabled={!nom.trim() || chargement} className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Enregistrer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
