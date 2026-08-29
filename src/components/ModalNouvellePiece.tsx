// components/ModalNouvellePiece.tsx
// Modal de création de pièce — client OU fournisseur selon prop `cote`

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, X, Plus, UserPlus, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem,
  SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface Article {
  id: string; nom: string; unite_base: string; stock: number;
  taux_tva_defaut?: number;
  unites: { id: string; libelle: string; facteur: number; prix_reference: number }[];
}
interface Tiers { id: string; nom: string; code?: string; telephone?: string; }
interface Ligne {
  article_id: string; unite_vente_id: string;
  article_nom: string; unite_libelle: string;
  quantite: number; prix_unitaire: number;
  remise_pct: number; taux_tva: number;
}

// =====================================================================
//  Constantes
// =====================================================================

const TYPES_CLIENT: Record<string, string> = {
  devis:           "Devis",
  proforma:        "Proforma",
  commande_client: "Commande client",
  bon_livraison:   "Bon de livraison",
  facture:         "Facture",
};

const TYPES_FOURNISSEUR: Record<string, string> = {
  bon_commande_fournisseur: "Bon de commande",
  bon_reception:            "Bon de réception",
  facture_fournisseur:      "Facture fournisseur",
};

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Props
// =====================================================================

interface ModalNouvellePieceProps {
  ouvert: boolean;
  cote: "client" | "fournisseur";
  onFermer: () => void;
  onCree: () => void;
  tiersIdInitial?: string; // pré-sélectionner un tiers
}

// =====================================================================
//  Composant
// =====================================================================

