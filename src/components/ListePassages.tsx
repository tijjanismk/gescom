// components/ListePassages.tsx — Clients → « Passages » (D35).
//
// Chaque vente sans nom a sa fiche « Client de passage » (code PAS-…),
// hors de la liste des clients et des chiffres. On la retrouve ici par
// son code ou par le numéro de la facture que le passant rapporte, et
// on en fait un client ordinaire quand il donne son nom : il garde ses
// achats, ses factures déjà émises gardent le nom sous lequel elles
// l'ont été.

import { useCallback, useEffect, useState } from "react";
import { Loader2, UserCheck } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";

interface Passage {
  id: string;
  code: string;
  nom: string;
  cree_le: string;
  nb_ventes: number;
  paye: number;
  numero: string | null;
}

const fmt = (n: number) => `${n.toLocaleString("fr-FR")} F`;
const jour = (iso: string) => {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso.slice(0, 10) : d.toLocaleDateString("fr-FR");
};

export function ListePassages({ recherche }: { recherche: string }) {
  const [passages, setPassages] = useState<Passage[]>([]);
  const [chargement, setChargement] = useState(true);
  const [erreur, setErreur] = useState("");
  const [choisi, setChoisi] = useState<Passage | null>(null);
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [erreurPromotion, setErreurPromotion] = useState("");
  const [fait, setFait] = useState("");
  const peutPromouvoir = peut("clients:modifier");

  const charger = useCallback(async () => {
    setChargement(true);
    setErreur("");
    try {
      setPassages(await invoke<Passage[]>("lire_passages", { recherche: recherche.trim() || null }));
    } catch (e) {
      setErreur(String(e));
    } finally {
      setChargement(false);
    }
  }, [recherche]);

  // Temporisée : la recherche part quand on s'arrête de taper.
  useEffect(() => {
    const t = setTimeout(charger, 200);
    return () => clearTimeout(t);
  }, [charger]);

  function ouvrir(p: Passage) {
    setChoisi(p);
    setNom("");
    setTelephone("");
    setErreurPromotion("");
  }

  async function promouvoir() {
    if (!choisi) return;
    setEnCours(true);
    setErreurPromotion("");
    try {
      await invoke("promouvoir_passage", {
        clientId: choisi.id,
        nom: nom.trim(),
        telephone: telephone.trim() || null,
      });
      setFait(`${nom.trim()} (${choisi.code}) est maintenant un client ordinaire.`);
      setChoisi(null);
      await charger();
    } catch (e) {
      setErreurPromotion(String(e));
    } finally {
      setEnCours(false);
    }
  }

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">
          Clients de passage — une fiche par vente sans nom
        </CardTitle>
        <p className="text-xs text-muted-foreground">
          Chercher par le code (PAS-…) ou le numéro de la facture du
          ticket. Payés en entier à la vente : ni crédit, ni avoir.
        </p>
      </CardHeader>
      <CardContent className="p-0">
        {fait && (
          <p role="status" className="mx-4 mb-3 rounded-md bg-green-600/10 p-2 text-xs text-green-700">
            {fait}
          </p>
        )}
        {erreur && (
          <p className="mx-4 mb-3 rounded-md bg-red-500/10 p-2 text-xs text-red-600">{erreur}</p>
        )}
        {chargement ? (
          <div className="flex justify-center py-8">
            <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        ) : passages.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-8">Aucun client de passage</p>
        ) : (
          <div className="divide-y divide-border">
            {passages.map(p => (
              <div key={p.id} data-testid="passage"
                className="flex items-center justify-between py-2.5 px-4 hover:bg-muted/40">
                <div>
                  <p className="text-sm font-medium">{p.code}</p>
                  <p className="text-xs text-muted-foreground">
                    {jour(p.cree_le)}
                    {p.numero ? ` · ${p.numero}` : ""}
                    {` · ${p.nb_ventes} vente${p.nb_ventes > 1 ? "s" : ""} · ${fmt(p.paye)}`}
                  </p>
                </div>
                {peutPromouvoir && (
                  <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => ouvrir(p)}>
                    <UserCheck className="h-3 w-3" /> Passer en client ordinaire
                  </Button>
                )}
              </div>
            ))}
          </div>
        )}
      </CardContent>

      <Dialog open={!!choisi} onOpenChange={o => { if (!o) setChoisi(null); }}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{choisi?.code} : passer en client ordinaire</DialogTitle>
          </DialogHeader>
          <div className="space-y-3 pt-1">
            <p className="text-xs text-muted-foreground">
              Il garde son code et ses achats. Ses factures déjà émises
              gardent « Client de passage ». Le crédit lui devient possible,
              selon les droits.
            </p>
            <div>
              <Label>Nom *</Label>
              <Input value={nom} onChange={e => setNom(e.target.value)} placeholder="Nom du client"
                autoFocus className="mt-1" />
            </div>
            <div>
              <Label>Téléphone</Label>
              <Input value={telephone} onChange={e => setTelephone(e.target.value)} placeholder="76 00 00 00"
                className="mt-1" />
            </div>
            {erreurPromotion && (
              <p className="rounded-md bg-red-500/10 p-2 text-xs text-red-600">{erreurPromotion}</p>
            )}
            <div className="flex gap-2 pt-1">
              <Button variant="outline" className="flex-1" onClick={() => setChoisi(null)}>Annuler</Button>
              <Button className="flex-1" disabled={enCours || !nom.trim()} onClick={promouvoir}>
                {enCours ? <Loader2 className="h-4 w-4 animate-spin" /> : "Enregistrer"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
