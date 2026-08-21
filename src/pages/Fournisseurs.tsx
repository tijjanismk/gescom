import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Truck, Plus, Search, X, Loader2,
  FileText, Banknote, ChevronDown, ChevronRight,
  CheckCircle2,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { Pagination } from "@/components/Pagination";

const LIMITE = 30;

// =====================================================================
//  Types
// =====================================================================

interface Fournisseur {
  id: string; nom: string; telephone?: string;
  adresse?: string; est_voisin: boolean;
  total_achats: number; total_paye: number; dette: number;
}

interface PageResult {
  donnees: Fournisseur[]; total: number; pages: number; page: number;
}

// =====================================================================
//  Props
// =====================================================================

interface FournisseursProps {
  onOuvrirFiche?: (fournisseurId: string) => void;
}

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Modal : Nouveau fournisseur
// =====================================================================

function ModalNouveauFournisseur({
  ouvert, onFermer, onCree,
}: { ouvert: boolean; onFermer: () => void; onCree: () => void }) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [adresse, setAdresse] = useState("");
  const [estVoisin, setEstVoisin] = useState(false);
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      await invoke("creer_fournisseur", {
        nom: nom.trim(),
        telephone: telephone.trim() || null,
        adresse: adresse.trim() || null,
        estVoisin,
      });
      setNom(""); setTelephone(""); setAdresse(""); setEstVoisin(false);
      onCree();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouveau fournisseur</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du fournisseur" autoFocus className="mt-1"
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" className="mt-1" />
          </div>
          <div>
            <Label>Adresse</Label>
            <Input value={adresse} onChange={e => setAdresse(e.target.value)}
              placeholder="Quartier, rue..." className="mt-1" />
          </div>
          <label className="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" checked={estVoisin}
              onChange={e => setEstVoisin(e.target.checked)}
              className="w-4 h-4 rounded accent-primary" />
            <span className="text-sm">Fournisseur voisin / informel</span>
          </label>
          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleCreer}
              disabled={!nom.trim() || chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Règlement dette rapide
// =====================================================================

function ModalReglementRapide({
  ouvert, fournisseur, onFermer, onRegle,
}: {
  ouvert: boolean; fournisseur: Fournisseur | null;
  onFermer: () => void; onRegle: () => void;
}) {
  const [montant, setMontant] = useState("");
  const [mode, setMode] = useState("especes");
  const [note, setNote] = useState("");
  const [chargement, setChargement] = useState(false);
  const [ok, setOk] = useState(false);

  useEffect(() => {
    if (ouvert && fournisseur) {
      setMontant(fournisseur.dette.toString());
      setMode("especes"); setNote(""); setOk(false);
    }
  }, [ouvert, fournisseur]);

  async function handleRegler() {
    if (!fournisseur) return;
    setChargement(true);
    try {
      await invoke("regler_dette_fournisseur", {
        fournisseurId: fournisseur.id,
        montant: parseMontant(montant),
        mode, note: note || null,
      });
      setOk(true);
      setTimeout(() => { onRegle(); }, 1200);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  if (!fournisseur) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Banknote className="h-4 w-4" /> Régler dette
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          {ok ? (
            <div className="flex items-center gap-3 p-4 rounded-lg
                            bg-green-50 border border-green-200">
              <CheckCircle2 className="h-6 w-6 text-green-600 shrink-0" />
              <p className="text-sm font-medium text-green-800">Paiement enregistré ✓</p>
            </div>
          ) : (
            <>
              <div className="bg-muted rounded-md px-3 py-2.5 space-y-1">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{fournisseur.nom}</span>
                  <span className="font-bold text-orange-600">
                    {fmt(fournisseur.dette)}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground">Dette actuelle</p>
              </div>
              <div>
                <Label className="text-xs mb-1.5 block">Montant (F)</Label>
                <MoneyInput value={montant} onChange={setMontant}
                  className="h-9" autoFocus />
              </div>
              <div>
                <Label className="text-xs mb-1.5 block">Mode</Label>
                <Select value={mode} onValueChange={v => { if (v) setMode(v); }}>
                  <SelectTrigger className="h-9 text-sm"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="especes">Espèces</SelectItem>
                    <SelectItem value="orange_money">Orange Money</SelectItem>
                    <SelectItem value="moov_money">Moov Money</SelectItem>
                    <SelectItem value="cheque">Chèque</SelectItem>
                    <SelectItem value="virement">Virement</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <Label className="text-xs mb-1.5 block">Note (optionnel)</Label>
                <Input value={note} onChange={e => setNote(e.target.value)}
                  placeholder="Ex: facture n°..." />
              </div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
                <Button onClick={handleRegler}
                  disabled={!montant || chargement} className="flex-1">
                  {chargement
                    ? <Loader2 className="h-4 w-4 animate-spin" />
                    : "Confirmer"}
                </Button>
              </div>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Fournisseurs
// =====================================================================

export function Fournisseurs({ onOuvrirFiche }: FournisseursProps) {
  const [fournisseurs, setFournisseurs] = useState<Fournisseur[]>([]);
  const [chargement, setChargement] = useState(true);
  const [page, setPage] = useState(0);
  const [total, setTotal] = useState(0);
  const [recherche, setRecherche] = useState("");
  const [avecDettesSeulement, setAvecDettesSeulement] = useState(false);
  const [modalNouv, setModalNouv] = useState(false);
  const [modalRegler, setModalRegler] = useState(false);
  const [fournisseurActif, setFournisseurActif] = useState<Fournisseur | null>(null);
  const [expandeId, setExpandeId] = useState<string | null>(null);

  const charger = useCallback(async (p = 0) => {
    setChargement(true);
    try {
      const data = await invoke<PageResult>("lire_fournisseurs_pagines", {
        page: p, limite: LIMITE,
        recherche: recherche || null,
        avecDettesSeulement: avecDettesSeulement || null,
      });
      setFournisseurs(data.donnees);
      setTotal(data.total);
    } catch (e) {
      // Fallback : lire_fournisseurs_avec_dettes
      try {
        const data = await invoke<Fournisseur[]>("lire_fournisseurs_avec_dettes");
        const filtre = recherche
          ? data.filter(f => f.nom.toLowerCase().includes(recherche.toLowerCase()))
          : data;
        const avecDettes = avecDettesSeulement
          ? filtre.filter(f => f.dette > 0)
          : filtre;
        setFournisseurs(avecDettes);
        setTotal(avecDettes.length);
      } catch (e2) {
        console.error("Erreur fournisseurs :", e2);
      }
    } finally { setChargement(false); }
  }, [recherche, avecDettesSeulement]);

  useEffect(() => { setPage(0); charger(0); }, [recherche, avecDettesSeulement]);

  const totalDettes = fournisseurs.reduce((s, f) => s + (f.dette ?? 0), 0);

  return (
    <div className="flex-1 overflow-auto p-6">

      {/* En-tête */}
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Fournisseurs</h1>
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{total} fournisseurs</Badge>
          <Button size="sm" onClick={() => setModalNouv(true)}>
            <Plus className="h-4 w-4 mr-1" /> Nouveau
          </Button>
        </div>
      </div>

      {/* Résumé dettes */}
      {totalDettes > 0 && (
        <div className="flex items-center justify-between px-4 py-3 mb-4
                        bg-orange-50 border border-orange-200 rounded-lg">
          <div className="flex items-center gap-2">
            <Banknote className="h-4 w-4 text-orange-600" />
            <span className="text-sm font-medium text-orange-800">
              Total dettes fournisseurs
            </span>
          </div>
          <span className="text-lg font-bold text-orange-600">{fmt(totalDettes)}</span>
        </div>
      )}

      {/* Filtres */}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
            placeholder="Nom ou téléphone..."
            className="h-8 text-sm w-48 pl-8" />
          {recherche && (
            <button onClick={() => setRecherche("")}
              className="absolute right-2 top-2 text-muted-foreground">
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
        <button
          onClick={() => setAvecDettesSeulement(!avecDettesSeulement)}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md border
                      text-xs font-medium transition-colors ${
            avecDettesSeulement
              ? "border-orange-400 bg-orange-50 text-orange-700"
              : "border-border text-muted-foreground hover:bg-muted"
          }`}>
          <Banknote className="h-3 w-3" /> Avec dettes
        </button>
        {(recherche || avecDettesSeulement) && (
          <Button variant="ghost" size="sm" className="h-8 text-xs"
            onClick={() => { setRecherche(""); setAvecDettesSeulement(false); }}>
            <X className="h-3 w-3 mr-1" /> Réinitialiser
          </Button>
        )}
      </div>

      {/* Liste */}
      {chargement ? (
        <div className="flex justify-center py-12">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : fournisseurs.length === 0 ? (
        <div className="text-center py-12 text-muted-foreground">
          <Truck className="h-10 w-10 mx-auto mb-3 opacity-20" />
          <p className="text-sm">Aucun fournisseur</p>
          <Button size="sm" variant="outline" className="mt-3"
            onClick={() => setModalNouv(true)}>
            <Plus className="h-4 w-4 mr-1" /> Ajouter un fournisseur
          </Button>
        </div>
      ) : (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Truck className="h-4 w-4" />
              {total} fournisseur{total > 1 ? "s" : ""}
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div className="divide-y divide-border">
              {fournisseurs.map(f => {
                const expanded = expandeId === f.id;
                return (
                  <div key={f.id}>
                    <div className="flex items-center justify-between py-3 px-4
                                    hover:bg-muted/40 transition-colors">
                      {/* Info principale */}
                      <div className="flex items-center gap-3 min-w-0">
                        <button
                          onClick={() => setExpandeId(expanded ? null : f.id)}
                          className="text-muted-foreground hover:text-foreground">
                          {expanded
                            ? <ChevronDown className="h-4 w-4" />
                            : <ChevronRight className="h-4 w-4" />
                          }
                        </button>
                        <div className="min-w-0">
                          <div className="flex items-center gap-2">
                            <p className="text-sm font-medium truncate">{f.nom}</p>
                            {f.est_voisin && (
                              <Badge variant="outline" className="text-[10px] shrink-0">
                                Voisin
                              </Badge>
                            )}
                          </div>
                          {f.telephone && (
                            <p className="text-xs text-muted-foreground">{f.telephone}</p>
                          )}
                        </div>
                      </div>

                      {/* Actions */}
                      <div className="flex items-center gap-3 shrink-0">
                        {(f.dette ?? 0) > 0 && (
                          <div className="text-right">
                            <p className="text-xs text-muted-foreground">Dette</p>
                            <p className="text-sm font-bold text-orange-600">
                              {fmt(f.dette)}
                            </p>
                          </div>
                        )}
                        {(f.dette ?? 0) > 0 && (
                          <Button size="sm" variant="outline"
                            className="h-7 text-xs gap-1 border-orange-200
                                       text-orange-700 hover:bg-orange-50"
                            onClick={() => {
                              setFournisseurActif(f);
                              setModalRegler(true);
                            }}>
                            <Banknote className="h-3 w-3" /> Régler
                          </Button>
                        )}
                        <Button size="sm" variant="outline"
                          className="h-7 text-xs gap-1"
                          onClick={() => onOuvrirFiche?.(f.id)}>
                          <FileText className="h-3 w-3" /> Fiche
                        </Button>
                      </div>
                    </div>

                    {/* Détail déplié */}
                    {expanded && (
                      <div className="px-12 py-3 bg-muted/20 border-t border-border
                                      text-xs text-muted-foreground space-y-1.5">
                        {f.total_achats > 0 && (
                          <div className="flex justify-between">
                            <span>Total achats</span>
                            <span className="font-medium text-foreground">
                              {fmt(f.total_achats)}
                            </span>
                          </div>
                        )}
                        {f.total_paye > 0 && (
                          <div className="flex justify-between">
                            <span>Total payé</span>
                            <span className="text-green-600">{fmt(f.total_paye)}</span>
                          </div>
                        )}
                        {(f.dette ?? 0) > 0 && (
                          <div className="flex justify-between font-medium
                                          text-orange-600 border-t border-border pt-1">
                            <span>Reste dû</span>
                            <span>{fmt(f.dette)}</span>
                          </div>
                        )}
                        {(f.dette ?? 0) === 0 && f.total_achats > 0 && (
                          <div className="flex items-center gap-1 text-green-600">
                            <CheckCircle2 className="h-3.5 w-3.5" />
                            <span>Aucune dette</span>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
            <Pagination page={page} total={total}
              limite={LIMITE} onChanger={p => { setPage(p); charger(p); }} />
          </CardContent>
        </Card>
      )}

      {/* Modals */}
      <ModalNouveauFournisseur
        ouvert={modalNouv}
        onFermer={() => setModalNouv(false)}
        onCree={() => { setModalNouv(false); charger(0); }}
      />

      <ModalReglementRapide
        ouvert={modalRegler}
        fournisseur={fournisseurActif}
        onFermer={() => setModalRegler(false)}
        onRegle={() => { setModalRegler(false); charger(page); }}
      />
    </div>
  );
}