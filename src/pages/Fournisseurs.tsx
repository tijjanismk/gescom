import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Truck, TrendingDown, Loader2, Search, Plus } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { message } from "@tauri-apps/plugin-dialog";

interface FournisseurAvecDette {
  id: string;
  nom: string;
  telephone?: string;
  est_voisin: boolean;
  total_dettes: number;
  nb_achats: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Modal : Nouveau fournisseur
// =====================================================================

function ModalNouveauFournisseur({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: () => void;
}) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [nif, setNif] = useState("");
  const [adresse, setAdresse] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      await invoke("creer_fournisseur", {
        nom: nom.trim(),
        telephone: telephone.trim() || null,
        nif: nif.trim() || null,
        adresse: adresse.trim() || null,
      });
      setNom(""); setTelephone(""); setNif(""); setAdresse("");
      onCreer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouveau fournisseur</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du fournisseur" autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" />
          </div>
          <div>
            <Label>NIF <span className="text-xs text-muted-foreground">optionnel</span></Label>
            <Input value={nif} onChange={e => setNif(e.target.value)}
              placeholder="Numéro d'identification fiscale" />
          </div>
          <div>
            <Label>Adresse <span className="text-xs text-muted-foreground">optionnel</span></Label>
            <Input value={adresse} onChange={e => setAdresse(e.target.value)}
              placeholder="Adresse" />
          </div>
          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleCreer} disabled={!nom.trim() || chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Fournisseurs
// =====================================================================

export function Fournisseurs() {
  const [fournisseurs, setFournisseurs] = useState<FournisseurAvecDette[]>([]);
  const [chargement, setChargement] = useState(true);
  const [recherche, setRecherche] = useState("");
  const [modalNouveauFournisseur, setModalNouveauFournisseur] = useState(false);

  async function charger() {
    try {
      const data = await invoke<FournisseurAvecDette[]>("lire_fournisseurs_avec_dettes");
      setFournisseurs(data);
    } catch (e) {
      console.error("Erreur chargement fournisseurs :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  const filtres = fournisseurs.filter(f =>
    f.nom.toLowerCase().includes(recherche.toLowerCase()) ||
    (f.telephone && f.telephone.includes(recherche))
  ).filter(f => !f.est_voisin); // Les fournisseurs secondaires sont gérés séparément

  const avecDettes = filtres.filter(f => f.total_dettes > 0);
  const sansDettes = filtres.filter(f => f.total_dettes === 0);

  if (chargement) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Fournisseurs</h1>
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{filtres.length} fournisseurs</Badge>
          <Button size="sm" onClick={() => setModalNouveauFournisseur(true)}>
            <Plus className="h-4 w-4 mr-1" /> Nouveau
          </Button>
        </div>
      </div>

      <div className="relative mb-6 max-w-sm">
        <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input value={recherche} onChange={e => setRecherche(e.target.value)}
          placeholder="Rechercher..." className="pl-8" />
      </div>

      {/* Fournisseurs avec dettes */}
      {avecDettes.length > 0 && (
        <Card className="mb-6">
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2 text-red-500">
              <TrendingDown className="h-4 w-4" />
              Dettes en cours ({avecDettes.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-1">
              {avecDettes.map(f => (
                <div key={f.id}
                  className="flex items-center justify-between py-2.5 px-3 rounded-md hover:bg-muted/40 transition-colors">
                  <div>
                    <p className="text-sm font-medium">{f.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {f.telephone ?? "—"}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-semibold text-red-500">
                      {formaterMontant(f.total_dettes)}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {f.nb_achats} achat{f.nb_achats > 1 ? "s" : ""}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Fournisseurs à jour */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Truck className="h-4 w-4" />
            Tous les fournisseurs ({sansDettes.length} à jour)
          </CardTitle>
        </CardHeader>
        <CardContent>
          {sansDettes.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              Aucun fournisseur
            </p>
          ) : (
            <div className="space-y-1">
              {sansDettes.map(f => (
                <div key={f.id}
                  className="flex items-center justify-between py-2.5 px-3 rounded-md hover:bg-muted/40 transition-colors">
                  <div>
                    <p className="text-sm font-medium">{f.nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {f.telephone ?? "—"}
                    </p>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {f.nb_achats} achat{f.nb_achats > 1 ? "s" : ""}
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <ModalNouveauFournisseur
        ouvert={modalNouveauFournisseur}
        onFermer={() => setModalNouveauFournisseur(false)}
        onCreer={() => { setModalNouveauFournisseur(false); charger(); }}
      />
    </div>
  );
}
