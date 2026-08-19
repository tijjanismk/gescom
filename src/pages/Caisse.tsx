import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Wallet, RefreshCw, Loader2, TrendingUp, TrendingDown,
  Lock, Unlock, AlertTriangle, CheckCircle2
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { message } from "@tauri-apps/plugin-dialog";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface ResumeCaisse {
  session_id: string | null;
  statut: "ouverte" | "fermee" | "aucune";
  fond_ouverture: number;
  total_entrees: number;
  total_sorties: number;
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
//  Modal : Fermeture de session (rapprochement)
// =====================================================================

function ModalFermetureSession({
  ouvert, resume, onFermer, onFermer2,
}: {
  ouvert: boolean;
  resume: ResumeCaisse | null;
  onFermer: () => void;
  onFermer2: () => void;
}) {
  const [espècesComptees, setEspecesComptees] = useState("");
  const [chargement, setChargement] = useState(false);

  const montantCompte = parseMontant(espècesComptees);
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
      onFermer2();
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
              <span>Entrées</span>
              <span>+ {fmt(resume?.total_entrees ?? 0)}</span>
            </div>
            <div className="flex justify-between text-sm text-red-500">
              <span>Sorties</span>
              <span>- {fmt(resume?.total_sorties ?? 0)}</span>
            </div>
            <div className="flex justify-between text-sm font-bold border-t border-border pt-2">
              <span>Solde théorique espèces</span>
              <span>{fmt(resume?.solde_theorique ?? 0)}</span>
            </div>
          </div>

          {/* Rapprochement */}
          <div>
            <Label>Espèces comptées physiquement (F)</Label>
            <MoneyInput value={espècesComptees} onChange={setEspecesComptees}
              placeholder="0" className="mt-1" autoFocus />
          </div>

          {espècesComptees && (
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
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button onClick={handleFermer}
              disabled={!espècesComptees || chargement}
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
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Caisse</h1>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={charger}>
            <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
          </Button>
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

      {/* Statut de la session */}
      {resume?.statut === "aucune" && (
        <div className="flex items-center gap-3 p-4 bg-muted rounded-lg mb-6">
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
        <div className="flex items-center gap-3 p-3 bg-green-50 dark:bg-green-950/20
          border border-green-200 rounded-lg mb-6 text-sm">
          <CheckCircle2 className="h-4 w-4 text-green-600 shrink-0" />
          <span className="text-green-700 dark:text-green-400">
            Session ouverte le {fmtDate(resume.ouvert_le!)}
            {" "}— fond : {fmt(resume.fond_ouverture)}
          </span>
        </div>
      )}

      {/* KPIs */}
      {resume && resume.statut !== "aucune" && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
          <Card>
            <CardContent className="pt-4">
              <p className="text-xs text-muted-foreground">Fond ouverture</p>
              <p className="text-lg font-bold mt-1">{fmt(resume.fond_ouverture)}</p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <p className="text-xs text-green-600 flex items-center gap-1">
                <TrendingUp className="h-3 w-3" /> Entrées
              </p>
              <p className="text-lg font-bold mt-1 text-green-600">
                + {fmt(resume.total_entrees)}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <p className="text-xs text-red-500 flex items-center gap-1">
                <TrendingDown className="h-3 w-3" /> Sorties
              </p>
              <p className="text-lg font-bold mt-1 text-red-500">
                - {fmt(resume.total_sorties)}
              </p>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="pt-4">
              <p className="text-xs text-muted-foreground">Solde théorique</p>
              <p className="text-lg font-bold mt-1">{fmt(resume.solde_theorique)}</p>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Mouvements du jour */}
      <Card>
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
                  <div>
                    <p className="text-sm font-medium">{fmtMotif(m.motif)}</p>
                    <p className="text-xs text-muted-foreground">
                      {fmtHeure(m.date_mouvement)} · {fmtMoyen(m.moyen)}
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

      <ModalOuvertureSession
        ouvert={modalOuverture}
        onFermer={() => setModalOuverture(false)}
        onOuvrir={handleApresOuverture} />

      <ModalFermetureSession
        ouvert={modalFermeture}
        resume={resume}
        onFermer={() => setModalFermeture(false)}
        onFermer2={handleApresFermeture} />
    </div>
  );
}
