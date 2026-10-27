// components/ListePassages.tsx — Clients → « Passages » (D35).
//
// Chaque vente sans nom a sa fiche « Client de passage » (code PAS-…),
// hors de la liste des clients et des chiffres. On la retrouve ici par
// son code ou par le numéro de la facture que le passant rapporte, et
// on en fait un client ordinaire quand il donne son nom : il garde ses
// achats, ses factures déjà émises gardent le nom sous lequel elles
// l'ont été.
//
// Lot D.10 : « Rattacher à un client » — le passant revenu est un client
// qu'on connaît déjà. Le patron choisit sa fiche ; les ventes, pièces,
// paiements, retours et avoirs du passage y passent, ses factures déjà
// émises gardent leur PAS-…, et la fiche de passage, vide, est
// désactivée. Jamais automatique : un passant n'a ni nom ni téléphone.

import { useCallback, useEffect, useState } from "react";
import { Link2, Loader2, UserCheck } from "lucide-react";
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

/** Un client ordinaire proposé comme cible (lire_clients_pagines). */
interface ClientCible {
  id: string;
  code: string;
  nom: string;
  telephone: string | null;
}

interface Rattachement {
  client_nom: string;
  client_code: string;
  deplaces: { ventes: number; pieces: number; paiements: number; retours: number; avoirs: number };
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

  // Lot D.10 : la fenêtre « Rattacher à un client ».
  const [aRattacher, setARattacher] = useState<Passage | null>(null);
  const [rechercheCible, setRechercheCible] = useState("");
  const [cibles, setCibles] = useState<ClientCible[]>([]);
  const [cible, setCible] = useState<ClientCible | null>(null);
  const [chercheCibles, setChercheCibles] = useState(false);
  const [erreurRattachement, setErreurRattachement] = useState("");

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

  function ouvrirRattachement(p: Passage) {
    setARattacher(p);
    setRechercheCible("");
    setCibles([]);
    setCible(null);
    setErreurRattachement("");
  }

  // Les clients ordinaires du dossier (ni « Comptant », ni passages),
  // cherchés par nom ou téléphone quand on s'arrête de taper.
  useEffect(() => {
    if (!aRattacher) return;
    const r = rechercheCible.trim();
    if (!r) { setCibles([]); return; }
    let annule = false;
    const t = setTimeout(async () => {
      setChercheCibles(true);
      try {
        const page = await invoke<{ donnees: ClientCible[] }>("lire_clients_pagines", {
          page: 0, limite: 8, recherche: r,
          avecCreancesSeulement: false, ventesFiltre: null, tri: "nom",
        });
        if (!annule) setCibles(page.donnees);
      } catch (e) {
        if (!annule) setErreurRattachement(String(e));
      } finally {
        if (!annule) setChercheCibles(false);
      }
    }, 250);
    return () => { annule = true; clearTimeout(t); };
  }, [rechercheCible, aRattacher]);

  async function rattacher() {
    if (!aRattacher || !cible) return;
    setEnCours(true);
    setErreurRattachement("");
    try {
      const r = await invoke<Rattachement>("rattacher_passage", {
        passageId: aRattacher.id,
        clientId: cible.id,
      });
      const d = r.deplaces;
      setFait(
        `${aRattacher.code} est rattaché à ${r.client_nom} (${r.client_code}) : ` +
        `${d.ventes} vente${d.ventes > 1 ? "s" : ""}, ${d.pieces} pièce${d.pieces > 1 ? "s" : ""}, ` +
        `${d.paiements} paiement${d.paiements > 1 ? "s" : ""} passés sur sa fiche.`
      );
      setARattacher(null);
      await charger();
    } catch (e) {
      setErreurRattachement(String(e));
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
                  <div className="flex items-center gap-1.5">
                    <Button size="sm" variant="outline" className="h-7 text-xs gap-1" onClick={() => ouvrir(p)}>
                      <UserCheck className="h-3 w-3" /> Passer en client ordinaire
                    </Button>
                    <Button size="sm" variant="outline" className="h-7 text-xs gap-1"
                      onClick={() => ouvrirRattachement(p)}>
                      <Link2 className="h-3 w-3" /> Rattacher à un client
                    </Button>
                  </div>
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

      <Dialog open={!!aRattacher} onOpenChange={o => { if (!o) setARattacher(null); }}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>{aRattacher?.code} : rattacher à un client</DialogTitle>
          </DialogHeader>
          <div className="space-y-3 pt-1">
            <p className="text-xs text-muted-foreground">
              Le passant est un client que vous connaissez déjà. Ses
              {" "}{aRattacher?.nb_ventes ?? 0} vente{(aRattacher?.nb_ventes ?? 0) > 1 ? "s" : ""}
              {aRattacher ? ` (${fmt(aRattacher.paye)})` : ""}, avec leurs paiements,
              retours et pièces, passent sur la fiche choisie. Ses factures
              déjà émises gardent {aRattacher?.code}. La fiche de passage est
              ensuite désactivée. Ce geste ne se défait pas.
            </p>
            <div>
              <Label>Client</Label>
              <Input value={rechercheCible} onChange={e => { setRechercheCible(e.target.value); setCible(null); }}
                placeholder="Nom ou téléphone" autoFocus className="mt-1" />
            </div>
            <div className="max-h-48 overflow-auto rounded-md border border-border divide-y divide-border">
              {chercheCibles ? (
                <div className="flex justify-center py-3">
                  <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                </div>
              ) : cibles.length === 0 ? (
                <p className="text-xs text-muted-foreground text-center py-3">
                  {rechercheCible.trim() ? "Aucun client trouvé" : "Taper un nom ou un téléphone"}
                </p>
              ) : (
                cibles.map(c => (
                  <button key={c.id} type="button" data-testid="cible-rattachement"
                    onClick={() => setCible(c)}
                    className={`w-full text-left px-3 py-2 text-sm hover:bg-muted/40 ${
                      cible?.id === c.id ? "bg-primary/10 font-medium" : ""}`}>
                    {c.nom}
                    <span className="text-xs text-muted-foreground">
                      {` · ${c.code}`}{c.telephone ? ` · ${c.telephone}` : ""}
                    </span>
                  </button>
                ))
              )}
            </div>
            {erreurRattachement && (
              <p className="rounded-md bg-red-500/10 p-2 text-xs text-red-600">{erreurRattachement}</p>
            )}
            <div className="flex gap-2 pt-1">
              <Button variant="outline" className="flex-1" onClick={() => setARattacher(null)}>Annuler</Button>
              <Button className="flex-1" disabled={enCours || !cible} onClick={rattacher}>
                {enCours
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : cible ? `Rattacher à ${cible.nom}` : "Rattacher"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
