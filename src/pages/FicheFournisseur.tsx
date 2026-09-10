import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import {
  ArrowLeft, Truck, Phone, MapPin, Mail, FileText,
  Loader2, TrendingDown, Clock, Eye, Pencil, Printer,
  Package, Banknote, CheckCircle2, RotateCcw
} from "lucide-react";
import { ApercuPiece } from "@/components/ApercuPiece";
import { ApercuRecu } from "@/components/ApercuRecu";
import { ModalModifierTiers } from "@/components/ModalModifierTiers";
import {
  genererReleveHTML, genererHistoriqueReglementsHTML, type DonneesReleve,
} from "@/lib/genererReleve";
import { GlassHalos } from "@/components/ui/GlassIcon";
import { KpiLigne, CARTE, GRILLE } from "@/components/ui/KpiVerre";
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
import { Input } from "@/components/ui/input";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { UTILISATEUR_ACTIF } from "@/App";

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
  /** FAF réglée par ce versement — vide pour un règlement global. */
  numero_facture: string;
  /** Dette restant sur CETTE facture juste après CE versement. */
  reste_apres: number;
  /** Cette ligne EST une contre-passation. */
  est_annulation: boolean;
  /** Ce versement a déjà été annulé par une contre-passation. */
  annule: boolean;
}

const MOYENS: Record<string, string> = {
  especes: "Espèces", orange_money: "Orange Money",
  moov_money: "Moov Money", cheque: "Chèque", virement: "Virement",
};

interface MouvementAchat {
  id: string; article_nom: string; quantite: number;
  prix_achat: number; date_mouvement: string;
  /** FAF d'origine — vide pour une entrée sans facture. */
  piece_id: string; piece_numero: string;
}

