// components/RetourFournisseur.tsx
//
// Onglet « Fournisseur » de l'écran Retours.
//
// Le retour part TOUJOURS d'une facture d'achat : on retourne ce qu'on a
// acheté, au prix payé sur cette facture-là. Ça évite de retourner plus
// que la quantité reçue, et de ressaisir article, unité et prix.
//
// Flux inversé par rapport au retour client : le stock SORT et la dette
// DIMINUE.

import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message } from "@tauri-apps/plugin-dialog";
import {
  Truck, Search, Loader2, Undo2, ChevronDown, ChevronRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { UTILISATEUR_ACTIF } from "@/App";

interface Fournisseur { id: string; nom: string; telephone?: string | null; }

/** Ce qu'on peut réellement retourner sur une ligne. */
function plafond(l: LigneFacture): number {
  // `stock_disponible` vaut -1 quand la facture n'a pas de dépôt (base
  // ancienne) : on ne sait pas d'où la marchandise repartirait, donc on
  // ne promet rien de plus que le reliquat. Le refus à
  // l'enregistrement reste le filet.
  return l.stock_disponible < 0 ? l.quantite_restante : l.retournable;
}

interface LigneFacture {
  ligne_id: string;
  article_id: string;
  article_nom: string;
  unite_vente_id: string;
  unite_libelle: string;
  facteur: number;
  quantite: number;
  prix_achat: number;
  deja_retourne: number;
  quantite_restante: number;
  /** Stock réel dans le dépôt de la facture. -1 : dépôt inconnu. */
  stock_disponible: number;
  /** Le plus petit des deux : reliquat de facture et stock réel. */
  retournable: number;
}

interface FactureRetournable {
  piece_id: string;
  numero: string;
  date_piece: string;
  statut: string;
  /** Dépôt d'où la marchandise repartira. */
  depot_nom: string;
  total: number;
  lignes: LigneFacture[];
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

export function RetourFournisseur({ onTermine }: { onTermine: () => void }) {
  const [fournisseurs, setFournisseurs] = useState<Fournisseur[]>([]);
  const [fournisseurId, setFournisseurId] = useState<string>("");
  const [factures, setFactures] = useState<FactureRetournable[]>([]);
  const [ouverte, setOuverte] = useState<string | null>(null);
  const [chargement, setChargement] = useState(false);
  const [envoi, setEnvoi] = useState(false);

  // ligne_id -> quantité à retourner
  const [quantites, setQuantites] = useState<Record<string, string>>({});
  const [mode, setMode] = useState<"avoir" | "remboursement">("avoir");
  const [modeEncaissement, setModeEncaissement] = useState("especes");
  const [motif, setMotif] = useState("");
  const [recherche, setRecherche] = useState("");

  useEffect(() => {
    invoke<Fournisseur[]>("lire_fournisseurs")
      .then(setFournisseurs)
      .catch(console.error);
  }, []);

  useEffect(() => {
    if (!fournisseurId) { setFactures([]); return; }
    setChargement(true);
    setQuantites({});
    setOuverte(null);
    invoke<FactureRetournable[]>("lire_factures_fournisseur_retournables", {
      fournisseurId,
    })
      .then(setFactures)
      .catch(e => console.error("Erreur factures :", e))
      .finally(() => setChargement(false));
  }, [fournisseurId]);

  const facture = factures.find(f => f.piece_id === ouverte) ?? null;

  const lignesRetour = (facture?.lignes ?? [])
    .map(l => ({ ligne: l, qte: parseFloat(quantites[l.ligne_id] || "0") || 0 }))
    .filter(x => x.qte > 0);

  const totalRetour = lignesRetour.reduce(
    (s, x) => s + Math.round(x.ligne.prix_achat * x.qte), 0,
  );

  // Deux plafonds, pas un seul.
  //
  // Le reliquat de facture dit ce qu'on n'a pas encore rendu. Il ne dit
  // pas ce qu'on PEUT rendre : la marchandise a pu être vendue depuis.
  // Le serveur refuse le retour qui dépasse le stock ; l'écran doit le
  // dire avant le clic, sinon on saisit sa ligne pour rien.
  const depassement = (facture?.lignes ?? []).some(l => {
    const q = parseFloat(quantites[l.ligne_id] || "0") || 0;
    return q > plafond(l);
  });

  async function handleValider() {
    if (!facture || lignesRetour.length === 0 || depassement) return;
    setEnvoi(true);
    try {
      const res = await invoke<{ numero: string; total: number }>(
        "enregistrer_retour_fournisseur", {
          fournisseurId,
          depotId: null,
          // Rattache l'AVF a sa facture : le reliquat se calcule par
          // facture, pas par article.
          pieceOrigineId: facture.piece_id,
          modeResolution: mode,
          modeEncaissement: mode === "remboursement" ? modeEncaissement : null,
          motif: motif.trim() || `Retour sur ${facture.numero}`,
          utilisateurRole: UTILISATEUR_ACTIF?.role ?? "employe",
          lignes: lignesRetour.map(x => ({
            article_id:     x.ligne.article_id,
            unite_vente_id: x.ligne.unite_vente_id,
            quantite:       x.qte,
            facteur:        x.ligne.facteur,
            prix_achat:     x.ligne.prix_achat,
          })),
        },
      );

      await message(
        `Avoir ${res.numero} créé — ${fmt(res.total)}.\n` +
        (mode === "avoir"
          ? "La dette fournisseur a été réduite d'autant."
          : "Le remboursement a été encaissé."),
        { title: "Retour enregistré", kind: "info" },
      );

      setQuantites({});
      setMotif("");
      setOuverte(null);
      // Recharger les reliquats.
      const maj = await invoke<FactureRetournable[]>(
        "lire_factures_fournisseur_retournables", { fournisseurId },
      );
      setFactures(maj);
      onTermine();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setEnvoi(false);
    }
  }

  const facturesFiltrees = factures.filter(f =>
    !recherche || f.numero.toLowerCase().includes(recherche.toLowerCase()),
  );

  return (
    <div className="space-y-4">

      {/* Choix du fournisseur */}
      <div className="flex flex-wrap items-end gap-3">
        <div className="min-w-[240px]">
          <Label className="text-xs mb-1 block">Fournisseur</Label>
          <Select value={fournisseurId}
            onValueChange={v => { if (v) setFournisseurId(v); }}>
            <SelectTrigger className="h-9">
              <SelectValue placeholder="Choisir un fournisseur…" />
            </SelectTrigger>
            <SelectContent>
              {fournisseurs.map(f => (
                <SelectItem key={f.id} value={f.id}>{f.nom}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {factures.length > 0 && (
          <div className="relative max-w-[220px]">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input value={recherche} onChange={e => setRecherche(e.target.value)}
              placeholder="N° de facture…" className="h-9 pl-8 text-sm" />
          </div>
        )}
      </div>

      {chargement && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground py-6">
          <Loader2 className="h-4 w-4 animate-spin" /> Chargement des factures…
        </div>
      )}

      {!chargement && fournisseurId && facturesFiltrees.length === 0 && (
        <p className="text-sm text-muted-foreground py-6">
          Aucune facture d'achat pour ce fournisseur.
        </p>
      )}

      {!fournisseurId && (
        <div className="flex items-center gap-3 p-4 bg-muted rounded-lg">
          <Truck className="h-5 w-5 text-muted-foreground shrink-0" />
          <p className="text-sm text-muted-foreground">
            Choisir un fournisseur pour voir ses factures d'achat.
            Le retour part toujours d'une facture.
          </p>
        </div>
      )}

      {/* Factures */}
      <div className="space-y-2">
        {facturesFiltrees.map(f => {
          const estOuverte = ouverte === f.piece_id;
          const restant = f.lignes.some(l => l.quantite_restante > 0);
          return (
            <div key={f.piece_id} className="border border-border rounded-lg">
              <button
                onClick={() => {
                  setOuverte(estOuverte ? null : f.piece_id);
                  setQuantites({});
                }}
                className="w-full flex items-center gap-3 px-4 py-3 hover:bg-muted/40 text-left">
                {estOuverte
                  ? <ChevronDown className="h-4 w-4 shrink-0" />
                  : <ChevronRight className="h-4 w-4 shrink-0" />}
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium">{f.numero}</p>
                  <p className="text-xs text-muted-foreground">
                    {fmtDate(f.date_piece)} · {f.lignes.length} ligne(s)
                    {!restant && " · entièrement retournée"}
                  </p>
                </div>
                <span className="text-sm font-semibold">{fmt(f.total)}</span>
              </button>

              {estOuverte && (
                <div className="border-t border-border px-4 py-3 space-y-3">
                  {f.lignes.map(l => {
                    const val = quantites[l.ligne_id] || "";
                    const q = parseFloat(val) || 0;
                    const max = plafond(l);
                    const trop = q > max;
                    const bride = l.stock_disponible >= 0
                      && l.stock_disponible < l.quantite_restante;
                    return (
                      <div key={l.ligne_id}
                        className="flex flex-wrap items-center gap-3 text-sm">
                        <div className="flex-1 min-w-[180px]">
                          <p className="font-medium">{l.article_nom}</p>
                          <p className="text-xs text-muted-foreground">
                            Acheté {l.quantite} {l.unite_libelle} ×{" "}
                            {fmt(l.prix_achat)}
                            {l.deja_retourne > 0 && (
                              <> · déjà retourné {l.deja_retourne}</>
                            )}
                          </p>
                          {bride && (
                            <p className="text-xs text-amber-600">
                              En stock : {l.stock_disponible} {l.unite_libelle}
                              {" "}— le reste a été vendu depuis.
                            </p>
                          )}
                        </div>
                        <div className="w-32">
                          <Input
                            type="number" min="0" step="any"
                            max={max}
                            disabled={max <= 0}
                            value={val}
                            onChange={e => setQuantites(p => ({
                              ...p, [l.ligne_id]: e.target.value,
                            }))}
                            placeholder={`max ${max}`}
                            className={cn("h-8 text-sm text-right",
                              trop && "border-destructive")} />
                        </div>
                        <span className="w-24 text-right font-medium">
                          {q > 0 ? fmt(Math.round(l.prix_achat * q)) : "—"}
                        </span>
                      </div>
                    );
                  })}

                  {depassement && (
                    <p className="text-xs text-destructive">
                      Une quantité dépasse ce qui peut être retourné —
                      reliquat de la facture, ou stock réellement présent.
                    </p>
                  )}

                  {/* Résolution */}
                  <div className="flex flex-wrap items-end gap-3 pt-2 border-t border-border">
                    <div className="min-w-[200px]">
                      <Label className="text-xs mb-1 block">Résolution</Label>
                      <Select value={mode}
                        onValueChange={v => { if (v) setMode(v as typeof mode); }}>
                        <SelectTrigger className="h-8 text-sm"><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="avoir">
                            Avoir — le fournisseur crédite
                          </SelectItem>
                          <SelectItem value="remboursement">
                            Remboursement — il rend l'argent
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    {mode === "remboursement" && (
                      <div className="min-w-[160px]">
                        <Label className="text-xs mb-1 block">Encaissement</Label>
                        <Select value={modeEncaissement}
                          onValueChange={v => { if (v) setModeEncaissement(v); }}>
                          <SelectTrigger className="h-8 text-sm"><SelectValue /></SelectTrigger>
                          <SelectContent>
                            <SelectItem value="especes">Espèces</SelectItem>
                            <SelectItem value="orange_money">Orange Money</SelectItem>
                            <SelectItem value="moov_money">Moov Money</SelectItem>
                            <SelectItem value="cheque">Chèque</SelectItem>
                          </SelectContent>
                        </Select>
                      </div>
                    )}

                    <div className="flex-1 min-w-[180px]">
                      <Label className="text-xs mb-1 block">Motif (optionnel)</Label>
                      <Input value={motif} onChange={e => setMotif(e.target.value)}
                        placeholder="Marchandise abîmée…" className="h-8 text-sm" />
                    </div>
                  </div>

                  <div className="flex items-center justify-between pt-2">
                    <span className="text-sm">
                      Total du retour :{" "}
                      <strong>{fmt(totalRetour)}</strong>
                    </span>
                    <Button size="sm" onClick={handleValider}
                      disabled={envoi || lignesRetour.length === 0 || depassement}>
                      {envoi
                        ? <Loader2 className="h-4 w-4 animate-spin" />
                        : <><Undo2 className="h-4 w-4 mr-2" /> Valider le retour</>}
                    </Button>
                  </div>

                  <p className="text-xs text-muted-foreground">
                    {mode === "avoir"
                      ? "Le stock sortira et la dette diminuera d'autant."
                      : "Le stock sortira et l'argent entrera en caisse."}
                  </p>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}