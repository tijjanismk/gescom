// components/ModalsRetour.tsx
//
// Les trois modals de retour — remboursement, avoir, échange.
//
// Extraits de `pages/Retours.tsx` sans changer une ligne de leur
// contenu : l'écran Pièces doit pouvoir lancer les mêmes opérations
// sur une facture. Les dupliquer aurait donné un quatrième cas de
// divergence après les trois générateurs d'impression et les deux
// FiltresVentes.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Search } from "lucide-react";
import { genererBonEchangeHTML } from "@/lib/genererPDF";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

// =====================================================================
//  Types partagés
// =====================================================================

export interface UniteVente {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

export interface ArticleAchat {
  id: string;
  nom: string;
  unite_base: string;
  stock: number;
  unites: UniteVente[];
}

export interface LigneVente {
  id: string;
  article_id: string;
  article_nom: string;
  unite_libelle: string;
  unite_vente_id: string;
  depot_source_id: string;
  quantite: number;
  prix_pratique: number;
  montant: number;
}

export interface Vente {
  id: string;
  numero_facture?: string;
  client_id: string;
  client_nom: string;
  client_code: string;
  date_vente: string;
  total: number;
  statut: string;
  lignes: LigneVente[];
}

export interface Avoir {
  id: string;
  client_nom: string;
  client_code: string;
  montant: number;
  cree_le: string;
}

export function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

export function formaterDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Modal : Remboursement simple
// =====================================================================

export function ModalRemboursement({
  ouvert, vente, ligne, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  vente: Vente | null;
  ligne: LigneVente | null;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const [quantiteRetour, setQuantiteRetour] = useState("1");
  const [modeEncaissement, setModeEncaissement] = useState("especes");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert && ligne) setQuantiteRetour(ligne.quantite.toString());
  }, [ouvert, ligne]);

  const quantiteNum = parseFloat(quantiteRetour) || 0;
  const montantCredit = ligne ? Math.round(ligne.prix_pratique * quantiteNum) : 0;

  async function handleConfirmer() {
    if (!vente || !ligne || quantiteNum <= 0) return;
    setChargement(true);
    try {
      await invoke("enregistrer_retour", {
        venteId: vente.id,
        ligneVenteId: ligne.id,
        quantite: quantiteNum,
        modeResolution: "remboursement",
        modeEncaissement,
        articleRemplacementId: null,
        uniteRemplacementId: null,
        quantiteRemplacement: null,
        modeReliquatPositif: null,
      });
      onConfirmer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  if (!vente || !ligne) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Remboursement — {ligne.article_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 text-xs text-muted-foreground">
            Vente du {formaterDate(vente.date_vente)} · {vente.client_nom}
          </div>

          <div>
            <Label>Quantité retournée ({ligne.unite_libelle}) *</Label>
            <Input type="number" value={quantiteRetour}
              onChange={e => setQuantiteRetour(e.target.value)}
              max={ligne.quantite} className="mt-1" autoFocus />
            <p className="text-xs text-muted-foreground mt-1">
              Vendu : {ligne.quantite} {ligne.unite_libelle}
            </p>
          </div>

          <div className="bg-muted rounded-md px-3 py-2">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Montant à rembourser</span>
              <span className="font-bold text-green-600">{formaterMontant(montantCredit)}</span>
            </div>
          </div>

          <div>
            <Label>Mode de remboursement</Label>
            <Select value={modeEncaissement} onValueChange={v => { if (v) setModeEncaissement(v); }}>
              <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="especes">Espèces</SelectItem>
                <SelectItem value="orange_money">Orange Money</SelectItem>
                <SelectItem value="moov_money">Moov Money</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-orange-500 mt-1">
              Une sortie de caisse sera enregistrée
            </p>
          </div>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} disabled={chargement} className="flex-1">
              Annuler
            </Button>
            <Button onClick={handleConfirmer}
              disabled={quantiteNum <= 0 || quantiteNum > ligne.quantite || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Rembourser"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Avoir conservé
// =====================================================================

export function ModalAvoirConserve({
  ouvert, vente, ligne, onFermer, onConfirmer,
}: {
  ouvert: boolean;
  vente: Vente | null;
  ligne: LigneVente | null;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const [quantiteRetour, setQuantiteRetour] = useState("1");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert && ligne) setQuantiteRetour(ligne.quantite.toString());
  }, [ouvert, ligne]);

  const quantiteNum = parseFloat(quantiteRetour) || 0;
  const montantCredit = ligne ? Math.round(ligne.prix_pratique * quantiteNum) : 0;

  async function handleConfirmer() {
    if (!vente || !ligne || quantiteNum <= 0) return;
    setChargement(true);
    try {
      const res = await invoke<{ numero_avoir: string | null }>(
        "enregistrer_retour", {
          venteId: vente.id,
          ligneVenteId: ligne.id,
          quantite: quantiteNum,
          modeResolution: "avoir_conserve",
          modeEncaissement: null,
          articleRemplacementId: null,
          uniteRemplacementId: null,
          quantiteRemplacement: null,
          modeReliquatPositif: null,
        });
      // L'avoir est desormais un document numerote (AVC-AAAA-NNNNN),
      // consultable et imprimable depuis l'ecran Pieces.
      if (res?.numero_avoir) {
        await message(
          `Avoir ${res.numero_avoir} créé — imprimable depuis Pièces.`,
          { title: "Avoir créé", kind: "info" },
        );
      }
      onConfirmer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  if (!vente || !ligne) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Avoir conservé — {ligne.article_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div className="bg-muted rounded-md px-3 py-2 text-xs text-muted-foreground">
            Vente du {formaterDate(vente.date_vente)} · {vente.client_nom}
          </div>

          <div>
            <Label>Quantité retournée ({ligne.unite_libelle}) *</Label>
            <Input type="number" value={quantiteRetour}
              onChange={e => setQuantiteRetour(e.target.value)}
              max={ligne.quantite} className="mt-1" autoFocus />
          </div>

          <div className="bg-muted rounded-md px-3 py-2">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Avoir créé au nom de</span>
              <span className="font-medium">{vente?.client_nom}</span>
            </div>
            <div className="flex justify-between text-sm mt-1">
              <span className="text-muted-foreground">Montant</span>
              <span className="font-bold text-green-600">{formaterMontant(montantCredit)}</span>
            </div>
          </div>

          <p className="text-xs text-muted-foreground">
            L'avoir sera utilisable lors d'une prochaine vente. Pas de mouvement de caisse.
          </p>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} disabled={chargement} className="flex-1">
              Annuler
            </Button>
            <Button onClick={handleConfirmer}
              disabled={quantiteNum <= 0 || quantiteNum > ligne.quantite || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer l'avoir"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Échange complet
// =====================================================================

export function ModalEchange({
  ouvert, vente, ligne, articles, comptant, bonSortieActif,
  onFermer, onConfirmer,
}: {
  ouvert: boolean;
  vente: Vente | null;
  ligne: LigneVente | null;
  articles: ArticleAchat[];
  // Client de passage : le reliquat repart en especes, jamais en avoir.
  comptant: boolean;
  // Magasin separe de la caisse : on imprime un bon pour le magasinier.
  bonSortieActif: boolean;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  // Article retourné
  const [quantiteRetour, setQuantiteRetour] = useState("1");

  // Article de remplacement
  const [rechercheRemplacement, setRechercheRemplacement] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<ArticleAchat[]>([]);
  const [articleRemplacement, setArticleRemplacement] = useState<ArticleAchat | null>(null);
  const [uniteRemplacement, setUniteRemplacement] = useState<UniteVente | null>(null);
  const [quantiteRemplacement, setQuantiteRemplacement] = useState("1");

  // Reliquat
  const [modeReliquatPositif, setModeReliquatPositif] = useState<"remboursement" | "avoir">("remboursement");
  // Garde-fou d'etat : un changement de vente ne doit pas laisser
  // "avoir" selectionne sur un client de passage.
  useEffect(() => {
    if (comptant) setModeReliquatPositif("remboursement");
  }, [comptant, ouvert]);
  const [modeEncaissementReliquat, setModeEncaissementReliquat] = useState("especes");
  const [modeEncaissementComplement, setModeEncaissementComplement] = useState("especes");

  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (ouvert && ligne) {
      setQuantiteRetour(ligne.quantite.toString());
      setArticleRemplacement(null);
      setUniteRemplacement(null);
      setQuantiteRemplacement("1");
      setRechercheRemplacement("");
      setArticlesFiltres([]);
    }
  }, [ouvert, ligne]);

  const quantiteRetourNum = parseFloat(quantiteRetour) || 0;
  const quantiteRemplacementNum = parseFloat(quantiteRemplacement) || 0;

  const creditRetour = ligne
    ? Math.round(ligne.prix_pratique * quantiteRetourNum)
    : 0;

  const montantRemplacement = uniteRemplacement
    ? Math.round(uniteRemplacement.prix_reference * quantiteRemplacementNum)
    : 0;

  const reliquat = creditRetour - montantRemplacement;

  function handleRechercheRemplacement(val: string) {
    setRechercheRemplacement(val);
    if (val.length < 1) { setArticlesFiltres([]); return; }
    // Le meme article reste proposable : echanger un carton contre les
    // pieces qu'il contient est le cas le plus courant. Il n'etait
    // exclu que parce qu'a l'epoque un article n'avait qu'une unite —
    // l'echanger contre lui-meme n'aurait alors rien voulu dire.
    setArticlesFiltres(
      articles.filter(a => a.nom.toLowerCase().includes(val.toLowerCase()))
    );
  }

  function selectionnerArticleRemplacement(a: ArticleAchat) {
    setArticleRemplacement(a);
    // Meme article : proposer d'emblee une AUTRE unite que celle
    // vendue, sinon l'echange serait sans objet.
    const memeArticle = a.id === ligne?.article_id;
    const autre = memeArticle
      ? a.unites.find(u => u.id !== ligne?.unite_vente_id)
      : undefined;
    setUniteRemplacement(autre ?? a.unites[0]);
    setQuantiteRemplacement("1");
    setRechercheRemplacement("");
    setArticlesFiltres([]);
  }

  async function handleConfirmer() {
    if (!vente || !ligne || !articleRemplacement || !uniteRemplacement) return;
    setChargement(true);
    try {
      await invoke("enregistrer_retour", {
        venteId: vente.id,
        ligneVenteId: ligne.id,
        quantite: quantiteRetourNum,
        modeResolution: "echange",
        modeEncaissement: reliquat < 0 ? modeEncaissementComplement : null,
        articleRemplacementId: articleRemplacement.id,
        uniteRemplacementId: uniteRemplacement.id,
        quantiteRemplacement: quantiteRemplacementNum,
        modeReliquatPositif: reliquat > 0 ? modeReliquatPositif : null,
        modeEncaissementReliquat: reliquat > 0 && modeReliquatPositif === "remboursement"
          ? modeEncaissementReliquat
          : null,
      });

      // Bon d'échange pour le magasinier : c'est le seul cas où il doit
      // faire deux gestes opposés, reprendre puis remettre. Non
      // bloquant — l'échange est déjà enregistré, une impression ratée
      // ne doit pas laisser croire qu'il a échoué.
      if (bonSortieActif) {
        try {
          const [societe, logo] = await Promise.all([
            invoke<any>("lire_parametres_societe"),
            invoke<string | null>("lire_logo_base64").catch(() => null),
          ]);
          await invoke("imprimer_facture", {
            html: genererBonEchangeHTML({
              numero_vente: vente.numero_facture ?? vente.id.slice(0, 8),
              date: new Date().toISOString(),
              client_nom: vente.client_nom,
              rendu: {
                article_nom: ligne.article_nom,
                unite_libelle: ligne.unite_libelle,
                quantite: quantiteRetourNum,
              },
              remis: {
                article_nom: articleRemplacement.nom,
                unite_libelle: uniteRemplacement.libelle,
                quantite: quantiteRemplacementNum,
              },
              societe,
            }, logo),
            nomFichier: `echange_${vente.id.slice(0, 8)}.html`,
          });
        } catch (e) {
          console.error("Bon d'échange non imprimé :", e);
        }
      }

      onConfirmer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  if (!vente || !ligne) return null;

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Échange — {ligne.article_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2 max-h-[70vh] overflow-auto pr-1">

          <div className="bg-muted rounded-md px-3 py-2 text-xs text-muted-foreground">
            Vente du {formaterDate(vente.date_vente)} · {vente.client_nom}
          </div>

          {/* Article retourné */}
          <div className="border border-border rounded-lg p-3 space-y-2">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Article retourné
            </p>
            <div className="flex items-center justify-between">
              <p className="text-sm font-medium">{ligne.article_nom}</p>
              <span className="text-xs text-muted-foreground">
                {formaterMontant(ligne.prix_pratique)}/{ligne.unite_libelle}
              </span>
            </div>
            <div>
              <Label className="text-xs">Quantité retournée ({ligne.unite_libelle})</Label>
              <Input type="number" value={quantiteRetour}
                onChange={e => setQuantiteRetour(e.target.value)}
                max={ligne.quantite} className="h-8 text-sm mt-0.5" />
            </div>
            <div className="flex justify-between text-sm pt-1">
              <span className="text-muted-foreground">Crédit généré</span>
              <span className="font-semibold text-green-600">{formaterMontant(creditRetour)}</span>
            </div>
          </div>

          {/* Article de remplacement */}
          <div className="border border-border rounded-lg p-3 space-y-2">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              Article de remplacement
            </p>

            {articleRemplacement ? (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <p className="text-sm font-medium">{articleRemplacement.nom}</p>
                  <button onClick={() => setArticleRemplacement(null)}
                    className="text-xs text-muted-foreground hover:text-foreground">✕</button>
                </div>

                {/* Toujours affiche, meme avec une seule unite : l'operateur
                    doit VOIR dans quel conditionnement il echange. Masquer
                    le champ faisait passer un carton pour une piece. */}
                {articleRemplacement.unites.length >= 1 && (
                  <Select value={uniteRemplacement?.id ?? ""}
                    onValueChange={v => {
                      if (v) {
                        const u = articleRemplacement.unites.find(u => u.id === v);
                        if (u) setUniteRemplacement(u);
                      }
                    }}>
                    <SelectTrigger className="h-8 text-sm">
                      <SelectValue>
                        {uniteRemplacement?.libelle ?? "Choisir"}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {articleRemplacement.unites.map(u => (
                        <SelectItem key={u.id} value={u.id}>
                          {u.libelle} — {formaterMontant(u.prix_reference)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}

                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <Label className="text-xs">Quantité</Label>
                    <Input type="number" value={quantiteRemplacement}
                      onChange={e => setQuantiteRemplacement(e.target.value)}
                      className="h-8 text-sm mt-0.5" />
                  </div>
                  <div className="flex items-end pb-0.5">
                    <p className="text-sm">
                      {uniteRemplacement && `${formaterMontant(uniteRemplacement.prix_reference)}/${uniteRemplacement.libelle}`}
                    </p>
                  </div>
                </div>

                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">Montant remplacement</span>
                  <span className="font-semibold">{formaterMontant(montantRemplacement)}</span>
                </div>
              </div>
            ) : (
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
                <Input value={rechercheRemplacement}
                  onChange={e => handleRechercheRemplacement(e.target.value)}
                  placeholder="Rechercher l'article de remplacement..."
                  className="pl-8 h-8 text-sm" autoFocus />
                {articlesFiltres.length > 0 && (
                  <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md">
                    {articlesFiltres.map(a => (
                      <button key={a.id} onClick={() => selectionnerArticleRemplacement(a)}
                        className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent transition-colors">
                        <div className="flex items-center justify-between">
                          <span className="font-medium">{a.nom}</span>
                          <span className="text-xs text-muted-foreground">
                            {a.unites[0].prix_reference} F/{a.unite_base}
                          </span>
                        </div>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Résumé du reliquat */}
          {articleRemplacement && (
            <div className={cn(
              "border rounded-lg p-3 space-y-2",
              reliquat > 0 ? "border-green-200 bg-green-50 dark:bg-green-950/20"
              : reliquat < 0 ? "border-orange-200 bg-orange-50 dark:bg-orange-950/20"
              : "border-border bg-muted"
            )}>
              <div className="flex justify-between text-sm font-semibold">
                <span>
                  {reliquat > 0 ? "Reliquat à restituer"
                  : reliquat < 0 ? "Complément à payer"
                  : "Échange soldé exactement"}
                </span>
                <span className={
                  reliquat > 0 ? "text-green-600"
                  : reliquat < 0 ? "text-orange-600"
                  : ""
                }>
                  {reliquat !== 0 ? formaterMontant(Math.abs(reliquat)) : "0 F"}
                </span>
              </div>

              {/* Reliquat positif — client reçoit de l'argent */}
              {reliquat > 0 && (
                <div className="space-y-2 pt-1 border-t border-green-200">
                  <p className="text-xs text-muted-foreground">
                    {comptant
                      ? "Reliquat remboursé — pas d'avoir pour un client comptant."
                      : "Comment restituer le reliquat ?"}
                  </p>
                  <div className={cn("flex gap-2", comptant && "hidden")}>
                    {[
                      { value: "remboursement", label: "Rembourser", icone: Wallet },
                      { value: "avoir", label: "Avoir", icone: Gift },
                    ].map(opt => {
                      const Icone = opt.icone;
                      return (
                        <button key={opt.value}
                          onClick={() => setModeReliquatPositif(opt.value as typeof modeReliquatPositif)}
                          className={cn(
                            "flex-1 flex items-center justify-center gap-2 py-1.5 rounded-md border text-xs font-medium transition-all",
                            modeReliquatPositif === opt.value
                              ? "border-primary bg-primary text-primary-foreground"
                              : "border-border hover:bg-muted"
                          )}>
                          <Icone className="h-3.5 w-3.5" />
                          {opt.label}
                        </button>
                      );
                    })}
                  </div>

                  {modeReliquatPositif === "remboursement" && (
                    <Select value={modeEncaissementReliquat}
                      onValueChange={v => { if (v) setModeEncaissementReliquat(v); }}>
                      <SelectTrigger className="h-7 text-xs"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="especes">Espèces</SelectItem>
                        <SelectItem value="orange_money">Orange Money</SelectItem>
                        <SelectItem value="moov_money">Moov Money</SelectItem>
                      </SelectContent>
                    </Select>
                  )}
                </div>
              )}

              {/* Reliquat négatif — client paie la différence */}
              {reliquat < 0 && (
                <div className="space-y-2 pt-1 border-t border-orange-200">
                  <p className="text-xs text-muted-foreground">
                    Le client complète {formaterMontant(Math.abs(reliquat))}
                  </p>
                  <Select value={modeEncaissementComplement}
                    onValueChange={v => { if (v) setModeEncaissementComplement(v); }}>
                    <SelectTrigger className="h-7 text-xs"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="especes">Espèces</SelectItem>
                      <SelectItem value="orange_money">Orange Money</SelectItem>
                      <SelectItem value="moov_money">Moov Money</SelectItem>
                      <SelectItem value="cheque">Chèque</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              )}
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} disabled={chargement} className="flex-1">
              Annuler
            </Button>
            <Button
              onClick={handleConfirmer}
              disabled={
                !articleRemplacement ||
                quantiteRetourNum <= 0 ||
                quantiteRemplacementNum <= 0 ||
                chargement
              }
              className="flex-1"
            >
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Confirmer l'échange"
              }
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}