function fmt(n: number) {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string) {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

/** Couleur du montant selon le sort de la ligne.
 *
 *  Trois états, et les confondre trompe : une contre-passation est rouge
 *  (c'est de l'argent qui revient), le versement qu'elle annule est barré
 *  (il a existé, il ne compte plus), un versement normal est vert. */
function CLS_MONTANT(p: PaiementFournisseur) {
  const teinte = p.est_annulation ? "text-red-600"
    : p.annule ? "text-muted-foreground line-through"
    : "text-green-600";
  return `px-3 py-2 text-right font-semibold whitespace-nowrap ${teinte}`;
}

function CLS_RESTE(p: PaiementFournisseur) {
  const teinte = p.reste_apres > 0 ? "text-orange-600" : "text-green-700";
  return `px-3 py-2 text-right whitespace-nowrap text-xs ${teinte}`;
}

// =====================================================================
//  Modal : contester un paiement fournisseur
// =====================================================================
//
//  Jumeau de ModalAnnulerReglement (FicheClient) : on se trompe aussi en
//  payant. Le seul écart est le sens de la caisse — annuler un versement
//  au fournisseur fait RENTRER l'argent — et c'est le backend qui le
//  décide, pas cet écran.
// =====================================================================

function ModalAnnulerPaiement({
  paiement, onFermer, onAnnule,
}: {
  paiement: PaiementFournisseur | null;
  onFermer: () => void; onAnnule: () => void;
}) {
  const [motif, setMotif] = useState("");
  const [remboursement, setRemboursement] = useState(false);
  const [chargement, setChargement] = useState(false);

  useEffect(() => {
    if (paiement) { setMotif(""); setRemboursement(false); }
  }, [paiement]);

  async function handleAnnuler() {
    if (!paiement || !motif.trim()) return;
    setChargement(true);
    try {
      const r = await invoke<{
        montant_annule: number; entree_de_caisse: boolean;
      }>("annuler_paiement_fournisseur", {
        paiementId: paiement.id,
        motif: motif.trim(),
        remboursement,
        utilisateurRole: UTILISATEUR_ACTIF?.role ?? "patron",
      });
      await message(
        `Paiement de ${fmt(r.montant_annule)} annulé.\n\n`
        + (r.entree_de_caisse
            ? "L'argent est rentré dans la caisse."
            : "Aucun mouvement de caisse."),
        { title: "Paiement annulé", kind: "info" });
      onAnnule();
    } catch (e) {
      await message(`${e}`, { title: "Annulation impossible", kind: "error" });
    } finally { setChargement(false); }
  }

  return (
    <Dialog open={paiement !== null} onOpenChange={o => { if (!o) onFermer(); }}>
      <DialogContent style={{ width: "460px", maxWidth: "94vw" }}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RotateCcw className="h-4 w-4" />
            Annuler un paiement fournisseur
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 pt-1">
          <div className="bg-muted rounded-md px-3 py-2 text-sm space-y-1">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Montant</span>
              <span className="font-semibold">
                {paiement ? fmt(paiement.montant) : ""}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted-foreground">Facture</span>
              <span className="font-mono text-xs">
                {paiement?.numero_facture || "—"}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted-foreground">Versé le</span>
              <span>{paiement ? fmtDate(paiement.date_paiement) : ""}</span>
            </div>
          </div>

          {/* Le choix qui décide du sort de la caisse. */}
          <div className="space-y-2">
            <Label>Que s'est-il passé ?</Label>
            {([
              { val: false, titre: "Erreur de saisie",
                detail: "L'argent n'est jamais sorti : mauvais montant, "
                      + "mauvais fournisseur, ligne saisie deux fois." },
              { val: true, titre: "Le fournisseur nous rend l'argent",
                detail: "Le versement avait bien eu lieu. L'argent rentre "
                      + "dans le tiroir maintenant — la caisse doit être "
                      + "ouverte." },
            ] as const).map(o => (
              <button key={String(o.val)}
                onClick={() => setRemboursement(o.val)}
                className={`w-full text-left px-3 py-2.5 rounded-lg border-2
                            transition-colors ${
                  remboursement === o.val
                    ? "border-primary bg-primary/5"
                    : "border-border hover:border-muted-foreground"
                }`}>
                <p className={`text-sm font-medium ${
                  remboursement === o.val ? "text-primary" : ""}`}>
                  {o.titre}
                </p>
                <p className="text-xs text-muted-foreground mt-0.5">{o.detail}</p>
              </button>
            ))}
          </div>

          <div>
            <Label>Motif *</Label>
            <Input value={motif} onChange={e => setMotif(e.target.value)}
              placeholder="Ex : versement saisi deux fois le même jour"
              className="mt-1" autoFocus />
            <p className="text-xs text-muted-foreground mt-1">
              C'est ce qui explique la correction au fournisseur et au
              contrôle.
            </p>
          </div>

          <p className="text-xs text-muted-foreground border-t border-border pt-3">
            Le paiement n'est pas supprimé : une ligne de correction vient
            l'annuler. Les deux restent visibles dans l'historique, et la
            facture redevient due si elle ne l'est plus.
          </p>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Renoncer
            </Button>
            <Button onClick={handleAnnuler}
              disabled={!motif.trim() || chargement}
              variant="destructive" className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Annuler le paiement"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
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
  const [pieceApercue, setPieceApercue] =
    useState<{ id: string; numero: string } | null>(null);
  const [modalModifier, setModalModifier] = useState(false);
  const [releveEnCours, setReleveEnCours] = useState(false);
  const [recuApercu, setRecuApercu] = useState<string | null>(null);

  // Filtre de l'onglet Paiements — mêmes critères que l'onglet
  // Règlements du client, et ce qu'il montre est ce qui s'imprime.
  const [payDu, setPayDu] = useState("");
  const [payAu, setPayAu] = useState("");
  const [payMoyen, setPayMoyen] = useState("tous");
  const [payFacture, setPayFacture] = useState("");
  const [histoEnCours, setHistoEnCours] = useState(false);
  const [paiementAAnnuler, setPaiementAAnnuler] =
    useState<PaiementFournisseur | null>(null);

  const paiementsFiltres = paiements.filter(p => {
    const jour = p.date_paiement.slice(0, 10);
    if (payDu && jour < payDu) return false;
    if (payAu && jour > payAu) return false;
    if (payMoyen !== "tous" && p.mode !== payMoyen) return false;
    // Recherche PARTIELLE : personne ne tape « FAF-2026-00012 » en
    // entier, on cherche sur « 12 » ou « 00012 ».
    if (payFacture.trim()
        && !(p.numero_facture ?? "").toLowerCase()
              .includes(payFacture.trim().toLowerCase())) return false;
    return true;
  });
  const critereActif = !!payDu || !!payAu || payMoyen !== "tous"
    || !!payFacture.trim();
  const totalFiltre = paiementsFiltres.reduce((s, p) => s + p.montant, 0);

  async function imprimerHistorique() {
    if (!fournisseur) return;
    setHistoEnCours(true);
    try {
      const [societeP, logo, entete] = await Promise.all([
        invoke<any>("lire_parametres_societe"),
        invoke<string | null>("lire_logo_base64").catch(() => null),
        invoke<string | null>("lire_entete_base64").catch(() => null),
      ]);
      const criteres = [
        payDu ? `du ${fmtDate(payDu)}` : null,
        payAu ? `au ${fmtDate(payAu)}` : null,
        payMoyen !== "tous" ? `moyen : ${MOYENS[payMoyen] ?? payMoyen}` : null,
        payFacture.trim() ? `facture : « ${payFacture.trim()} »` : null,
      ].filter(Boolean).join(" · ");

      await invoke("imprimer_facture", {
        html: genererHistoriqueReglementsHTML(
          { nom: fournisseur.nom, telephone: fournisseur.telephone },
          "fournisseur",
          paiementsFiltres.map(p => ({
            date_paiement: p.date_paiement,
            numero_facture: p.numero_facture,
            mode:           p.mode,
            montant:        p.montant,
            reste_apres:    p.reste_apres,
            auteur_nom:     p.auteur_nom ?? "—",
            est_annulation: p.est_annulation,
            deja_annule:    p.annule,
          })),
          criteres,
          // Ce qu'on doit ENCORE au fournisseur, toutes factures — le
          // chiffre qu'il vient vérifier, distinct du solde par ligne.
          stats?.dette ?? 0,
          societeP, logo, entete),
        nomFichier: `paiements_${fournisseur.nom}`
          .replace(/[\\/:*?"<>|]/g, "-") + ".html",
      });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    } finally {
      setHistoEnCours(false);
    }
  }

  /**
   * État de dette — le relevé qu'on oppose au fournisseur.
   *
   * Les montants viennent du backend : la dette se lit dans
   * `paiement_fournisseur` (D9, D36), jamais dans le statut des pièces.
   * La recalculer ici ferait diverger le papier de l'écran.
   */
  async function imprimerReleve() {
    setReleveEnCours(true);
    try {
      const [donnees, logo, entete] = await Promise.all([
        invoke<DonneesReleve>("lire_etat_dette_fournisseur", { fournisseurId }),
        invoke<string | null>("lire_logo_base64").catch(() => null),
        invoke<string | null>("lire_entete_base64").catch(() => null),
      ]);
      await invoke("imprimer_facture", {
        html: genererReleveHTML(donnees, "fournisseur", logo, entete),
        nomFichier: `dette_${donnees.tiers.nom}`
          .replace(/[\\/:*?"<>|]/g, "-") + ".html",
      });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    } finally {
      setReleveEnCours(false);
    }
  }

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
        <div className="ml-auto flex gap-2">
          <Button size="sm" variant="outline"
            onClick={() => setModalModifier(true)}>
            <Pencil className="h-4 w-4 mr-1" /> Modifier
          </Button>
          <Button size="sm" variant="outline" onClick={imprimerReleve}
            disabled={releveEnCours}>
            {releveEnCours
              ? <Loader2 className="h-4 w-4 mr-1 animate-spin" />
              : <Printer className="h-4 w-4 mr-1" />}
            État de dette
          </Button>
          {stats.dette > 0 && (
            <Button size="sm" onClick={() => setModalDette(true)}
              className="gap-1.5 bg-orange-600 hover:bg-orange-700">
              <Banknote className="h-3.5 w-3.5" />
              Régler dette · {fmt(stats.dette)}
            </Button>
          )}
        </div>
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

      {/* Voir FicheClient : les halos en `inset:-10%` débordent à droite
          et créaient une barre de défilement horizontale. */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden p-6 relative">
        {/* Le verre a besoin d'un fond non uni. */}
        <GlassHalos sansRose />

        {/* ---- Résumé ---- */}
        {onglet === "resume" && (
          <div className="space-y-6">

            {/* Infos fournisseur */}
            <div className={`${CARTE} p-4 space-y-2`}>
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
            <div style={GRILLE}>
              {[
                { label: "Total achats", val: fmt(stats.total_achats),
                  icone: TrendingDown, variante: "tinted" as const, inactif: false },
                { label: "Nb commandes", val: stats.nb_achats.toString(),
                  icone: Package, variante: "neutral" as const, inactif: false },
                // D9 : la dette se lit dans les paiements, pas dans le
                // statut — la tuile ne fait que refleter stats.dette.
                { label: "Dette actuelle", val: fmt(stats.dette),
                  icone: Banknote,
                  variante: stats.dette > 0 ? ("tinted" as const) : ("clear" as const),
                  inactif: false },
                { label: "Total payé", val: fmt(stats.total_paye),
                  icone: CheckCircle2, variante: "neutral" as const, inactif: false },
                { label: "Dernière commande",
                  val: stats.derniere_commande ? fmtDate(stats.derniere_commande) : "—",
                  icone: Clock, variante: "clear" as const,
                  inactif: !stats.derniere_commande },
              ].map(k => (
                <KpiLigne key={k.label} label={k.label} valeur={k.val}
                  icone={k.icone} variante={k.variante} inactif={k.inactif} />
              ))}
            </div>

            {/* Barre dette */}
            {stats.total_achats > 0 && (
              <div className={`${CARTE} p-4`}>
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
                      {a.piece_numero && ` · ${a.piece_numero}`}
                    </p>
                  </div>
                  <div className="flex items-center gap-3">
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
                    {/* Seulement si la facture est retrouvable : une
                        entrée sans facture n'a pas de pièce à montrer. */}
                    {a.piece_id && (
                      <Button size="sm" variant="ghost" title="Aperçu de la facture"
                        onClick={() => setPieceApercue(
                          { id: a.piece_id, numero: a.piece_numero })}
                        className="h-7 w-7 p-0 shrink-0">
                        <Eye className="h-3.5 w-3.5" />
                      </Button>
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
            {/* Filtre. Ce qu'il montre est exactement ce qui s'imprime. */}
            <div className="flex items-end gap-2 flex-wrap">
              <div>
                <Label className="text-xs text-muted-foreground">Du</Label>
                <Input type="date" value={payDu}
                  onChange={e => setPayDu(e.target.value)}
                  className="h-8 text-sm w-36 mt-0.5" />
              </div>
              <div>
                <Label className="text-xs text-muted-foreground">Au</Label>
                <Input type="date" value={payAu}
                  onChange={e => setPayAu(e.target.value)}
                  className="h-8 text-sm w-36 mt-0.5" />
              </div>
              <div>
                <Label className="text-xs text-muted-foreground">Moyen</Label>
                <select value={payMoyen}
                  onChange={e => setPayMoyen(e.target.value)}
                  className="h-8 px-2 text-sm border border-border rounded-md
                             bg-background w-36 mt-0.5 block">
                  <option value="tous">Tous</option>
                  {Object.entries(MOYENS).map(([k, v]) => (
                    <option key={k} value={k}>{v}</option>
                  ))}
                </select>
              </div>
              <div>
                <Label className="text-xs text-muted-foreground">Facture</Label>
                <Input value={payFacture}
                  onChange={e => setPayFacture(e.target.value)}
                  placeholder="N° ou fin du n°"
                  className="h-8 text-sm w-36 mt-0.5" />
              </div>
              {critereActif && (
                <Button variant="ghost" size="sm" className="h-8 text-xs"
                  onClick={() => {
                    setPayDu(""); setPayAu("");
                    setPayMoyen("tous"); setPayFacture("");
                  }}>
                  Réinitialiser
                </Button>
              )}
              <Button variant="outline" size="sm" className="h-8 ml-auto"
                onClick={imprimerHistorique}
                disabled={histoEnCours || paiementsFiltres.length === 0}>
                {histoEnCours
                  ? <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                  : <Printer className="h-4 w-4 mr-1" />}
                Imprimer
              </Button>
            </div>

            {paiements.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun paiement enregistré pour ce fournisseur.
              </p>
            ) : paiementsFiltres.length === 0 ? (
              <p className="text-sm text-muted-foreground text-center py-8">
                Aucun paiement sur cette période.
              </p>
            ) : (
              /* Le tableau défile DANS son cadre : sans ce conteneur,
                 c'est la page entière qui part de côté. */
              <div className="border border-border rounded-lg overflow-x-auto">
                <table className="w-full text-sm min-w-[680px]">
                  <thead className="bg-muted/50 border-b border-border">
                    <tr>
                      <th className="text-left px-3 py-2 font-medium">Date</th>
                      <th className="text-left px-3 py-2 font-medium">Facture</th>
                      <th className="text-left px-3 py-2 font-medium">Moyen</th>
                      <th className="text-left px-3 py-2 font-medium">Saisi par</th>
                      <th className="text-right px-3 py-2 font-medium">Montant</th>
                      <th className="text-right px-3 py-2 font-medium">
                        Reste à payer après
                      </th>
                      <th className="px-3 py-2"></th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {paiementsFiltres.map(p => (
                      <tr key={p.id} className={p.annule ? "bg-muted/30" : ""}>
                        <td className="px-3 py-2 text-xs whitespace-nowrap">
                          {fmtDate(p.date_paiement)}
                        </td>
                        <td className="px-3 py-2 text-xs font-mono">
                          {p.numero_facture || "—"}
                        </td>
                        <td className="px-3 py-2 text-xs">
                          {MOYENS[p.mode] ?? p.mode}
                        </td>
                        <td className="px-3 py-2 text-xs text-muted-foreground">
                          {p.auteur_nom ?? "—"}
                        </td>
                        {/* Une annulation s'affiche en négatif, en rouge :
                            le fournisseur doit voir la correction, pas une
                            ligne qui a disparu. */}
                        <td className={CLS_MONTANT(p)}>
                          {fmt(p.montant)}
                        </td>
                        {/* Le solde de CETTE facture après CE versement —
                            pas la dette totale envers le fournisseur. */}
                        <td className={CLS_RESTE(p)}>
                          {p.numero_facture
                            ? (p.reste_apres > 0 ? fmt(p.reste_apres) : "soldée")
                            : "—"}
                        </td>
                        <td className="px-3 py-2 text-right whitespace-nowrap">
                          {/* Le reçu du paiement fournisseur : la preuve
                              de ce qu'on lui a versé, à opposer s'il le
                              conteste. */}
                          <Button size="sm" variant="ghost"
                            className="h-7 w-7 p-0" title="Reçu de paiement"
                            onClick={() => setRecuApercu(p.id)}>
                            <Eye className="h-3.5 w-3.5" />
                          </Button>
                          {p.est_annulation ? (
                            <span className="text-[10px] text-red-600 ml-1">
                              Annulation
                            </span>
                          ) : p.annule ? (
                            <span className="text-[10px] text-muted-foreground ml-1">
                              Annulé
                            </span>
                          ) : (
                            <Button size="sm" variant="ghost"
                              className="h-7 text-xs gap-1"
                              onClick={() => setPaiementAAnnuler(p)}>
                              <RotateCcw className="h-3 w-3" /> Annuler
                            </Button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {paiementsFiltres.length > 0 && (
              <div className="flex items-center justify-between px-4 py-3
                              border border-border rounded-lg bg-muted/30
                              flex-wrap gap-2">
                <span className="text-sm">
                  {critereActif ? "Total sur la période" : "Total versé"}
                  <span className="text-xs text-muted-foreground ml-1">
                    ({paiementsFiltres.length} ligne{
                      paiementsFiltres.length > 1 ? "s" : ""})
                  </span>
                </span>
                <span className="font-bold">{fmt(totalFiltre)}</span>
              </div>
            )}

            <p className="text-xs text-muted-foreground">
              « Reste à payer après » donne le solde de la facture concernée
              juste après ce versement, pas la dette totale envers ce
              fournisseur. Un paiement annulé n'est jamais effacé : la ligne
              reste, et une contre-passation vient l'annuler.
            </p>
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

      <ApercuPiece
        pieceId={pieceApercue?.id ?? null}
        numero={pieceApercue?.numero}
        typePiece="facture_fournisseur"
        onFermer={() => setPieceApercue(null)}
      />

      <ModalModifierTiers
        tiers={modalModifier ? fournisseur : null}
        cote="fournisseur"
        onFermer={() => setModalModifier(false)}
        onModifie={charger}
      />

      <ApercuRecu
        paiementId={recuApercu}
        cote="fournisseur"
        onFermer={() => setRecuApercu(null)}
      />

      <ModalAnnulerPaiement
        paiement={paiementAAnnuler}
        onFermer={() => setPaiementAAnnuler(null)}
        onAnnule={() => { setPaiementAAnnuler(null); charger(); }}
      />
    </div>
  );
}