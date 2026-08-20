import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Users, TrendingUp, Loader2, Plus, X,
  Search, Wallet, ChevronDown, ChevronRight,
  CheckCircle2, FileText,
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
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { Pagination } from "@/components/Pagination";
import { UTILISATEUR_ACTIF } from "@/App";

const LIMITE = 30;

// =====================================================================
//  Types
// =====================================================================

interface ClientRow {
  id: string; code: string; nom: string;
  telephone?: string; total_creances: number; nb_ventes: number;
}

interface Creance {
  id: string; date_vente: string; statut: string;
  client_id: string; client_nom: string; client_code: string;
  telephone?: string; numero_facture?: string;
  total: number; total_paye: number; reste: number;
}

interface PageResult {
  donnees: ClientRow[]; total: number; pages: number; page: number;
}

// =====================================================================
//  Props — navigation vers la fiche client
// =====================================================================

interface ClientsProps {
  onOuvrirFiche?: (clientId: string) => void;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Modal : Règlement d'une créance
// =====================================================================

function ModalReglementCreance({
  ouvert, creance, onFermer, onRegle,
}: {
  ouvert: boolean;
  creance: Creance | null;
  onFermer: () => void;
  onRegle: () => void;
}) {
  const [montant, setMontant] = useState("");
  const [mode, setMode] = useState("especes");
  const [chargement, setChargement] = useState(false);
  const [resultat, setResultat] = useState<{
    montant_encaisse: number; reste_apres: number; soldee: boolean;
  } | null>(null);

  useEffect(() => {
    if (ouvert && creance) {
      setMontant(creance.reste.toString());
      setMode("especes");
      setResultat(null);
    }
  }, [ouvert, creance]);

  const montantNum = parseMontant(montant);

  async function handleRegler() {
    if (!creance || montantNum <= 0) return;
    setChargement(true);
    try {
      const role = UTILISATEUR_ACTIF?.role ?? "patron";
      const res = await invoke<{
        montant_encaisse: number; reste_apres: number; soldee: boolean;
      }>("regler_creance", {
        venteId: creance.id,
        montant: montantNum,
        mode,
        utilisateurRole: role,
      });
      setResultat(res);
      if (res.soldee) {
        setTimeout(() => { onRegle(); }, 1500);
      }
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  if (!creance) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Wallet className="h-4 w-4" />
            Règlement créance
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 space-y-1">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Client</span>
              <span className="font-medium">{creance.client_nom}</span>
            </div>
            {creance.numero_facture && (
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Facture</span>
                <span className="font-mono text-xs">{creance.numero_facture}</span>
              </div>
            )}
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Date</span>
              <span>{fmtDate(creance.date_vente)}</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Total facture</span>
              <span>{fmt(creance.total)}</span>
            </div>
            {creance.total_paye > 0 && (
              <div className="flex justify-between text-sm text-green-600">
                <span>Déjà payé</span>
                <span>{fmt(creance.total_paye)}</span>
              </div>
            )}
            <div className="flex justify-between text-sm font-bold border-t border-border pt-1">
              <span>Reste dû</span>
              <span className="text-orange-500">{fmt(creance.reste)}</span>
            </div>
          </div>

          {resultat ? (
            <div className={`flex items-center gap-3 p-3 rounded-lg ${
              resultat.soldee
                ? "bg-green-50 border border-green-200"
                : "bg-blue-50 border border-blue-200"
            }`}>
              <CheckCircle2 className={`h-5 w-5 shrink-0 ${
                resultat.soldee ? "text-green-600" : "text-blue-600"
              }`} />
              <div>
                <p className="text-sm font-medium">
                  {resultat.soldee ? "Créance soldée ✓" : "Paiement partiel enregistré"}
                </p>
                {!resultat.soldee && (
                  <p className="text-xs text-muted-foreground">
                    Reste encore : {fmt(resultat.reste_apres)}
                  </p>
                )}
              </div>
            </div>
          ) : (
            <>
              <div>
                <Label>Montant encaissé (F)</Label>
                <MoneyInput value={montant} onChange={setMontant}
                  className="mt-1" autoFocus />
                {montantNum > creance.reste && (
                  <p className="text-xs text-orange-500 mt-1">
                    Limité au reste dû : {fmt(creance.reste)}
                  </p>
                )}
              </div>
              <div>
                <Label>Mode de paiement</Label>
                <Select value={mode} onValueChange={v => { if (v) setMode(v); }}>
                  <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="especes">Espèces</SelectItem>
                    <SelectItem value="orange_money">Orange Money</SelectItem>
                    <SelectItem value="moov_money">Moov Money</SelectItem>
                    <SelectItem value="cheque">Chèque</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={onFermer} className="flex-1">
                  Annuler
                </Button>
                <Button
                  onClick={handleRegler}
                  disabled={montantNum <= 0 || chargement}
                  className="flex-1">
                  {chargement
                    ? <Loader2 className="h-4 w-4 animate-spin" />
                    : "Encaisser"
                  }
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
//  Modal : Nouveau client
// =====================================================================

function ModalNouveauClient({
  ouvert, onFermer, onCreer,
}: { ouvert: boolean; onFermer: () => void; onCreer: () => void }) {
  const [nom, setNom] = useState("");
  const [telephone, setTelephone] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargement(true);
    try {
      await invoke("creer_client_rapide", {
        nom: nom.trim(), telephone: telephone.trim() || null,
      });
      setNom(""); setTelephone(""); onCreer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouveau client</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Nom du client" autoFocus className="mt-1"
              onKeyDown={e => e.key === "Enter" && handleCreer()} />
          </div>
          <div>
            <Label>Téléphone</Label>
            <Input value={telephone} onChange={e => setTelephone(e.target.value)}
              placeholder="76 00 00 00" className="mt-1" />
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
//  Page Clients
// =====================================================================

export function Clients({ onOuvrirFiche }: ClientsProps) {
  const [resultat, setResultat] = useState<PageResult>({
    donnees: [], total: 0, pages: 0, page: 0,
  });
  const [creances, setCreances] = useState<Creance[]>([]);
  const [chargement, setChargement] = useState(true);
  const [page, setPage] = useState(0);
  const [recherche, setRecherche] = useState("");
  const [avecCreancesSeulement, setAvecCreancesSeulement] = useState(false);
  const [onglet, setOnglet] = useState<"clients" | "creances">("clients");
  const [creanceSelectionnee, setCreanceSelectionnee] = useState<Creance | null>(null);
  const [modalRegler, setModalRegler] = useState(false);
  const [modalNouveauClient, setModalNouveauClient] = useState(false);
  const [expandeId, setExpandeId] = useState<string | null>(null);

  const chargerClients = useCallback(async (p: number) => {
    setChargement(true);
    try {
      const data = await invoke<PageResult>("lire_clients_pagines", {
        page: p, limite: LIMITE,
        recherche: recherche || null,
        avecCreancesSeulement,
      });
      setResultat(data);
    } catch (e) {
      console.error("Erreur clients :", e);
    } finally { setChargement(false); }
  }, [recherche, avecCreancesSeulement]);

  const chargerCreances = useCallback(async () => {
    setChargement(true);
    try {
      const data = await invoke<Creance[]>("lire_creances_ouvertes", {
        recherche: recherche || null,
      });
      setCreances(data);
    } catch (e) {
      console.error("Erreur créances :", e);
    } finally { setChargement(false); }
  }, [recherche]);

  useEffect(() => {
    setPage(0);
    if (onglet === "clients") chargerClients(0);
    else chargerCreances();
  }, [recherche, avecCreancesSeulement, onglet]);

  useEffect(() => {
    if (onglet === "clients") chargerClients(page);
  }, [page]);

  function changerPage(p: number) { setPage(p); window.scrollTo(0, 0); }

  async function handleApresReglement() {
    setModalRegler(false);
    if (onglet === "clients") chargerClients(page);
    else chargerCreances();
  }

  const creancesParClient = creances.reduce<Record<string, Creance[]>>((acc, c) => {
    if (!acc[c.client_id]) acc[c.client_id] = [];
    acc[c.client_id].push(c);
    return acc;
  }, {});

  const totalCreances = creances.reduce((s, c) => s + c.reste, 0);

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

      {/* Onglets */}
      <div className="flex gap-1 mb-4 border-b border-border">
        <button onClick={() => setOnglet("clients")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            onglet === "clients"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}>
          <Users className="h-4 w-4" /> Clients
        </button>
        <button onClick={() => setOnglet("creances")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            onglet === "creances"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground"
          }`}>
          <TrendingUp className="h-4 w-4" />
          Créances
          {creances.length > 0 && (
            <Badge variant="destructive" className="text-xs ml-1">
              {creances.length}
            </Badge>
          )}
        </button>
      </div>

      {/* Filtres */}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <div className="relative">
          <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
          <Input value={recherche} onChange={e => setRecherche(e.target.value)}
            placeholder="Nom ou téléphone..."
            className="h-8 text-sm w-48 pl-8" />
        </div>
        {onglet === "clients" && (
          <button
            onClick={() => setAvecCreancesSeulement(!avecCreancesSeulement)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md border text-xs font-medium transition-colors ${
              avecCreancesSeulement
                ? "border-orange-400 bg-orange-50 text-orange-700"
                : "border-border text-muted-foreground hover:bg-muted"
            }`}>
            <TrendingUp className="h-3 w-3" /> Avec créances
          </button>
        )}
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
      ) : onglet === "clients" ? (
        // ---- Onglet Clients ----
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm flex items-center gap-2">
              <Users className="h-4 w-4" />
              {resultat.total} clients
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            {resultat.donnees.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun client
              </p>
            ) : (
              <>
                <div className="divide-y divide-border">
                  {resultat.donnees.map(c => (
                    <div key={c.id}
                      className="flex items-center justify-between py-2.5 px-4
                                 hover:bg-muted/40 transition-colors">
                      <div>
                        <p className="text-sm font-medium">{c.nom}</p>
                        <p className="text-xs text-muted-foreground">
                          {c.code}{c.telephone ? ` · ${c.telephone}` : ""}
                        </p>
                      </div>
                      <div className="flex items-center gap-3">
                        {c.total_creances > 0 && (
                          <span className="text-sm font-semibold text-orange-500">
                            {fmt(c.total_creances)}
                          </span>
                        )}
                        <p className="text-xs text-muted-foreground">
                          {c.nb_ventes} vente{c.nb_ventes > 1 ? "s" : ""}
                        </p>
                        {/* ← Bouton Fiche */}
                        <Button
                          size="sm" variant="outline"
                          className="h-7 text-xs gap-1"
                          onClick={() => onOuvrirFiche?.(c.id)}>
                          <FileText className="h-3 w-3" />
                          Fiche
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
                <Pagination page={page} total={resultat.total}
                  limite={LIMITE} onChanger={changerPage} />
              </>
            )}
          </CardContent>
        </Card>
      ) : (
        // ---- Onglet Créances ----
        <div className="space-y-3">
          {creances.length > 0 && (
            <div className="flex items-center justify-between px-4 py-3
                            bg-orange-50 dark:bg-orange-950/20
                            border border-orange-200 rounded-lg mb-2">
              <div className="flex items-center gap-2">
                <TrendingUp className="h-4 w-4 text-orange-500" />
                <span className="text-sm font-medium text-orange-700">
                  Total créances ouvertes
                </span>
              </div>
              <span className="text-lg font-bold text-orange-500">
                {fmt(totalCreances)}
              </span>
            </div>
          )}

          {creances.length === 0 ? (
            <div className="text-center py-12">
              <CheckCircle2 className="h-10 w-10 text-green-500 mx-auto mb-3" />
              <p className="text-sm font-medium">Aucune créance ouverte</p>
              <p className="text-xs text-muted-foreground mt-1">
                Tous les clients sont à jour
              </p>
            </div>
          ) : (
            Object.entries(creancesParClient).map(([clientId, ventes]) => {
              const premierClient = ventes[0];
              const totalClient = ventes.reduce((s, v) => s + v.reste, 0);
              const expanded = expandeId === clientId;

              return (
                <Card key={clientId} className="overflow-hidden">
                  <button
                    onClick={() => setExpandeId(expanded ? null : clientId)}
                    className="w-full flex items-center justify-between px-4 py-3
                               hover:bg-muted/40 transition-colors text-left">
                    <div className="flex items-center gap-2">
                      {expanded
                        ? <ChevronDown className="h-4 w-4 text-muted-foreground" />
                        : <ChevronRight className="h-4 w-4 text-muted-foreground" />
                      }
                      <div>
                        <p className="text-sm font-medium">{premierClient.client_nom}</p>
                        <p className="text-xs text-muted-foreground">
                          {premierClient.client_code}
                          {premierClient.telephone ? ` · ${premierClient.telephone}` : ""}
                          {" · "}{ventes.length} vente{ventes.length > 1 ? "s" : ""}
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-bold text-orange-500">
                        {fmt(totalClient)}
                      </span>
                      {/* Fiche depuis l'onglet créances aussi */}
                      <Button size="sm" variant="outline"
                        className="h-7 text-xs gap-1"
                        onClick={e => {
                          e.stopPropagation();
                          onOuvrirFiche?.(clientId);
                        }}>
                        <FileText className="h-3 w-3" /> Fiche
                      </Button>
                    </div>
                  </button>

                  {expanded && (
                    <div className="border-t border-border bg-muted/20 p-3 space-y-2">
                      {ventes.map(v => (
                        <div key={v.id}
                          className="flex items-center justify-between py-2 px-3
                                     bg-card border border-border rounded-md">
                          <div>
                            {v.numero_facture && (
                              <p className="text-xs font-mono text-muted-foreground">
                                {v.numero_facture}
                              </p>
                            )}
                            <p className="text-xs text-muted-foreground">
                              {fmtDate(v.date_vente)}
                            </p>
                            {v.total_paye > 0 && (
                              <p className="text-xs text-green-600">
                                Acompte : {fmt(v.total_paye)}
                              </p>
                            )}
                          </div>
                          <div className="flex items-center gap-3">
                            <div className="text-right">
                              <p className="text-xs text-muted-foreground">
                                Total : {fmt(v.total)}
                              </p>
                              <p className="text-sm font-semibold text-orange-500">
                                Reste : {fmt(v.reste)}
                              </p>
                            </div>
                            <Button size="sm"
                              onClick={() => {
                                setCreanceSelectionnee(v);
                                setModalRegler(true);
                              }}
                              className="h-8 text-xs">
                              <Wallet className="h-3 w-3 mr-1" />
                              Régler
                            </Button>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </Card>
              );
            })
          )}
        </div>
      )}

      {/* Modals */}
      <ModalReglementCreance
        ouvert={modalRegler}
        creance={creanceSelectionnee}
        onFermer={() => setModalRegler(false)}
        onRegle={handleApresReglement}
      />

      <ModalNouveauClient
        ouvert={modalNouveauClient}
        onFermer={() => setModalNouveauClient(false)}
        onCreer={() => { setModalNouveauClient(false); chargerClients(page); }}
      />
    </div>
  );
}