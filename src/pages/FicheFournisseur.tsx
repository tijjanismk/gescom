import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft, Truck, Phone, MapPin, Mail, FileText,
  Loader2, Plus, Printer, TrendingDown, Clock,
  Package, Banknote, History, CheckCircle2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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

// =====================================================================
//  Types
// =====================================================================

interface Fournisseur {
  id: string; nom: string; telephone?: string;
  adresse?: string; nif?: string; email?: string;
  est_voisin: boolean; cree_le: string;
}

interface StatsFournisseur {
  total_achats: number;
  nb_achats: number;
  dette: number;
  total_paye: number;
  derniere_commande?: string;
}

interface PaiementFournisseur {
  id: string; montant: number; mode: string;
  note?: string; date_paiement: string; auteur_nom?: string;
}

interface MouvementAchat {
  id: string; article_nom: string; quantite: number;
  prix_achat: number; date_mouvement: string;
}

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

// =====================================================================
//  Modal règlement dette fournisseur
// =====================================================================

function ModalReglementDette({
  ouvert, fournisseur, dette, onFermer, onRegle,
}: {
  ouvert: boolean;
  fournisseur: Fournisseur | null;
  dette: number;
  onFermer: () => void;
  onRegle: () => void;
}) {
  const [montant, setMontant] = useState("");
  const [mode, setMode] = useState("especes");
  const [note, setNote] = useState("");
  const [chargement, setChargement] = useState(false);
  const [resultat, setResultat] = useState(false);

  useEffect(() => {
    if (ouvert) {
      setMontant(dette.toString());
      setMode("especes"); setNote(""); setResultat(false);
    }
  }, [ouvert, dette]);

  async function handleRegler() {
    if (!fournisseur) return;
    setChargement(true);
    try {
      await invoke("regler_dette_fournisseur", {
        fournisseurId: fournisseur.id,
        montant: parseMontant(montant),
        mode, note: note || null,
      });
      setResultat(true);
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
          {resultat ? (
            <div className="flex items-center gap-3 p-4 rounded-lg
                            bg-green-50 border border-green-200">
              <CheckCircle2 className="h-6 w-6 text-green-600 shrink-0" />
              <p className="text-sm font-medium text-green-800">Paiement enregistré ✓</p>
            </div>
          ) : (
            <>
              <div className="bg-muted rounded-md px-3 py-2.5">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{fournisseur.nom}</span>
                  <span className="font-bold text-orange-600">{fmt(dette)}</span>
                </div>
                <p className="text-xs text-muted-foreground mt-0.5">Dette actuelle</p>
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
                <input value={note} onChange={e => setNote(e.target.value)}
                  placeholder="Ex: facture n°..."
                  className="w-full h-9 px-3 text-sm border border-border rounded-md
                             bg-background focus:outline-none focus:ring-1 focus:ring-primary" />
              </div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
                <Button onClick={handleRegler}
                  disabled={!montant || chargement} className="flex-1">
                  {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Confirmer"}
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
//  FicheFournisseur
// =====================================================================

interface FicheFournisseurProps {
  fournisseurId: string;
  onRetour: () => void;
}

export function FicheFournisseur({ fournisseurId, onRetour }: FicheFournisseurProps) {
  const [fournisseur, setFournisseur] = useState<Fournisseur | null>(null);
  const [stats, setStats] = useState<StatsFournisseur | null>(null);
  const [paiements, setPaiements] = useState<PaiementFournisseur[]>([]);
  const [achats, setAchats] = useState<MouvementAchat[]>([]);
  const [chargement, setChargement] = useState(true);
  const [onglet, setOnglet] = useState("resume");
  const [modalDette, setModalDette] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const [f, det] = await Promise.all([
        invoke<Fournisseur>("lire_fournisseur_detail", { fournisseurId }),
        invoke<{ stats: StatsFournisseur; paiements: PaiementFournisseur[]; achats: MouvementAchat[] }>(
          "lire_fiche_fournisseur", { fournisseurId }
        ),
      ]);
      setFournisseur(f);
      setStats(det.stats);
      setPaiements(det.paiements);
      setAchats(det.achats);
    } catch (e) {
      console.error("Erreur fiche fournisseur :", e);
    } finally { setChargement(false); }
  }

  useEffect(() => { charger(); }, [fournisseurId]);

  if (chargement) return (
    <div className="flex-1 flex items-center justify-center">
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
    </div>
  );
  if (!fournisseur || !stats) return null;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">

      {/* En-tête */}
      <div className="flex items-center gap-3 px-6 h-14 border-b border-border
                      bg-card shrink-0">
        <button onClick={onRetour}
          className="p-1.5 rounded-md hover:bg-accent transition-colors">
          <ArrowLeft className="h-4 w-4" />
        </button>
        <div className="w-8 h-8 rounded-full bg-orange-100 flex items-center justify-center">
          <Truck className="h-4 w-4 text-orange-600" />
        </div>
        <div>
          <p className="font-semibold text-sm">{fournisseur.nom}</p>
          <div className="flex items-center gap-2">
            {fournisseur.est_voisin && (
              <Badge variant="outline" className="text-[10px]">Voisin</Badge>
            )}
          </div>
        </div>
        {stats.dette > 0 && (
          <div className="ml-auto">
            <Button size="sm" onClick={() => setModalDette(true)}
              className="gap-1.5 bg-orange-600 hover:bg-orange-700">
              <Banknote className="h-3.5 w-3.5" />
              Régler dette · {fmt(stats.dette)}
            </Button>
          </div>
        )}
      </div>

      {/* Onglets */}
      <div className="flex gap-1 px-6 border-b border-border bg-card shrink-0">
        {[
          { key: "resume",    label: "Résumé"    },
          { key: "achats",    label: `Achats (${achats.length})`    },
          { key: "paiements", label: `Paiements (${paiements.length})` },
        ].map(o => (
          <button key={o.key} onClick={() => setOnglet(o.key)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors ${
              onglet === o.key
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground"
            }`}>
            {o.label}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-auto p-6">

        {/* ---- Résumé ---- */}
        {onglet === "resume" && (
          <div className="space-y-6">

            {/* Infos fournisseur */}
            <div className="border border-border rounded-lg p-4 space-y-2">
              <p className="text-sm font-medium mb-3">Informations</p>
              {fournisseur.telephone && (
                <div className="flex items-center gap-2 text-sm">
                  <Phone className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{fournisseur.telephone}</span>
                </div>
              )}
              {fournisseur.adresse && (
                <div className="flex items-center gap-2 text-sm">
                  <MapPin className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{fournisseur.adresse}</span>
                </div>
              )}
              {fournisseur.email && (
                <div className="flex items-center gap-2 text-sm">
                  <Mail className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>{fournisseur.email}</span>
                </div>
              )}
              {fournisseur.nif && (
                <div className="flex items-center gap-2 text-sm">
                  <FileText className="h-3.5 w-3.5 text-muted-foreground" />
                  <span>NIF : {fournisseur.nif}</span>
                </div>
              )}
              <p className="text-xs text-muted-foreground pt-1">
                Fournisseur depuis le {fmtDate(fournisseur.cree_le)}
              </p>
            </div>

            {/* KPIs */}
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              {[
                { label: "Total achats",      val: fmt(stats.total_achats),
                  icone: TrendingDown, color: "text-primary" },
                { label: "Nb commandes",      val: stats.nb_achats.toString(),
                  icone: Package, color: "text-blue-600" },
                { label: "Dette actuelle",    val: fmt(stats.dette),
                  icone: Banknote,
                  color: stats.dette > 0 ? "text-orange-600" : "text-green-600" },
                { label: "Total payé",        val: fmt(stats.total_paye),
                  icone: CheckCircle2, color: "text-green-600" },
                { label: "Dernière commande", val: stats.derniere_commande
                    ? fmtDate(stats.derniere_commande) : "—",
                  icone: Clock, color: "text-muted-foreground" },
              ].map(k => {
                const Icone = k.icone;
                return (
                  <div key={k.label}
                    className="border border-border rounded-lg p-3 flex items-center gap-3">
                    <Icone className={`h-5 w-5 shrink-0 ${k.color}`} />
                    <div>
                      <p className="text-xs text-muted-foreground">{k.label}</p>
                      <p className="text-sm font-semibold">{k.val}</p>
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Barre dette */}
            {stats.total_achats > 0 && (
              <div className="border border-border rounded-lg p-4">
                <div className="flex justify-between text-sm mb-2">
                  <span className="text-muted-foreground">Taux de paiement</span>
                  <span className="font-medium">
                    {Math.round((stats.total_paye / stats.total_achats) * 100)}%
                  </span>
                </div>
                <div className="w-full bg-muted rounded-full h-2">
                  <div
                    className="bg-green-500 h-2 rounded-full transition-all"
                    style={{ width: `${Math.min(100, (stats.total_paye / stats.total_achats) * 100)}%` }}
                  />
                </div>
                <div className="flex justify-between text-xs text-muted-foreground mt-1.5">
                  <span>Payé : {fmt(stats.total_paye)}</span>
                  <span>Total : {fmt(stats.total_achats)}</span>
                </div>
              </div>
            )}
          </div>
        )}

        {/* ---- Achats ---- */}
        {onglet === "achats" && (
          <div className="space-y-2">
            {achats.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun achat enregistré
              </p>
            ) : (
              achats.map(a => (
                <div key={a.id}
                  className="flex items-center justify-between px-4 py-3
                             border border-border rounded-lg">
                  <div>
                    <p className="text-sm font-medium">{a.article_nom}</p>
                    <p className="text-xs text-muted-foreground">
                      {fmtDate(a.date_mouvement)}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-medium">
                      {a.quantite % 1 === 0 ? a.quantite : a.quantite.toFixed(2)} unités
                    </p>
                    {a.prix_achat > 0 && (
                      <p className="text-xs text-muted-foreground">
                        {fmt(a.prix_achat)} / u
                      </p>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {/* ---- Paiements ---- */}
        {onglet === "paiements" && (
          <div className="space-y-2">
            {stats.dette > 0 && (
              <div className="flex items-center justify-between px-4 py-3 mb-2
                              bg-orange-50 border border-orange-200 rounded-lg">
                <span className="text-sm font-medium text-orange-800">Dette restante</span>
                <div className="flex items-center gap-3">
                  <span className="font-bold text-orange-700">{fmt(stats.dette)}</span>
                  <Button size="sm" onClick={() => setModalDette(true)}
                    className="bg-orange-600 hover:bg-orange-700 h-7 text-xs">
                    Régler
                  </Button>
                </div>
              </div>
            )}
            {paiements.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun paiement enregistré
              </p>
            ) : (
              paiements.map(p => (
                <div key={p.id}
                  className="flex items-center justify-between px-4 py-3
                             border border-border rounded-lg">
                  <div>
                    <p className="text-sm font-medium capitalize">
                      {p.mode.replace(/_/g, " ")}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {fmtDate(p.date_paiement)}
                      {p.auteur_nom && ` · ${p.auteur_nom}`}
                    </p>
                    {p.note && (
                      <p className="text-xs text-muted-foreground italic">{p.note}</p>
                    )}
                  </div>
                  <span className="text-sm font-bold text-green-600">{fmt(p.montant)}</span>
                </div>
              ))
            )}
          </div>
        )}
      </div>

      {/* Modal dette */}
      <ModalReglementDette
        ouvert={modalDette}
        fournisseur={fournisseur}
        dette={stats.dette}
        onFermer={() => setModalDette(false)}
        onRegle={() => { setModalDette(false); charger(); }}
      />
    </div>
  );
}