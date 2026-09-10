// components/ModalLivraison.tsx — Suivi de livraison d'une pièce
//
// Purement informatif : rien ici ne touche au stock ni à la caisse. Le
// stock sort toujours à la validation de la facture. On ne répond qu'à
// « qu'est-ce qui est déjà parti ? », question que le paiement ne pose
// pas — d'où un axe séparé, qui rend représentable le « payé non livré ».
//
// La quantité saisie est le CUMUL livré à ce jour, pas l'incrément :
// l'écran envoie ce qu'il affiche, donc deux enregistrements rapprochés
// ne peuvent pas doubler la quantité.

import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import { Truck, Loader2, CheckCheck } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface LigneLivraison {
  id: string; article_nom: string; unite: string;
  quantite: number; quantite_livree: number; reste: number;
}

interface DonneesLivraison {
  piece_id: string; numero: string; type_piece: string;
  lignes: LigneLivraison[]; etat: string;
}

interface ModalLivraisonProps {
  pieceId: string | null;
  numero?: string;
  /**
   * Le même mécanisme sert des deux côtés, mais pas le même mot : chez
   * le client la marchandise PART, chez le fournisseur elle ARRIVE.
   * Afficher « livré » sur un bon de commande fournisseur ferait lire
   * l'inverse de ce qui s'est passé.
   */
  cote?: "client" | "fournisseur";
  onFermer: () => void;
  onEnregistre?: () => void;
}

const MOTS = {
  client: {
    titre: "Livraison", action: "Tout livrer", colonne: "Livré",
    rien: "Rien de livré", partiel: "Partiellement livré",
    tout: "Entièrement livré", reference: "Commandé",
  },
  fournisseur: {
    titre: "Réception", action: "Tout recevoir", colonne: "Reçu",
    rien: "Rien de reçu", partiel: "Partiellement reçu",
    tout: "Entièrement reçu", reference: "Attendu",
  },
} as const;

function fmtQte(n: number): string {
  return n % 1 === 0 ? String(n) : n.toFixed(2);
}

