// components/ModalsRetour.tsx
//
// La fenêtre de retour, commune à l'écran Retours et à l'écran Pièces.
//
// Tous les articles de la vente s'affichent, chacun avec sa case et sa
// quantité (ce qui reste à retourner), puis on choisit : rembourser,
// avoir ou échange. UN geste, une commande (`enregistrer_retour_groupe`),
// tout ou rien. Elle remplace « Quel article retourner ? » suivi d'une
// fenêtre par article et par mode — il fallait cliquer article par
// article, échanges compris.

import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { ArrowLeftRight, Gift, Loader2, Search, Wallet, X } from "lucide-react";
import { genererBonEchangeHTML } from "@/lib/genererPDF";
import { chargerHabillage } from "@/lib/impression";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
  /** Ce qui en est déjà revenu (tous retours confondus). */
  deja_retourne?: number;
}

export interface Vente {
  id: string;
  numero_facture?: string;
  client_id: string;
  client_nom: string;
  client_code: string;
  /** Le « Comptant » ou une fiche de passage : pas d'avoir (D40). */
  client_generique?: boolean;
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

export type ModeRetour = "remboursement" | "avoir" | "echange";

export function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

export function formaterDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

/** Ce qui reste à retourner sur une ligne, arrondi au centième. */
export function restantDe(l: LigneVente): number {
  return Math.max(0, Math.round((l.quantite - (l.deja_retourne ?? 0)) * 100) / 100);
}

const MOYENS = [
  { value: "especes", label: "Espèces" },
  { value: "orange_money", label: "Orange Money" },
  { value: "moov_money", label: "Moov Money" },
];

function ChoixMoyen({ valeur, onChange, cheque = false }: {
  valeur: string; onChange: (v: string) => void; cheque?: boolean;
}) {
  const liste = cheque ? [...MOYENS, { value: "cheque", label: "Chèque" }] : MOYENS;
  return (
    <Select value={valeur} onValueChange={v => { if (v) onChange(v); }}>
      <SelectTrigger className="h-8 text-xs w-44">
        <SelectValue>{liste.find(m => m.value === valeur)?.label}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        {liste.map(m => <SelectItem key={m.value} value={m.value}>{m.label}</SelectItem>)}
      </SelectContent>
    </Select>
  );
}

interface Pris { article: ArticleAchat; unite: UniteVente; quantite: string }

interface ResultatRetour {
  montant_credit: number; part_creance: number; part_client: number;
  part_especes: number; part_avoir: number; reliquat: number;
  numero_avoir: string | null;
}

// =====================================================================
//  La fenêtre de retour
// =====================================================================

export function ModalRetour({
  ouvert, vente, modeInitial, ligneInitiale, articles, bonSortieActif,
  onFermer, onConfirmer,
}: {
  ouvert: boolean;
  vente: Vente | null;
  modeInitial: ModeRetour;
  /** Cochée d'office (le bouton d'une ligne dans l'écran Retours). */
  ligneInitiale?: string | null;
  articles: ArticleAchat[];
  /** Magasin séparé de la caisse : un échange imprime un bon d'échange. */
  bonSortieActif: boolean;
  onFermer: () => void;
  onConfirmer: () => void;
}) {
  const comptant = !!vente?.client_generique;
  const [mode, setMode] = useState<ModeRetour>("remboursement");
  const [choix, setChoix] = useState<Record<string, { coche: boolean; quantite: string }>>({});
  const [moyen, setMoyen] = useState("especes");
  const [pris, setPris] = useState<Pris[]>([]);
  const [recherche, setRecherche] = useState("");
  const [modeReliquat, setModeReliquat] = useState<"remboursement" | "avoir">("remboursement");
  const [moyenReliquat, setMoyenReliquat] = useState("especes");
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (!ouvert || !vente) return;
    const ouvertes = vente.lignes.filter(l => restantDe(l) > 0);
    setChoix(Object.fromEntries(vente.lignes.map(l => [l.id, {
      // Une seule ligne encore retournable : inutile de la faire cocher.
      coche: restantDe(l) > 0 && (l.id === ligneInitiale || ouvertes.length === 1),
      quantite: String(restantDe(l)),
    }])));
    setMode(comptant && modeInitial === "avoir" ? "remboursement" : modeInitial);
    setMoyen("especes");
    setPris([]);
    setRecherche("");
    setModeReliquat("remboursement");
    setMoyenReliquat("especes");
  }, [ouvert, vente, modeInitial, ligneInitiale, comptant]);

  if (!vente) return null;
  const v = vente;

  const qte = (l: LigneVente) => parseFloat(choix[l.id]?.quantite ?? "") || 0;
  const coches = v.lignes.filter(l => choix[l.id]?.coche);
  const credit = coches.reduce((s, l) => s + Math.round(l.prix_pratique * qte(l)), 0);
  const montantPris = pris.reduce((s, p) => s + Math.round(p.unite.prix_reference * (parseFloat(p.quantite) || 0)), 0);
  const reliquat = credit - montantPris;
  const ligneInvalide = (l: LigneVente) => qte(l) <= 0 || qte(l) > restantDe(l) + 1e-9;
  const invalide = coches.length === 0 || coches.some(ligneInvalide)
    || (mode === "echange" && (pris.length === 0 || pris.some(p => (parseFloat(p.quantite) || 0) <= 0)));

  function basculer(l: LigneVente, coche: boolean) {
    setChoix(c => ({ ...c, [l.id]: { ...c[l.id], coche } }));
  }
  function toutCocher() {
    setChoix(c => Object.fromEntries(v.lignes.map(l => [l.id, {
      ...c[l.id], coche: restantDe(l) > 0,
    }])));
  }
  function ajouterPris(a: ArticleAchat) {
    if (a.unites.length === 0) return;
    setPris(ps => [...ps, { article: a, unite: a.unites[0], quantite: "1" }]);
    setRecherche("");
  }

  async function confirmer() {
    setChargement(true);
    try {
      const r = await invoke<ResultatRetour>("enregistrer_retour_groupe", {
        venteId: v.id,
        articles: coches.map(l => ({ ligne_vente_id: l.id, quantite: qte(l) })),
        modeResolution: mode === "avoir" ? "avoir_conserve" : mode,
        // Remboursement : le moyen rendu ; échange : celui du complément.
        modeEncaissement: mode === "avoir" ? null : moyen,
        pris: mode === "echange"
          ? pris.map(p => ({ article_id: p.article.id, unite_vente_id: p.unite.id, quantite: parseFloat(p.quantite) || 0 }))
          : [],
        modeReliquatPositif: mode === "echange" && reliquat > 0 ? (comptant ? "remboursement" : modeReliquat) : null,
        modeEncaissementReliquat: mode === "echange" && reliquat > 0 ? moyenReliquat : null,
      });

      // Ce qui s'est passé, chiffré par le serveur : c'est lui qui sait
      // ce que le client avait versé et ce qu'il devait encore.
      const dire: string[] = [];
      if (r.part_creance > 0) dire.push(`Dette du client réduite de ${formaterMontant(r.part_creance)}.`);
      if (mode === "remboursement") {
        if (r.part_especes > 0) dire.push(`À rendre au client : ${formaterMontant(r.part_especes)}.`);
        if (r.part_avoir > 0) {
          dire.push(`${formaterMontant(r.part_avoir)} payés avec un avoir reviennent en avoir ${r.numero_avoir ?? ""}, pas en espèces.`);
        }
      } else if (mode === "avoir") {
        if (r.part_client > 0) dire.push(`Avoir ${r.numero_avoir ?? ""} : ${formaterMontant(r.part_client)} au crédit du client.`);
      } else if (r.reliquat > 0) {
        dire.push(r.numero_avoir
          ? `Reliquat de ${formaterMontant(r.reliquat)} : en avoir ${r.numero_avoir} (tout ou partie).`
          : `À rendre au client : ${formaterMontant(r.reliquat)}.`);
      } else if (r.reliquat < 0) {
        dire.push(`Le client paie la différence : ${formaterMontant(-r.reliquat)}.`);
      } else {
        dire.push("Échange soldé exactement.");
      }

      // Bon d'échange pour le magasinier : deux gestes opposés, reprendre
      // puis remettre. Non bloquant — le retour est déjà enregistré.
      if (mode === "echange" && bonSortieActif) {
        try {
          const [societe, habillage] = await Promise.all([
            invoke<any>("lire_parametres_societe"),
            chargerHabillage("bon_livraison"),
          ]);
          await invoke("imprimer_facture", {
            html: genererBonEchangeHTML({
              numero_vente: v.numero_facture ?? v.id.slice(0, 8),
              date: new Date().toISOString(),
              // Jamais « Client de passage » sur le papier : son code
              // (même règle que passage::tiers_a_imprimer côté serveur).
              client_nom: v.client_nom === "Client de passage" ? v.client_code : v.client_nom,
              rendu: coches.map(l => ({ article_nom: l.article_nom, unite_libelle: l.unite_libelle, quantite: qte(l) })),
              remis: pris.map(p => ({ article_nom: p.article.nom, unite_libelle: p.unite.libelle, quantite: parseFloat(p.quantite) || 0 })),
              societe,
            }, habillage),
            nomFichier: `echange_${v.id.slice(0, 8)}.html`,
          });
        } catch (e) {
          console.error("Bon d'échange non imprimé :", e);
        }
      }

      await message(dire.join("\n") || "Retour enregistré.", { title: "Retour enregistré", kind: "info" });
      onConfirmer();
    } catch (e) {
      await message(`${e}`, { title: "Retour refusé", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  const MODES: { value: ModeRetour; label: string; icone: typeof Wallet }[] = [
    { value: "remboursement", label: "Rembourser", icone: Wallet },
    ...(comptant ? [] : [{ value: "avoir" as const, label: "Avoir", icone: Gift }]),
    { value: "echange", label: "Échange", icone: ArrowLeftRight },
  ];
  const articlesTrouves = recherche.trim().length < 1 ? [] :
    articles.filter(a => a.nom.toLowerCase().includes(recherche.toLowerCase())).slice(0, 8);

  return (
    <Dialog open={ouvert} onOpenChange={o => { if (!o) onFermer(); }}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Retour — {v.client_nom}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-1 max-h-[72vh] overflow-auto pr-1">
          <p className="text-xs text-muted-foreground">
            Vente du {formaterDate(v.date_vente)}
            {v.numero_facture ? ` · ${v.numero_facture}` : ""} · {formaterMontant(v.total)}
          </p>

          {/* Le mode */}
          <div className="flex gap-2">
            {MODES.map(m => {
              const Icone = m.icone;
              return (
                <button key={m.value} type="button" onClick={() => setMode(m.value)}
                  className={cn(
                    "flex-1 flex items-center justify-center gap-2 py-2 rounded-md border text-sm font-medium transition-colors",
                    mode === m.value ? "border-primary bg-primary text-primary-foreground" : "border-border hover:bg-muted",
                  )}>
                  <Icone className="h-4 w-4" />{m.label}
                </button>
              );
            })}
          </div>
          {comptant && (
            <p className="text-xs text-muted-foreground -mt-2">
              Client de passage : pas d'avoir, on rembourse ou on échange.
            </p>
          )}

          {/* Les articles de la vente */}
          <div className="border border-border rounded-lg">
            <div className="flex items-center justify-between px-3 py-2 border-b border-border">
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                {mode === "echange" ? "Ce que le client rapporte" : "Articles retournés"}
              </p>
              <button type="button" onClick={toutCocher} className="text-xs text-primary hover:underline">
                Tout cocher
              </button>
            </div>
            {v.lignes.map(l => {
              const reste = restantDe(l);
              const c = choix[l.id];
              const fini = reste <= 0;
              return (
                <label key={l.id}
                  className={cn("flex items-center gap-3 px-3 py-2 border-b border-border/60 last:border-0",
                    fini ? "opacity-50" : "cursor-pointer hover:bg-muted/40")}>
                  <input type="checkbox" className="h-4 w-4 accent-primary"
                    disabled={fini} checked={!!c?.coche}
                    onChange={e => basculer(l, e.target.checked)} />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium truncate">{l.article_nom}</p>
                    <p className="text-xs text-muted-foreground">
                      Vendu {l.quantite} {l.unite_libelle} × {formaterMontant(l.prix_pratique)}
                      {(l.deja_retourne ?? 0) > 0 && (fini
                        ? " · tout est déjà revenu"
                        : ` · déjà revenu ${l.deja_retourne}`)}
                    </p>
                  </div>
                  {!fini && (
                    <Input type="number" min={0} max={reste} step="any"
                      value={c?.quantite ?? ""} disabled={!c?.coche}
                      onChange={e => setChoix(x => ({ ...x, [l.id]: { ...x[l.id], quantite: e.target.value } }))}
                      className={cn("h-8 w-20 text-sm text-right",
                        c?.coche && ligneInvalide(l) && "border-red-500")} />
                  )}
                  <span className="w-24 text-right text-sm tabular-nums">
                    {c?.coche ? formaterMontant(Math.round(l.prix_pratique * qte(l))) : ""}
                  </span>
                </label>
              );
            })}
          </div>

          {/* Échange : ce que le client prend */}
          {mode === "echange" && (
            <div className="border border-border rounded-lg p-3 space-y-2">
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                Ce que le client prend
              </p>
              {pris.map((p, i) => (
                <div key={i} className="flex items-center gap-2">
                  <p className="flex-1 text-sm font-medium truncate">{p.article.nom}</p>
                  <Select value={p.unite.id} onValueChange={id => {
                    const u = p.article.unites.find(x => x.id === id);
                    if (u) setPris(ps => ps.map((q, j) => j === i ? { ...q, unite: u } : q));
                  }}>
                    <SelectTrigger className="h-8 text-xs w-36"><SelectValue>{p.unite.libelle}</SelectValue></SelectTrigger>
                    <SelectContent>
                      {p.article.unites.map(u => (
                        <SelectItem key={u.id} value={u.id}>{u.libelle} — {formaterMontant(u.prix_reference)}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Input type="number" min={0} step="any" value={p.quantite}
                    onChange={e => setPris(ps => ps.map((q, j) => j === i ? { ...q, quantite: e.target.value } : q))}
                    className="h-8 w-20 text-sm text-right" />
                  <span className="w-24 text-right text-sm tabular-nums">
                    {formaterMontant(Math.round(p.unite.prix_reference * (parseFloat(p.quantite) || 0)))}
                  </span>
                  <button type="button" onClick={() => setPris(ps => ps.filter((_, j) => j !== i))}
                    className="text-muted-foreground hover:text-foreground" title="Retirer">
                    <X className="h-4 w-4" />
                  </button>
                </div>
              ))}
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
                <Input value={recherche} onChange={e => setRecherche(e.target.value)}
                  placeholder="Ajouter un article pris en échange…" className="pl-8 h-8 text-sm" />
                {articlesTrouves.length > 0 && (
                  <div className="absolute z-10 w-full mt-1 bg-card border border-border rounded-md shadow-md">
                    {articlesTrouves.map(a => (
                      <button key={a.id} type="button" onClick={() => ajouterPris(a)}
                        className="w-full text-left px-3 py-1.5 text-sm hover:bg-accent flex justify-between">
                        <span className="font-medium">{a.nom}</span>
                        {a.unites[0] && (
                          <span className="text-xs text-muted-foreground">
                            {formaterMontant(a.unites[0].prix_reference)}/{a.unites[0].libelle}
                          </span>
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Le bilan, et comment régler */}
          <div className="rounded-lg bg-muted px-3 py-2.5 space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Valeur rendue</span>
              <span className="font-semibold">{formaterMontant(credit)}</span>
            </div>
            {mode === "remboursement" && (
              <>
                <p className="text-xs text-muted-foreground">
                  La dette du client s'éteint d'abord ; le reste lui est rendu, jamais plus que ce qu'il a versé.
                </p>
                <div className="flex items-center justify-between">
                  <span className="text-muted-foreground">Rendu en</span>
                  <ChoixMoyen valeur={moyen} onChange={setMoyen} />
                </div>
              </>
            )}
            {mode === "avoir" && (
              <p className="text-xs text-muted-foreground">
                Un seul avoir pour tous les articles cochés, au crédit du client.
              </p>
            )}
            {mode === "echange" && pris.length > 0 && (
              <>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Articles pris</span>
                  <span className="font-semibold">{formaterMontant(montantPris)}</span>
                </div>
                <div className={cn("flex justify-between font-semibold border-t border-border pt-1.5",
                  reliquat > 0 ? "text-green-700" : reliquat < 0 ? "text-orange-700" : "")}>
                  <span>{reliquat > 0 ? "Reliquat au client" : reliquat < 0 ? "Le client paie" : "Échange soldé"}</span>
                  <span>{formaterMontant(Math.abs(reliquat))}</span>
                </div>
                {reliquat > 0 && (
                  <div className="flex items-center justify-between gap-2">
                    {comptant ? (
                      <span className="text-xs text-muted-foreground">Remboursé — pas d'avoir pour un client de passage.</span>
                    ) : (
                      <div className="flex gap-1">
                        {(["remboursement", "avoir"] as const).map(m => (
                          <button key={m} type="button" onClick={() => setModeReliquat(m)}
                            className={cn("px-2.5 py-1 rounded-md border text-xs font-medium",
                              modeReliquat === m ? "border-primary bg-primary text-primary-foreground" : "border-border bg-background")}>
                            {m === "remboursement" ? "Rembourser" : "En avoir"}
                          </button>
                        ))}
                      </div>
                    )}
                    {(comptant || modeReliquat === "remboursement") && (
                      <ChoixMoyen valeur={moyenReliquat} onChange={setMoyenReliquat} />
                    )}
                  </div>
                )}
                {reliquat < 0 && (
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Payé en</span>
                    <ChoixMoyen valeur={moyen} onChange={setMoyen} cheque />
                  </div>
                )}
              </>
            )}
          </div>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} disabled={chargement} className="flex-1">
              Annuler
            </Button>
            <Button onClick={confirmer} disabled={invalide || chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" />
                : mode === "remboursement" ? `Rembourser ${coches.length > 1 ? `${coches.length} articles` : ""}`
                : mode === "avoir" ? "Créer l'avoir" : "Confirmer l'échange"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