export function ModalNouvellePiece({
  ouvert, cote, onFermer, onCree, tiersIdInitial,
}: ModalNouvellePieceProps) {
  const typesDisponibles = cote === "client" ? TYPES_CLIENT : TYPES_FOURNISSEUR;
  const defaultType = cote === "client" ? "devis" : "bon_commande_fournisseur";

  const [typePiece, setTypePiece] = useState(defaultType);
  const [tiersId, setTiersId] = useState(tiersIdInitial ?? "");
  const [tiersNom, setTiersNom] = useState("");
  const [rechercheTiers, setRechercheTiers] = useState("");
  const [tiersFiltres, setTiersFiltres] = useState<Tiers[]>([]);
  const [tousTiers, setTousTiers] = useState<Tiers[]>([]);
  const [articles, setArticles] = useState<Article[]>([]);
  const [lignes, setLignes] = useState<Ligne[]>([]);
  const [remiseGlobale, setRemiseGlobale] = useState("0");
  const [dateEcheance, setDateEcheance] = useState("");
  const [note, setNote] = useState("");
  const [rechercheArticle, setRechercheArticle] = useState("");
  const [articlesFiltres, setArticlesFiltres] = useState<Article[]>([]);
  const [chargement, setChargement] = useState(false);

  // Création rapide tiers
  const [creerTiersEnCours, setCreerTiersEnCours] = useState(false);
  const [nomNouveauTiers, setNomNouveauTiers] = useState("");
  const [telNouveauTiers, setTelNouveauTiers] = useState("");

  // Création rapide article
  const [creerArticleEnCours, setCreerArticleEnCours] = useState(false);
  const [nomNouvelArticle, setNomNouvelArticle] = useState("");
  const [prixNouvelArticle, setPrixNouvelArticle] = useState("");

  useEffect(() => {
    if (!ouvert) return;
    setTypePiece(defaultType);
    setLignes([]); setRemiseGlobale("0");
    setDateEcheance(""); setNote("");
    setRechercheArticle(""); setArticlesFiltres([]);
    if (!tiersIdInitial) { setTiersId(""); setTiersNom(""); }

    const role = UTILISATEUR_ACTIF?.role ?? "employe";
    const promiseTiers = cote === "client"
      ? invoke<Tiers[]>("lire_clients")
      : invoke<Tiers[]>("lire_fournisseurs");

    Promise.all([
      promiseTiers,
      invoke<Article[]>("lire_articles_avec_unites", { role }),
    ]).then(([tiers, arts]) => {
      setTousTiers(tiers);
      setArticles(arts);
      if (tiersIdInitial) {
        const t = tiers.find(t => t.id === tiersIdInitial);
        if (t) setTiersNom(t.nom);
      }
    }).catch(console.error);
  }, [ouvert, cote]);

  function handleFermer() {
    setLignes([]); setRemiseGlobale("0");
    setDateEcheance(""); setNote("");
    if (!tiersIdInitial) { setTiersId(""); setTiersNom(""); }
    onFermer();
  }

  async function handleCreerTiers() {
    if (!nomNouveauTiers.trim()) return;
    setCreerTiersEnCours(true);
    try {
      const cmd = cote === "client" ? "creer_client_rapide" : "creer_fournisseur";
      const res = await invoke<any>(cmd, {
        nom: nomNouveauTiers.trim(),
        telephone: telNouveauTiers.trim() || null,
      });
      const id = typeof res === "string" ? res : (res.id ?? res);
      setTiersId(id);
      setTiersNom(nomNouveauTiers.trim());
      setTousTiers(prev => [...prev, {
        id, nom: nomNouveauTiers.trim(),
        telephone: telNouveauTiers.trim() || undefined
      }]);
      setRechercheTiers("");
      setTiersFiltres([]);
      setNomNouveauTiers("");
      setTelNouveauTiers("");
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setCreerTiersEnCours(false); }
  }

  async function handleCreerArticle() {
    if (!nomNouvelArticle.trim()) return;
    setCreerArticleEnCours(true);
    try {
      // creer_article_rapide renvoie l'article COMPLET :
      //   { id, nom, unite_base, stock, unites: [{ id, libelle, ... }] }
      //
      // Le code precedent cherchait `res.unite_id`, qui n'existe pas, et
      // retombait sur `artId`. L'id de l'ARTICLE partait donc comme
      // unite_vente_id dans la ligne de piece -> violation de cle
      // etrangere sur unite_vente(id).
      const res = await invoke<{
        id: string; nom: string; unite_base: string; stock: number;
        unites: { id: string; libelle: string; facteur: number;
                  prix_reference: number }[];
      }>("creer_article_rapide", {
        nom: nomNouvelArticle.trim(),
        uniteBase: "Unité",
        prixReference: parseInt(prixNouvelArticle) || 0,
      });

      if (!res?.unites?.[0]?.id) {
        throw new Error(
          "Article créé mais son unité de vente est introuvable. " +
          "Ne pas l'ajouter à la pièce."
        );
      }

      const nouvelArt: Article = {
        id: res.id,
        nom: res.nom,
        unite_base: res.unite_base,
        stock: res.stock ?? 0,
        taux_tva_defaut: 0,
        unites: res.unites,
      };
      setArticles(prev => [...prev, nouvelArt]);
      ajouterLigne(nouvelArt);
      setNomNouvelArticle("");
      setPrixNouvelArticle("");
      setRechercheArticle("");
      setArticlesFiltres([]);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setCreerArticleEnCours(false); }
  }

  function ajouterLigne(article: Article) {
    const unite = article.unites[0];
    setLignes(prev => [...prev, {
      article_id: article.id, unite_vente_id: unite.id,
      article_nom: article.nom, unite_libelle: unite.libelle,
      quantite: 1, prix_unitaire: unite.prix_reference,
      remise_pct: 0, taux_tva: article.taux_tva_defaut ?? 0,
    }]);
    setRechercheArticle(""); setArticlesFiltres([]);
  }

  function modif(i: number, champ: string, val: any) {
    setLignes(prev => prev.map((l, idx) => idx === i ? { ...l, [champ]: val } : l));
  }

  const totalHT = lignes.reduce((s, l) => {
    const brut = Math.round(l.prix_unitaire * l.quantite);
    return s + brut - Math.round(brut * l.remise_pct / 100);
  }, 0);
  const remiseMt = Math.round(totalHT * (parseFloat(remiseGlobale) || 0) / 100);
  const totalNet = totalHT - remiseMt;

  async function handleCreer() {
    if (!tiersId || lignes.length === 0) return;
    setChargement(true);
    try {
      const commande = cote === "client" ? "creer_piece" : "creer_piece_fournisseur";
      await invoke(commande, {
        ...(cote === "client" ? { clientId: tiersId } : { fournisseurId: tiersId }),
        typePiece,
        lignes: lignes.map(l => ({
          article_id: l.article_id,
          unite_vente_id: l.unite_vente_id,
          quantite: l.quantite,
          prix_unitaire: l.prix_unitaire,
          remise_pct: l.remise_pct,
          taux_tva: l.taux_tva,
        })),
        remiseGlobale: parseFloat(remiseGlobale) || 0,
        dateEcheance: dateEcheance || null,
        note: note || null,
        pieceOrigineId: null,
      });
      handleFermer();
      onCree();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally { setChargement(false); }
  }

  const labelTiers = cote === "client" ? "Client" : "Fournisseur";

  return (
    <Dialog open={ouvert} onOpenChange={handleFermer}>
      <DialogContent className="flex flex-col p-0 gap-0" style={{width:"96vw",maxWidth:"96vw",height:"92vh",maxHeight:"92vh"}}>
        <DialogHeader className="px-6 py-4 border-b border-border shrink-0">
          <DialogTitle>
            Nouvelle pièce {cote === "client" ? "client" : "fournisseur"}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-auto px-6 py-4 space-y-4">

          {/* Ligne 1 — Type + Tiers + Date + Note */}
          <div className="grid grid-cols-4 gap-3">

            {/* Type */}
            <div>
              <Label className="text-xs">Type de pièce</Label>
              <Select value={typePiece} onValueChange={v => { if (v) setTypePiece(v); }}>
                <SelectTrigger className="mt-1 h-9"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {Object.entries(typesDisponibles).map(([v, l]) => (
                    <SelectItem key={v} value={v}>{l}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Tiers */}
            <div>
              <Label className="text-xs">{labelTiers} *</Label>
              {tiersIdInitial ? (
                <div className="mt-1 h-9 px-3 flex items-center border border-border
                                rounded-md text-sm bg-muted/40">
                  {tiersNom}
                </div>
              ) : (
                <div className="relative mt-1">
                  <Input
                    value={tiersId ? tiersNom : rechercheTiers}
                    onChange={e => {
                      if (tiersId) { setTiersId(""); setTiersNom(""); }
                      setRechercheTiers(e.target.value);
                      setTiersFiltres(
                        e.target.value.length < 1 ? [] :
                        tousTiers.filter(t =>
                          t.nom.toLowerCase().includes(e.target.value.toLowerCase())
                        ).slice(0, 8)
                      );
                    }}
                    placeholder={`Rechercher ${labelTiers.toLowerCase()}...`}
                    className="h-9 pr-7" />
                  {tiersId && (
                    <button onClick={() => { setTiersId(""); setTiersNom(""); }}
                      className="absolute right-2 top-2.5 text-muted-foreground hover:text-foreground">
                      <X className="h-3.5 w-3.5" />
                    </button>
                  )}
                  {(tiersFiltres.length > 0 || (rechercheTiers.length >= 2 && !tiersId)) && (
                    <div className="absolute z-20 w-full mt-1 bg-card border border-border
                                    rounded-md shadow-lg max-h-52 overflow-auto">
                      {tiersFiltres.map(t => (
                        <button key={t.id}
                          onClick={() => {
                            setTiersId(t.id);
                            setTiersNom(t.nom);
                            setRechercheTiers("");
                            setTiersFiltres([]);
                          }}
                          className="w-full text-left px-3 py-2 text-sm hover:bg-accent
                                     flex justify-between border-b border-border/50 last:border-0">
                          <span className="font-medium">{t.nom}</span>
                          {t.code && (
                            <span className="text-muted-foreground text-xs">{t.code}</span>
                          )}
                        </button>
                      ))}
                      {/* Création rapide si aucun résultat */}
                      {tiersFiltres.length === 0 && rechercheTiers.length >= 2 && (
                        <div className="p-3 space-y-2 border-t border-border">
                          <p className="text-xs text-muted-foreground">
                            Aucun {cote === "client" ? "client" : "fournisseur"} trouvé.
                            Créer "{rechercheTiers}" ?
                          </p>
                          <div className="flex gap-2">
                            <Input
                              value={nomNouveauTiers || rechercheTiers}
                              onChange={e => setNomNouveauTiers(e.target.value)}
                              placeholder="Nom"
                              className="h-7 text-xs flex-1"
                            />
                            <Input
                              value={telNouveauTiers}
                              onChange={e => setTelNouveauTiers(e.target.value)}
                              placeholder="Téléphone"
                              className="h-7 text-xs w-28"
                            />
                            <Button size="sm" className="h-7 text-xs px-2 gap-1"
                              onClick={() => {
                                if (!nomNouveauTiers) setNomNouveauTiers(rechercheTiers);
                                handleCreerTiers();
                              }}
                              disabled={creerTiersEnCours}>
                              {creerTiersEnCours
                                ? <Loader2 className="h-3 w-3 animate-spin" />
                                : <><UserPlus className="h-3 w-3" /> Créer</>}
                            </Button>
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Date échéance */}
            <div>
              <Label className="text-xs">Échéance (optionnel)</Label>
              <Input type="date" value={dateEcheance}
                onChange={e => setDateEcheance(e.target.value)}
                className="mt-1 h-9" />
            </div>

            {/* Note */}
            <div>
              <Label className="text-xs">Note</Label>
              <Input value={note} onChange={e => setNote(e.target.value)}
                placeholder="Conditions, délais..." className="mt-1 h-9" />
            </div>
          </div>

          {/* Recherche article */}
          <div className="relative">
            <Input value={rechercheArticle}
              onChange={e => {
                setRechercheArticle(e.target.value);
                setArticlesFiltres(
                  e.target.value.length < 1 ? [] :
                  articles.filter(a =>
                    a.nom.toLowerCase().includes(e.target.value.toLowerCase())
                  ).slice(0, 10)
                );
              }}
              placeholder="🔍 Rechercher un article à ajouter..."
              className="h-9" />
            {(articlesFiltres.length > 0 || (rechercheArticle.length >= 2)) && (
              <div className="absolute z-20 w-full mt-1 bg-card border border-border
                              rounded-md shadow-lg max-h-64 overflow-auto">
                {articlesFiltres.map(a => (
                  <button key={a.id} onClick={() => ajouterLigne(a)}
                    className="w-full text-left px-4 py-2.5 text-sm hover:bg-accent
                               flex items-center justify-between
                               border-b border-border/50 last:border-0">
                    <span className="font-medium">{a.nom}</span>
                    <span className="text-xs text-muted-foreground shrink-0">
                      {fmt(a.unites[0].prix_reference)} / {a.unite_base}
                      {a.stock >= 0 && (
                        <span className={`ml-3 ${a.stock <= 0 ? "text-red-500" : "text-green-600"}`}>
                          Stock : {a.stock % 1 === 0 ? a.stock : a.stock.toFixed(2)} {a.unite_base}
                        </span>
                      )}
                    </span>
                  </button>
                ))}
                {/* Création rapide article */}
                {articlesFiltres.length === 0 && rechercheArticle.length >= 2 && (
                  <div className="p-3 space-y-2 border-t border-border bg-muted/20">
                    <p className="text-xs text-muted-foreground font-medium">
                      Article "{rechercheArticle}" introuvable — créer ?
                    </p>
                    <div className="flex gap-2">
                      <Input
                        value={nomNouvelArticle || rechercheArticle}
                        onChange={e => setNomNouvelArticle(e.target.value)}
                        placeholder="Nom article"
                        className="h-7 text-xs flex-1"
                      />
                      <div className="relative w-28">
                        <Input
                          type="number"
                          value={prixNouvelArticle}
                          onChange={e => setPrixNouvelArticle(e.target.value)}
                          placeholder="Prix"
                          className="h-7 text-xs pr-5"
                        />
                        <span className="absolute right-1.5 top-1.5 text-[10px] text-muted-foreground">F</span>
                      </div>
                      <Button size="sm" className="h-7 text-xs px-2 gap-1"
                        onClick={() => {
                          if (!nomNouvelArticle) setNomNouvelArticle(rechercheArticle);
                          handleCreerArticle();
                        }}
                        disabled={creerArticleEnCours}>
                        {creerArticleEnCours
                          ? <Loader2 className="h-3 w-3 animate-spin" />
                          : <><Package className="h-3 w-3" /> Créer</>}
                      </Button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Tableau lignes */}
          {lignes.length === 0 ? (
            <div className="border-2 border-dashed border-border rounded-lg py-12
                            text-center text-muted-foreground">
              <p className="text-sm">Recherchez et ajoutez des articles ci-dessus</p>
            </div>
          ) : (
            <div className="border border-border rounded-md overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-muted/80 text-xs">
                  <tr>
                    <th className="text-left px-4 py-2.5 w-[30%]">Article</th>
                    <th className="text-center px-2 py-2.5 w-[10%]">Unité</th>
                    <th className="text-right px-2 py-2.5 w-[10%]">Qté</th>
                    <th className="text-right px-2 py-2.5 w-[16%]">Prix (F)</th>
                    <th className="text-right px-2 py-2.5 w-[8%]">Remise %</th>
                    <th className="text-right px-2 py-2.5 w-[8%]">TVA %</th>
                    <th className="text-right px-2 py-2.5 w-[16%]">Montant (F)</th>
                    <th className="px-2 py-2.5 w-[6%]"></th>
                  </tr>
                </thead>
                <tbody>
                  {lignes.map((l, i) => {
                    const brut = Math.round(l.prix_unitaire * l.quantite);
                    const remise = Math.round(brut * l.remise_pct / 100);
                    const montant = brut - remise;
                    return (
                      <tr key={i} className="border-t border-border hover:bg-muted/20">
                        <td className="px-4 py-2 font-medium">{l.article_nom}</td>
                        <td className="px-2 py-2 text-center text-xs text-muted-foreground">
                          {l.unite_libelle}
                        </td>
                        <td className="px-2 py-2 text-right">
                          <input type="number" min="0.01" step="0.01" value={l.quantite}
                            onChange={e => modif(i, "quantite", parseFloat(e.target.value) || 1)}
                            className="w-20 h-8 text-right text-sm border border-border
                                       rounded px-2 bg-background focus:outline-none
                                       focus:ring-1 focus:ring-primary" />
                        </td>
                        <td className="px-2 py-2 text-right">
                          <input type="number" min="0" value={l.prix_unitaire}
                            onChange={e => modif(i, "prix_unitaire", parseInt(e.target.value) || 0)}
                            className="w-28 h-8 text-right text-sm border border-border
                                       rounded px-2 bg-background focus:outline-none
                                       focus:ring-1 focus:ring-primary" />
                        </td>
                        <td className="px-2 py-2 text-right">
                          <div className="relative inline-block">
                            <input type="number" min="0" max="100" value={l.remise_pct}
                              onChange={e => modif(i, "remise_pct", parseFloat(e.target.value) || 0)}
                              className="w-16 h-8 text-right text-sm border border-border
                                         rounded pl-2 pr-5 bg-background focus:outline-none
                                         focus:ring-1 focus:ring-primary" />
                            <span className="absolute right-1.5 top-2 text-xs text-muted-foreground">%</span>
                          </div>
                        </td>
                        <td className="px-2 py-2 text-right">
                          <div className="relative inline-block">
                            <input type="number" min="0" max="100" step="1"
                              value={(l.taux_tva * 100).toFixed(0)}
                              onChange={e => modif(i, "taux_tva", (parseFloat(e.target.value) || 0) / 100)}
                              className="w-14 h-8 text-right text-sm border border-border
                                         rounded pl-2 pr-4 bg-background focus:outline-none
                                         focus:ring-1 focus:ring-primary" />
                            <span className="absolute right-1 top-2 text-xs text-muted-foreground">%</span>
                          </div>
                        </td>
                        <td className="px-2 py-2 text-right">
                          <p className="font-semibold">{fmt(montant)}</p>
                          {remise > 0 && (
                            <p className="text-xs text-orange-500">−{fmt(remise)}</p>
                          )}
                        </td>
                        <td className="px-2 py-2 text-center">
                          <button
                            onClick={() => setLignes(prev => prev.filter((_, idx) => idx !== i))}
                            className="w-6 h-6 rounded-full flex items-center justify-center
                                       text-muted-foreground hover:bg-destructive/10
                                       hover:text-destructive transition-colors mx-auto">
                            ✕
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          {/* Remise globale + totaux */}
          {lignes.length > 0 && (
            <div className="flex items-center justify-between pt-2">
              <div className="flex items-center gap-3">
                <Label className="text-sm shrink-0">Remise globale</Label>
                <div className="relative w-24">
                  <Input type="number" min="0" max="100" step="1"
                    value={remiseGlobale}
                    onChange={e => setRemiseGlobale(e.target.value)}
                    className="h-8 pr-6 text-sm" />
                  <span className="absolute right-2.5 top-2 text-xs text-muted-foreground">%</span>
                </div>
              </div>
              <div className="text-right space-y-1">
                {remiseMt > 0 && (
                  <>
                    <p className="text-sm text-muted-foreground">Brut : {fmt(totalHT)}</p>
                    <p className="text-sm text-orange-600">Remise : −{fmt(remiseMt)}</p>
                  </>
                )}
                <p className="text-xl font-bold">Net : {fmt(totalNet)}</p>
              </div>
            </div>
          )}
        </div>

        {/* Pied fixe */}
        <div className="border-t border-border px-6 py-4 flex gap-3 shrink-0 bg-card">
          <Button variant="outline" onClick={handleFermer} className="flex-1">Annuler</Button>
          <Button onClick={handleCreer}
            disabled={!tiersId || lignes.length === 0 || chargement}
            className="flex-1">
            {chargement
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : `Créer ${typesDisponibles[typePiece] ?? typePiece}`
            }
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}