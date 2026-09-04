import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Search, Loader2, RotateCcw, ArrowLeftRight,
  Wallet, Gift, ChevronDown, ChevronRight, Truck
} from "lucide-react";
import { RetourFournisseur } from "@/components/RetourFournisseur";
import {
  ModalRemboursement, ModalAvoirConserve, ModalEchange,
  formaterMontant, formaterDate,
} from "@/components/ModalsRetour";
import type {
  ArticleAchat, LigneVente, Vente, Avoir,
} from "@/components/ModalsRetour";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { message } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

// =====================================================================
//  Types
// =====================================================================

// Types, formateurs et modals vivent desormais dans
// components/ModalsRetour.tsx : l'ecran Pieces lance les memes
// operations sur une facture, et deux copies auraient diverge.

// =====================================================================
//  Page Retours
// =====================================================================

export function Retours() {
  const [recherche, setRecherche] = useState("");
  const [ventes, setVentes] = useState<Vente[]>([]);
  const [avoirs, setAvoirs] = useState<Avoir[]>([]);
  const [articles, setArticles] = useState<ArticleAchat[]>([]);
  const [chargement, setChargement] = useState(true);
  const [onglet, setOnglet] = useState<"retours" | "avoirs" | "fournisseur">("retours");
  const [venteExpandee, setVenteExpandee] = useState<string | null>(null);
  // D40 — le client de passage ne peut pas recevoir d'avoir.
  const [clientGeneriqueId, setClientGeneriqueId] = useState<string | null>(null);
  const [bonSortieActif, setBonSortieActif] = useState(false);

  // Modals
  const [venteSelectionnee, setVenteSelectionnee] = useState<Vente | null>(null);
  const [ligneSelectionnee, setLigneSelectionnee] = useState<LigneVente | null>(null);
  const [modalRemboursement, setModalRemboursement] = useState(false);
  const [modalAvoir, setModalAvoir] = useState(false);
  const [modalEchange, setModalEchange] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const [v, a, arts, gen, bs] = await Promise.all([
        invoke<Vente[]>("lire_ventes_recentes"),
        invoke<Avoir[]>("lire_avoirs_ouverts_tous"),
        invoke<ArticleAchat[]>("lire_articles_avec_unites"),
        invoke<{ id: string }>("lire_client_generique"),
        invoke<boolean>("lire_config_bon_sortie").catch(() => false),
      ]);
      setVentes(v);
      setAvoirs(a);
      setArticles(arts);
      setClientGeneriqueId(gen?.id ?? null);
      setBonSortieActif(bs);
    } catch (e) {
      console.error("Erreur chargement retours :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  // Un avoir au comptant part sur le client generique, commun a tous
  // les clients de passage : il serait inutilisable (D40).
  const estComptant = (vente: Vente) =>
    clientGeneriqueId !== null && vente.client_id === clientGeneriqueId;

  function ouvrirModal(
    vente: Vente,
    ligne: LigneVente,
    mode: "remboursement" | "avoir" | "echange"
  ) {
    setVenteSelectionnee(vente);
    setLigneSelectionnee(ligne);
    if (mode === "remboursement") setModalRemboursement(true);
    else if (mode === "avoir") setModalAvoir(true);
    else setModalEchange(true);
  }

  async function handleApresOperation() {
    setModalRemboursement(false);
    setModalAvoir(false);
    setModalEchange(false);
    await charger();
    await message("Opération enregistrée ✓", { title: "Succès", kind: "info" });
  }

  const ventesFiltrees = ventes.filter(v =>
    v.client_nom.toLowerCase().includes(recherche.toLowerCase()) ||
    (v.numero_facture && v.numero_facture.includes(recherche))
  );

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
        <h1 className="text-2xl font-semibold">Retours & Avoirs</h1>
      </div>

      {/* Onglets */}
      <div className="flex gap-1 mb-6 border-b border-border">
        {[
          { key: "retours", label: "Retours", icone: RotateCcw },
          { key: "avoirs", label: `Avoirs ouverts (${avoirs.length})`, icone: Gift },
          { key: "fournisseur", label: "Retour fournisseur", icone: Truck },
        ].map(o => {
          const Icone = o.icone;
          return (
            <button key={o.key}
              onClick={() => setOnglet(o.key as typeof onglet)}
              className={cn(
                "flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors",
                onglet === o.key
                  ? "border-primary text-primary"
                  : "border-transparent text-muted-foreground hover:text-foreground"
              )}>
              <Icone className="h-4 w-4" />
              {o.label}
            </button>
          );
        })}
      </div>

      {/* Onglet Retour fournisseur — flux inverse : stock sort, dette baisse */}
      {onglet === "fournisseur" && (
        <RetourFournisseur onTermine={charger} />
      )}

      {/* Onglet Retours */}
      {onglet === "retours" && (
        <>
          <div className="relative mb-4 max-w-sm">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input value={recherche} onChange={e => setRecherche(e.target.value)}
              placeholder="Client ou numéro de facture..."
              className="pl-8" />
          </div>

          <div className="space-y-2">
            {ventesFiltrees.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-12">
                Aucune vente trouvée
              </p>
            ) : (
              ventesFiltrees.map(vente => (
                <Card key={vente.id} className="overflow-hidden">
                  <button
                    onClick={() => setVenteExpandee(
                      venteExpandee === vente.id ? null : vente.id
                    )}
                    className="w-full flex items-center justify-between px-4 py-3 hover:bg-muted/40 transition-colors text-left"
                  >
                    <div className="flex items-center gap-2">
                      {venteExpandee === vente.id
                        ? <ChevronDown className="h-4 w-4 text-muted-foreground" />
                        : <ChevronRight className="h-4 w-4 text-muted-foreground" />
                      }
                      <div>
                        <div className="flex items-center gap-2">
                          <p className="text-sm font-medium">{vente.client_nom}</p>
                          {vente.numero_facture && (
                            <span className="text-xs text-muted-foreground">
                              #{vente.numero_facture}
                            </span>
                          )}
                        </div>
                        <p className="text-xs text-muted-foreground">
                          {formaterDate(vente.date_vente)}
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-sm font-semibold">
                        {formaterMontant(vente.total)}
                      </span>
                      <Badge variant={
                        vente.statut === "payee" ? "secondary"
                        : vente.statut === "creance_ouverte" ? "destructive"
                        : "outline"
                      } className="text-xs">
                        {vente.statut === "payee" ? "Payée"
                          : vente.statut === "creance_ouverte" ? "Créance"
                          : "Partiel"}
                      </Badge>
                    </div>
                  </button>

                  {venteExpandee === vente.id && (
                    <div className="border-t border-border bg-muted/20 px-4 py-3">
                      <p className="text-xs font-medium text-muted-foreground mb-3">
                        Choisir un article et le type de retour
                      </p>
                      <div className="space-y-2">
                        {vente.lignes.map(ligne => (
                          <div key={ligne.id}
                            className="flex items-center justify-between py-2 px-3 rounded-md bg-card border border-border">
                            <div>
                              <p className="text-sm font-medium">{ligne.article_nom}</p>
                              <p className="text-xs text-muted-foreground">
                                {ligne.quantite} {ligne.unite_libelle} ×{" "}
                                {formaterMontant(ligne.prix_pratique)}
                              </p>
                            </div>
                            <div className="flex items-center gap-1">
                              <Button size="sm" variant="outline"
                                onClick={() => ouvrirModal(vente, ligne, "remboursement")}
                                className="h-7 text-xs px-2">
                                <Wallet className="h-3 w-3 mr-1" />
                                Rembourser
                              </Button>
                              <Button size="sm" variant="outline"
                                onClick={() => ouvrirModal(vente, ligne, "echange")}
                                className="h-7 text-xs px-2">
                                <ArrowLeftRight className="h-3 w-3 mr-1" />
                                Échanger
                              </Button>
                              {!estComptant(vente) && (
                                <Button size="sm" variant="outline"
                                  onClick={() => ouvrirModal(vente, ligne, "avoir")}
                                  className="h-7 text-xs px-2">
                                  <Gift className="h-3 w-3 mr-1" />
                                  Avoir
                                </Button>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </Card>
              ))
            )}
          </div>
        </>
      )}

      {/* Onglet Avoirs */}
      {onglet === "avoirs" && (
        <div className="space-y-2">
          {avoirs.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-12">
              Aucun avoir ouvert
            </p>
          ) : (
            avoirs.map(avoir => (
              <div key={avoir.id}
                className="flex items-center justify-between px-4 py-3 border border-border rounded-md">
                <div>
                  <p className="text-sm font-medium">{avoir.client_nom}</p>
                  <p className="text-xs text-muted-foreground">
                    {avoir.client_code} · Créé le {formaterDate(avoir.cree_le)}
                  </p>
                </div>
                <div className="text-right">
                  <p className="text-sm font-semibold text-green-600">
                    {formaterMontant(avoir.montant)}
                  </p>
                  <p className="text-xs text-muted-foreground">Disponible</p>
                </div>
              </div>
            ))
          )}
        </div>
      )}

      {/* Modals */}
      <ModalRemboursement
        ouvert={modalRemboursement}
        vente={venteSelectionnee}
        ligne={ligneSelectionnee}
        onFermer={() => setModalRemboursement(false)}
        onConfirmer={handleApresOperation}
      />

      <ModalAvoirConserve
        ouvert={modalAvoir}
        vente={venteSelectionnee}
        ligne={ligneSelectionnee}
        onFermer={() => setModalAvoir(false)}
        onConfirmer={handleApresOperation}
      />

      <ModalEchange
        ouvert={modalEchange}
        comptant={venteSelectionnee ? estComptant(venteSelectionnee) : false}
        bonSortieActif={bonSortieActif}
        vente={venteSelectionnee}
        ligne={ligneSelectionnee}
        articles={articles}
        onFermer={() => setModalEchange(false)}
        onConfirmer={handleApresOperation}
      />
    </div>
  );
}
