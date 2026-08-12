import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Users, TrendingUp, Loader2, Plus, X } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { message } from "@tauri-apps/plugin-dialog";
import { Pagination } from "@/components/Pagination";

const LIMITE = 30;

interface ClientRow {
  id: string;
  code: string;
  nom: string;
  telephone?: string;
  total_creances: number;
  nb_ventes: number;
}

interface PageResult {
  donnees: ClientRow[];
  total: number;
  pages: number;
  page: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function ModalNouveauClient({
  ouvert, onFermer, onCreer,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onCreer: () => void;
}) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      await invoke("creer_client_rapide", {
        nom: nom.trim(),
        telephone: telephone.trim() || null,
      });
      setNom(""); setTelephone("");
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
        <DialogHeader><DialogTitle>Nouveau client</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du client" autoFocus
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" />
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

export function Clients() {
  const [resultat, setResultat] = useState<PageResult>({
    donnees: [], total: 0, pages: 0, page: 0,
  });
  const [chargement, setChargement] = useState(true);
  const [page, setPage] = useState(0);
  const [recherche, setRecherche] = useState("");
  const [avecCreancesSeulement, setAvecCreancesSeulement] = useState(false);
  const [modalNouveauClient, setModalNouveauClient] = useState(false);

  const charger = useCallback(async (p: number) => {
    setChargement(true);
    try {
      const data = await invoke<PageResult>("lire_clients_pagines", {
        page: p,
        limite: LIMITE,
        recherche: recherche || null,
        avecCreancesSeulement,
      });
      setResultat(data);
    } catch (e) {
      console.error("Erreur clients :", e);
    } finally {
      setChargement(false);
    }
  }, [recherche, avecCreancesSeulement]);

  useEffect(() => {
    setPage(0);
    charger(0);
  }, [recherche, avecCreancesSeulement]);

  useEffect(() => { charger(page); }, [page]);

  function changerPage(p: number) {
    setPage(p);
    window.scrollTo(0, 0);
  }

  const avecCreances = resultat.donnees.filter(c => c.total_creances > 0);
  const sansCreances = resultat.donnees.filter(c => c.total_creances === 0);

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Clients</h1>
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{resultat.total} clients</Badge>
          <Button size="sm" onClick={() => setModalNouveauClient(true)}>
            <Plus className="h-4 w-4 mr-1" /> Nouveau
          </Button>
        </div>
      </div>

      {/* Filtres */}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <Input value={recherche} onChange={e => setRecherche(e.target.value)}
          placeholder="Nom ou téléphone..."
          className="h-8 text-sm w-48" />

        <button
          onClick={() => setAvecCreancesSeulement(!avecCreancesSeulement)}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md border text-xs font-medium transition-colors ${
            avecCreancesSeulement
              ? "border-orange-400 bg-orange-50 text-orange-700"
              : "border-border text-muted-foreground hover:bg-muted"
          }`}
        >
          <TrendingUp className="h-3 w-3" />
          Avec créances
        </button>

        {(recherche || avecCreancesSeulement) && (
          <Button variant="ghost" size="sm" className="h-8 text-xs"
            onClick={() => { setRecherche(""); setAvecCreancesSeulement(false); }}>
            <X className="h-3 w-3 mr-1" /> Réinitialiser
          </Button>
        )}
      </div>

      {chargement ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <>
          {/* Clients avec créances */}
          {avecCreances.length > 0 && (
            <Card className="mb-4">
              <CardHeader className="pb-3">
                <CardTitle className="text-sm flex items-center gap-2 text-orange-500">
                  <TrendingUp className="h-4 w-4" />
                  Créances ouvertes ({avecCreances.length})
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-1">
                  {avecCreances.map(c => (
                    <div key={c.id}
                      className="flex items-center justify-between py-2.5 px-3 rounded-md hover:bg-muted/40 transition-colors">
                      <div>
                        <p className="text-sm font-medium">{c.nom}</p>
                        <p className="text-xs text-muted-foreground">
                          {c.code}{c.telephone ? ` · ${c.telephone}` : ""}
                        </p>
                      </div>
                      <div className="text-right">
                        <p className="text-sm font-semibold text-orange-500">
                          {formaterMontant(c.total_creances)}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                        </p>
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {/* Clients à jour */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm flex items-center gap-2">
                <Users className="h-4 w-4" />
                Clients à jour ({sansCreances.length})
              </CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              {sansCreances.length === 0 ? (
                <p className="text-sm text-muted-foreground text-center py-8">
                  Aucun client
                </p>
              ) : (
                <>
                  <div className="divide-y divide-border">
                    {sansCreances.map(c => (
                      <div key={c.id}
                        className="flex items-center justify-between py-2.5 px-4 hover:bg-muted/40 transition-colors">
                        <div>
                          <p className="text-sm font-medium">{c.nom}</p>
                          <p className="text-xs text-muted-foreground">
                            {c.code}{c.telephone ? ` · ${c.telephone}` : ""}
                          </p>
                        </div>
                        <p className="text-xs text-muted-foreground">
                          {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                        </p>
                      </div>
                    ))}
                  </div>
                  <Pagination
                    page={page}
                    total={resultat.total}
                    limite={LIMITE}
                    onChanger={changerPage}
                  />
                </>
              )}
            </CardContent>
          </Card>
        </>
      )}

      <ModalNouveauClient
        ouvert={modalNouveauClient}
        onFermer={() => setModalNouveauClient(false)}
        onCreer={() => { setModalNouveauClient(false); charger(page); }}
      />
    </div>
  );
}