export function ModalLivraison({
  pieceId, numero, cote = "client", onFermer, onEnregistre,
}: ModalLivraisonProps) {
  const mots = MOTS[cote];
  const [donnees, setDonnees] = useState<DonneesLivraison | null>(null);
  // Saisie libre en texte : un `number` contrôlé empêche d'effacer le
  // champ pour retaper, ce qui rend la saisie pénible au comptoir.
  const [saisie, setSaisie] = useState<Record<string, string>>({});
  const [chargement, setChargement] = useState(false);
  const [enregistrement, setEnregistrement] = useState(false);

  const ouvert = pieceId !== null;

  useEffect(() => {
    if (!ouvert) return;
    let annule = false;

    setChargement(true);
    invoke<DonneesLivraison>("lire_livraison_piece", { pieceId })
      .then(d => {
        if (annule) return;
        setDonnees(d);
        setSaisie(Object.fromEntries(
          d.lignes.map(l => [l.id, String(l.quantite_livree)]),
        ));
      })
      .catch(e => {
        if (!annule) message(`Erreur : ${e}`, { title: "Livraison", kind: "error" });
      })
      .finally(() => { if (!annule) setChargement(false); });

    return () => { annule = true; };
  }, [pieceId, ouvert]);

  function toutLivrer() {
    if (!donnees) return;
    setSaisie(Object.fromEntries(
      donnees.lignes.map(l => [l.id, String(l.quantite)]),
    ));
  }

  async function handleEnregistrer() {
    if (!donnees) return;
    setEnregistrement(true);
    try {
      await invoke("enregistrer_livraison", {
        pieceId,
        lignes: donnees.lignes.map(l => ({
          ligneId: l.id,
          quantiteLivree: parseFloat(saisie[l.id] ?? "0") || 0,
        })),
      });
      onEnregistre?.();
      onFermer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Livraison", kind: "error" });
    } finally {
      setEnregistrement(false);
    }
  }

  // Etat prévisionnel, calculé sur la saisie en cours : le badge doit
  // refléter ce qu'on s'apprête à enregistrer, pas l'état en base.
  const commandee = donnees?.lignes.reduce((s, l) => s + l.quantite, 0) ?? 0;
  const livree = donnees?.lignes.reduce(
    (s, l) => s + (parseFloat(saisie[l.id] ?? "0") || 0), 0) ?? 0;
  const etatPrev = commandee <= 0 ? "sans_objet"
    : livree <= 0 ? "non_livre"
    : livree >= commandee - 0.0001 ? "livre"
    : "partiel";

  const ETIQUETTE: Record<string, string> = {
    sans_objet: "—", non_livre: mots.rien,
    partiel: mots.partiel, livre: mots.tout,
  };
  const COULEUR: Record<string, string> = {
    sans_objet: "bg-muted text-muted-foreground",
    non_livre: "bg-muted text-muted-foreground",
    partiel: "bg-orange-100 text-orange-700",
    livre: "bg-green-100 text-green-700",
  };

  return (
    <Dialog open={ouvert} onOpenChange={o => { if (!o) onFermer(); }}>
      {/* D22 : shadcn ignore max-w-* sur DialogContent, d'où le style
          inline. */}
      <DialogContent style={{ width: "620px", maxWidth: "94vw" }}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Truck className="h-4 w-4" />
            {mots.titre} — {numero ?? donnees?.numero ?? ""}
          </DialogTitle>
        </DialogHeader>

        {chargement ? (
          <div className="flex items-center justify-center py-10">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : !donnees || donnees.lignes.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-8">
            Cette pièce n'a aucune ligne.
          </p>
        ) : (
          <div className="space-y-3 pt-1">
            <div className="flex items-center justify-between">
              <span className={`text-xs font-medium px-2 py-1 rounded ${
                COULEUR[etatPrev]}`}>
                {ETIQUETTE[etatPrev]}
              </span>
              <Button variant="outline" size="sm" onClick={toutLivrer}
                className="h-7 text-xs">
                <CheckCheck className="h-3 w-3 mr-1" /> {mots.action}
              </Button>
            </div>

            <div className="border border-border rounded-lg overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-muted/50 border-b border-border">
                  <tr>
                    <th className="text-left px-3 py-2 font-medium">Article</th>
                    <th className="text-right px-3 py-2 font-medium">{mots.reference}</th>
                    <th className="text-right px-3 py-2 font-medium w-28">{mots.colonne}</th>
                    <th className="text-right px-3 py-2 font-medium">Reste</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {donnees.lignes.map(l => {
                    const v = parseFloat(saisie[l.id] ?? "0") || 0;
                    const reste = Math.max(0, l.quantite - v);
                    return (
                      <tr key={l.id}>
                        <td className="px-3 py-2">
                          <p className="font-medium">{l.article_nom}</p>
                          <p className="text-xs text-muted-foreground">{l.unite}</p>
                        </td>
                        <td className="px-3 py-2 text-right">
                          {fmtQte(l.quantite)}
                        </td>
                        <td className="px-3 py-2">
                          <Input type="number" min={0} max={l.quantite}
                            value={saisie[l.id] ?? ""}
                            onChange={e => setSaisie(s => ({
                              ...s, [l.id]: e.target.value,
                            }))}
                            className="h-8 text-sm text-right" />
                        </td>
                        <td className={`px-3 py-2 text-right font-medium ${
                          reste > 0 ? "text-orange-600" : "text-green-700"}`}>
                          {fmtQte(reste)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            <p className="text-xs text-muted-foreground">
              Information de suivi seulement : le stock et la caisse ne
              bougent pas. {cote === "client"
                ? "La marchandise sort à la validation de la facture."
                : "La marchandise entre à l'enregistrement de l'achat."}
            </p>

            <div className="flex gap-2">
              <Button variant="outline" onClick={onFermer} className="flex-1">
                Annuler
              </Button>
              <Button onClick={handleEnregistrer} disabled={enregistrement}
                className="flex-1">
                {enregistrement
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : "Enregistrer"}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
