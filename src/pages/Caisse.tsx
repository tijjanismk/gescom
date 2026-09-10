import { useState, useEffect, useCallback } from "react";
import { appeler as invoke } from "@/lib/pont";
import {
  Wallet, RefreshCw, Loader2, TrendingUp, TrendingDown,
  // `History` entre en collision avec window.History, une classe
  // native que React tenterait d'instancier (« Illegal constructor »).
  History as HistoryIcon,
  Lock, Unlock, AlertTriangle, CheckCircle2, MinusCircle
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { GlassIcon, GlassHalos } from "@/components/ui/GlassIcon";
import { KpiPetit, GRILLE } from "@/components/ui/KpiVerre";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { UTILISATEUR_ACTIF } from "@/App";
import { OngletHistoriqueCaisse } from "@/components/OngletHistoriqueCaisse";

// =====================================================================
//  Types
// =====================================================================

interface ResumeCaisse {
  session_id: string | null;
  statut: "ouverte" | "fermee" | "aucune";
  fond_ouverture: number;
  total_entrees: number;
  total_sorties: number;
  entrees_especes: number;
  sorties_especes: number;
  solde_theorique: number;
  nb_transactions: number;
  ouvert_le: string | null;
}

interface MouvementCaisse {
  id: string;
  sens: "entree" | "sortie";
  moyen: string;
  montant: number;
  motif: string;
  date_mouvement: string;
  /** Texte saisi pour une dépense — vide pour les mouvements automatiques. */
  libelle?: string | null;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function fmtHeure(iso: string): string {
  return new Date(iso).toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit",
  });
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

function fmtMotif(motif: string): string {
  return {
    vente: "Vente",
    achat: "Achat",
    reglement_fournisseur: "Règlement fournisseur",
    retour_fournisseur: "Retour fournisseur",
    depense: "Dépense",
    remboursement: "Remboursement",
    remboursement_reliquat: "Remboursement reliquat",
    complement_echange: "Complément échange",
    ouverture: "Fond d'ouverture",
    fermeture: "Clôture",
    autre: "Autre",
  }[motif] ?? motif;
}

function fmtMoyen(moyen: string): string {
  return {
    especes: "Espèces",
    orange_money: "Orange Money",
    moov_money: "Moov Money",
    cheque: "Chèque",
  }[moyen] ?? moyen;
}

// =====================================================================
//  Modal : Ouverture de session
// =====================================================================

function ModalOuvertureSession({
  ouvert, onFermer, onOuvrir,
}: {
  ouvert: boolean;
  onFermer: () => void;
  onOuvrir: () => void;
}) {
  const [fondOuverture, setFondOuverture] = useState("");
  const [chargement, setChargement] = useState(false);

  async function handleOuvrir() {
    setChargement(true);
    try {
      const role = UTILISATEUR_ACTIF?.role ?? "patron";
      await invoke("ouvrir_session_caisse", {
        fondOuverture: parseMontant(fondOuverture),
        utilisateurRole: role,
      });
      setFondOuverture("");
      onOuvrir();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Unlock className="h-4 w-4" /> Ouvrir la caisse
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <p className="text-sm text-muted-foreground">
            Saisir le fond de caisse initial (espèces en caisse avant l'ouverture).
          </p>
          <div>
            <Label>Fond d'ouverture (F)</Label>
            <MoneyInput value={fondOuverture} onChange={setFondOuverture}
              placeholder="0" className="mt-1" autoFocus />
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleOuvrir} disabled={chargement} className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Ouvrir"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Saisie d'une dépense
// =====================================================================
//
//  Une depense n'est ni un achat ni un reglement fournisseur : loyer,
//  transport, carburant. Sans ce module, ces sorties d'argent
//  n'apparaissaient nulle part et la caisse semblait en excedent.

// Postes de depense. "autre" est le defaut : mieux vaut une depense
// non ventilee qu'une depense non saisie.
const CATEGORIES_DEPENSE = [
  { value: "transport",   label: "Transport"        },
  { value: "carburant",   label: "Carburant"        },
  { value: "loyer",       label: "Loyer"            },
  { value: "salaire",     label: "Salaire / main d'œuvre" },
  { value: "electricite", label: "Électricité"      },
  { value: "eau",         label: "Eau"              },
  { value: "fourniture",  label: "Fournitures"      },
  { value: "entretien",   label: "Entretien"        },
  { value: "taxe",        label: "Taxe / impôt"     },
  { value: "autre",       label: "Autre"            },
];

function ModalDepense({
  ouvert, onAnnuler, onEnregistre,
}: {
  ouvert: boolean;
  onAnnuler: () => void;
  onEnregistre: () => void;
}) {
  const [montant, setMontant] = useState("");
  const [libelle, setLibelle] = useState("");
  const [categorie, setCategorie] = useState("autre");
  const [moyen, setMoyen] = useState("especes");
  const [chargement, setChargement] = useState(false);

  async function handleEnregistrer() {
    const val = parseMontant(montant);
    if (val <= 0 || !libelle.trim()) return;
    setChargement(true);
    try {
      await invoke("enregistrer_depense", {
        montant: val,
        libelle: libelle.trim(),
        categorie,
        moyen,
        utilisateurRole: UTILISATEUR_ACTIF?.role ?? "employe",
      });
      setMontant(""); setLibelle(""); setMoyen("especes");
      setCategorie("autre");
      onEnregistre();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onAnnuler}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <MinusCircle className="h-4 w-4" /> Enregistrer une dépense
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">
          <div>
            <Label>Libellé</Label>
            <input
              value={libelle}
              onChange={e => setLibelle(e.target.value)}
              placeholder="Transport, carburant, loyer…"
              autoFocus
              className="mt-1 w-full h-9 px-3 text-sm border border-border
                         rounded-md bg-background focus:outline-none
                         focus:ring-1 focus:ring-primary" />
          </div>
          <div>
            <Label>Montant (F)</Label>
            <MoneyInput value={montant} onChange={setMontant}
              placeholder="0" className="mt-1" />
          </div>
          <div>
            <Label>Poste</Label>
            <select value={categorie} onChange={e => setCategorie(e.target.value)}
              className="mt-1 w-full h-9 px-2 text-sm border border-border
                         rounded-md bg-background">
              {CATEGORIES_DEPENSE.map(c => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>
          <div>
            <Label>Moyen</Label>
            <select value={moyen} onChange={e => setMoyen(e.target.value)}
              className="mt-1 w-full h-9 px-2 text-sm border border-border
                         rounded-md bg-background">
              <option value="especes">Espèces</option>
              <option value="orange_money">Orange Money</option>
              <option value="moov_money">Moov Money</option>
              <option value="cheque">Chèque</option>
            </select>
          </div>
          <p className="text-xs text-muted-foreground">
            Seules les dépenses en espèces sortent du tiroir physique.
          </p>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onAnnuler} className="flex-1">
              Annuler
            </Button>
            <Button onClick={handleEnregistrer}
              disabled={chargement || !montant || !libelle.trim()}
              className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Enregistrer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Modal : Fermeture de session (rapprochement)
// =====================================================================

function ModalFermetureSession({
  ouvert, resume, onAnnuler, onCloturer,
}: {
  ouvert: boolean;
  resume: ResumeCaisse | null;
  /** Ferme le modal sans rien faire. */
  onAnnuler: () => void;
  /** Appelé APRÈS clôture effective de la caisse. */
  onCloturer: () => void;
}) {
  const [especesComptees, setEspecesComptees] = useState("");
  const [chargement, setChargement] = useState(false);

  const montantCompte = parseMontant(especesComptees);
  const ecart = resume ? montantCompte - resume.solde_theorique : 0;

  async function handleFermer() {
    if (!resume?.session_id) return;
    setChargement(true);
    try {
      await invoke("fermer_session_caisse", {
        sessionId: resume.session_id,
        especesComptees: montantCompte,
      });
      setEspecesComptees("");
      onCloturer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onAnnuler}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lock className="h-4 w-4" /> Fermer la caisse
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">

          {/* Résumé de la session */}
          <div className="space-y-2 bg-muted rounded-lg p-3">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Fond d'ouverture</span>
              <span>{fmt(resume?.fond_ouverture ?? 0)}</span>
            </div>
            <div className="flex justify-between text-sm text-green-600">
              <span>Entrées espèces</span>
              <span>+ {fmt(resume?.entrees_especes ?? 0)}</span>
            </div>
            <div className="flex justify-between text-sm text-red-500">
              <span>Sorties espèces</span>
              <span>- {fmt(resume?.sorties_especes ?? 0)}</span>
            </div>
            {(resume?.total_entrees ?? 0) !== (resume?.entrees_especes ?? 0) && (
              <div className="flex justify-between text-xs text-muted-foreground">
                <span>Dont mobile money / chèque (hors tiroir)</span>
                <span>
                  {fmt((resume?.total_entrees ?? 0) - (resume?.entrees_especes ?? 0))}
                </span>
              </div>
            )}
            <div className="flex justify-between text-sm font-bold border-t border-border pt-2">
              <span>Solde théorique espèces</span>
              <span>{fmt(resume?.solde_theorique ?? 0)}</span>
            </div>
          </div>

          {/* Rapprochement */}
          <div>
            <Label>Espèces comptées physiquement (F)</Label>
            <MoneyInput value={especesComptees} onChange={setEspecesComptees}
              placeholder="0" className="mt-1" autoFocus />
          </div>

          {especesComptees && (
            <div className={`flex justify-between text-sm font-semibold px-3 py-2 rounded-md ${
              ecart === 0 ? "bg-green-50 text-green-600"
              : ecart > 0 ? "bg-blue-50 text-blue-600"
              : "bg-red-50 text-red-600"
            }`}>
              <span>
                {ecart === 0 ? "✓ Caisse équilibrée"
                  : ecart > 0 ? "Excédent"
                  : "Manque"}
              </span>
              <span>
                {ecart !== 0
                  ? `${ecart > 0 ? "+" : ""}${fmt(ecart)}`
                  : "0 F"}
              </span>
            </div>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onAnnuler} className="flex-1">Annuler</Button>
            <Button onClick={handleFermer}
              disabled={!especesComptees || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Clôturer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Page Caisse
// =====================================================================

export function Caisse() {
  const [resume, setResume] = useState<ResumeCaisse | null>(null);
  const [mouvements, setMouvements] = useState<MouvementCaisse[]>([]);
  const [chargement, setChargement] = useState(true);
  const [modalOuverture, setModalOuverture] = useState(false);
  const [modalFermeture, setModalFermeture] = useState(false);
  const [modalDepense, setModalDepense] = useState(false);
  const [onglet, setOnglet] = useState<"jour" | "historique">("jour");

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const [r, m] = await Promise.all([
        invoke<ResumeCaisse>("lire_resume_caisse"),
        invoke<MouvementCaisse[]>("lire_mouvements_caisse_du_jour"),
      ]);
      setResume(r);
      setMouvements(m);
    } catch (e) {
      console.error("Erreur caisse :", e);
    } finally {
      setChargement(false);
    }
  }, []);

  useEffect(() => { charger(); }, []);

  async function handleApresOuverture() {
    setModalOuverture(false);
    await charger();
    await message("Caisse ouverte ✓", { title: "Succès", kind: "info" });
  }

  async function handleApresFermeture() {
    setModalFermeture(false);
    await charger();
    await message("Caisse clôturée ✓", { title: "Succès", kind: "info" });
  }

  if (chargement) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const sessionOuverte = resume?.statut === "ouverte";

  return (
    // Voir FicheClient : les halos en `inset:-10%` débordent à droite et
    // créaient une barre de défilement horizontale.
    <div className="flex-1 overflow-y-auto overflow-x-hidden p-6 relative"
         style={{ fontFamily: '"Archivo Variable", Archivo, system-ui, sans-serif' }}>
      {/* Le verre a besoin d'un fond non uni. */}
      <GlassHalos />
      <div className="relative z-[1] flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Caisse</h1>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={charger}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
          {sessionOuverte && (
            <Button variant="outline" size="sm"
              onClick={() => setModalDepense(true)}>
              <MinusCircle className="h-4 w-4 mr-2" /> Dépense
            </Button>
          )}
          {sessionOuverte ? (
            <Button variant="destructive" size="sm"
              onClick={() => setModalFermeture(true)}>
              <Lock className="h-4 w-4 mr-2" /> Clôturer
            </Button>
          ) : (
            <Button size="sm" onClick={() => setModalOuverture(true)}>
              <Unlock className="h-4 w-4 mr-2" /> Ouvrir
            </Button>
          )}
        </div>
      </div>

      {/* Onglets */}
      <div className="relative z-[1] flex gap-2 border-b border-border mb-6 pb-2">
        {[
          { key: "jour",       label: "Aujourd'hui", icone: Wallet },
          { key: "historique", label: "Historique",  icone: HistoryIcon },
        ].map(o => {
          const actif = onglet === o.key;
          return (
            <button key={o.key} onClick={() => setOnglet(o.key as typeof onglet)}
              className={`flex items-center gap-2.5 pr-3 text-sm transition-colors
                ${actif ? "text-foreground" : "text-muted-foreground hover:text-foreground"}`}
              style={{ fontWeight: actif ? 700 : 500 }}>
              <GlassIcon icone={o.icone} taille="sm"
                variante="clear" actif={actif} />
              {o.label}
            </button>
          );
        })}
      </div>

      {onglet === "historique" && <OngletHistoriqueCaisse />}

      {onglet === "jour" && (<>

      {/* Statut de la session */}
      {resume?.statut === "aucune" && (
        <div className="relative z-[1] flex items-center gap-3 p-4 bg-muted/70 border border-border mb-6">
          <AlertTriangle className="h-5 w-5 text-orange-500 shrink-0" />
          <div>
            <p className="text-sm font-medium">Aucune session ouverte</p>
            <p className="text-xs text-muted-foreground">
              Les paiements reçus ne seront pas enregistrés en caisse.
            </p>
          </div>
        </div>
      )}

      {sessionOuverte && resume && (
        <div className="relative z-[1] flex items-center gap-3 p-3 bg-green-50/70 dark:bg-green-950/20
          border border-green-200 mb-6 text-sm">
          <CheckCircle2 className="h-4 w-4 text-green-600 shrink-0" />
          <span className="text-green-700 dark:text-green-400">
            Session ouverte le {fmtDate(resume.ouvert_le!)}
            {" "}— fond : {fmt(resume.fond_ouverture)}
          </span>
        </div>
      )}

      {/* KPIs
          Session fermee : les tuiles s'eteignent. D46 — plus aucune
          operation d'argent n'est acceptee, le chiffre reste lisible
          mais ne se donne plus pour un etat courant. */}
      {resume && resume.statut !== "aucune" && (
        <div style={{ ...GRILLE, marginBottom: "1.5rem" }}>
          <KpiPetit
            titre="Fond d'ouverture"
            valeur={fmt(resume.fond_ouverture)}
            icone={Unlock}
            variante="clear"
            inactif={!sessionOuverte}
          />
          <KpiPetit
            titre="Entrées"
            valeur={`+ ${fmt(resume.total_entrees)}`}
            sous={`${resume.nb_transactions} mouvement${resume.nb_transactions > 1 ? "s" : ""}`}
            icone={TrendingUp}
            variante="neutral"
            inactif={!sessionOuverte}
          />
          <KpiPetit
            titre="Sorties"
            valeur={`- ${fmt(resume.total_sorties)}`}
            icone={TrendingDown}
            variante="neutral"
            inactif={!sessionOuverte}
          />
          <KpiPetit
            titre="Solde théorique"
            valeur={fmt(resume.solde_theorique)}
            icone={Wallet}
            variante="tinted"
            inactif={!sessionOuverte}
          />
        </div>
      )}

      {/* Mouvements du jour */}
      <Card className="relative z-[1] rounded-none bg-card/70">
        <CardHeader className="pb-3">
          <CardTitle className="text-sm flex items-center gap-2">
            <Wallet className="h-4 w-4" />
            Mouvements du jour ({mouvements.length})
          </CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {mouvements.length === 0 ? (
            <p className="text-sm text-muted-foreground text-center py-8">
              Aucun mouvement aujourd'hui
            </p>
          ) : (
            <div className="divide-y divide-border">
              {mouvements.map(m => (
                <div key={m.id}
                  className="flex items-center justify-between px-4 py-2.5 hover:bg-muted/40">
                  <div className="min-w-0">
                    <p className="text-sm font-medium truncate">
                      {m.libelle || fmtMotif(m.motif)}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {fmtHeure(m.date_mouvement)} · {fmtMoyen(m.moyen)}
                      {m.libelle && ` · ${fmtMotif(m.motif)}`}
                    </p>
                  </div>
                  <span className={`text-sm font-semibold ${
                    m.sens === "entree" ? "text-green-600" : "text-red-500"
                  }`}>
                    {m.sens === "entree" ? "+" : "-"}{fmt(m.montant)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      </>)}

      <ModalDepense
        ouvert={modalDepense}
        onAnnuler={() => setModalDepense(false)}
        onEnregistre={async () => {
          setModalDepense(false);
          await charger();
        }} />

      <ModalOuvertureSession
        ouvert={modalOuverture}
        onFermer={() => setModalOuverture(false)}
        onOuvrir={handleApresOuverture} />

      <ModalFermetureSession
        ouvert={modalFermeture}
        resume={resume}
        onAnnuler={() => setModalFermeture(false)}
        onCloturer={handleApresFermeture} />
    </div>
  );
}