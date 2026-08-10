import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Wallet, TrendingUp, TrendingDown, Loader2, AlertTriangle } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { message } from "@tauri-apps/plugin-dialog";

interface SessionCaisse {
  id: string;
  fond_initial: number;
  date_ouverture: string;
  statut: string;
  montant_compte?: number;
  ecart?: number;
}

interface MouvementCaisse {
  id: string;
  sens: string;
  moyen: string;
  montant: number;
  motif: string;
  date_mouvement: string;
}

interface ResumeCaisse {
  session?: SessionCaisse;
  mouvements: MouvementCaisse[];
  total_entrees_especes: number;
  total_entrees_mobile: number;
  total_sorties: number;
  solde_theorique_especes: number;
}

function formaterMontant(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

function formaterHeure(iso: string): string {
  return new Date(iso).toLocaleTimeString("fr-ML", {
    hour: "2-digit", minute: "2-digit"
  });
}

export function Caisse() {
  const [resume, setResume] = useState<ResumeCaisse | null>(null);
  const [chargement, setChargement] = useState(true);
  const [fondInitial, setFondInitial] = useState("10000");
  const [montantCompte, setMontantCompte] = useState("");
  const [ouvertureEnCours, setOuvertureEnCours] = useState(false);
  const [fermetureEnCours, setFermetureEnCours] = useState(false);

  async function charger() {
    try {
      const data = await invoke<ResumeCaisse>("lire_resume_caisse");
      setResume(data);
    } catch (e) {
      console.error("Erreur caisse :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  async function handleOuvrir() {
    setOuvertureEnCours(true);
    try {
      await invoke("ouvrir_session_caisse", {
        fondInitial: parseInt(fondInitial) || 0,
      });
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setOuvertureEnCours(false);
    }
  }

  async function handleFermer() {
    if (!montantCompte) {
      await message("Saisir le montant compté", { title: "Attention", kind: "warning" });
      return;
    }
    setFermetureEnCours(true);
    try {
      const ecart = await invoke<number>("fermer_session_caisse", {
        montantCompte: parseInt(montantCompte),
      });
      await charger();
      const msg = ecart === 0
        ? "Caisse fermée — aucun écart ✓"
        : ecart > 0
          ? `Caisse fermée — excédent de ${formaterMontant(ecart)}`
          : `Caisse fermée — manque de ${formaterMontant(Math.abs(ecart))}`;
      await message(msg, { title: "Fermeture de caisse", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setFermetureEnCours(false);
    }
  }

  if (chargement) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const sessionOuverte = resume?.session?.statut === "ouverte";

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Caisse</h1>
        <Badge variant={sessionOuverte ? "default" : "secondary"}>
          {sessionOuverte ? "Session ouverte" : "Session fermée"}
        </Badge>
      </div>

      {/* Pas de session — formulaire d'ouverture */}
      {!sessionOuverte && (
        <Card className="max-w-sm mb-6">
          <CardHeader>
            <CardTitle className="text-sm">Ouvrir la caisse</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div>
              <Label>Fond de caisse initial (F)</Label>
              <Input type="number" value={fondInitial}
                onChange={e => setFondInitial(e.target.value)}
                className="mt-1" />
            </div>
            <Button onClick={handleOuvrir} disabled={ouvertureEnCours} className="w-full">
              {ouvertureEnCours
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : "Ouvrir la caisse"
              }
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Session ouverte — résumé */}
      {sessionOuverte && resume && (
        <>
          {/* Indicateurs */}
          <div className="grid grid-cols-2 gap-4 mb-6">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-xs text-muted-foreground flex items-center gap-1">
                  <TrendingUp className="h-3 w-3 text-green-500" />
                  Entrées espèces
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-xl font-bold text-green-600">
                  {formaterMontant(resume.total_entrees_especes)}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-xs text-muted-foreground flex items-center gap-1">
                  <Wallet className="h-3 w-3 text-blue-500" />
                  Mobile Money
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-xl font-bold text-blue-600">
                  {formaterMontant(resume.total_entrees_mobile)}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-xs text-muted-foreground flex items-center gap-1">
                  <TrendingDown className="h-3 w-3 text-red-500" />
                  Sorties
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-xl font-bold text-red-600">
                  {formaterMontant(resume.total_sorties)}
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-xs text-muted-foreground flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3 text-orange-500" />
                  Solde théorique espèces
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-xl font-bold">
                  {formaterMontant(resume.solde_theorique_especes)}
                </p>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Fond {formaterMontant(resume.session?.fond_initial ?? 0)} + entrées - sorties
                </p>
              </CardContent>
            </Card>
          </div>

          {/* Fermeture */}
          <Card className="mb-6">
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Fermer la caisse</CardTitle>
            </CardHeader>
            <CardContent className="flex items-end gap-3">
              <div className="flex-1">
                <Label className="text-xs">Montant compté dans le tiroir (F)</Label>
                <Input type="number" value={montantCompte}
                  onChange={e => setMontantCompte(e.target.value)}
                  placeholder="0" className="mt-1" />
              </div>
              {montantCompte && (
                <div className="text-right pb-0.5">
                  <p className="text-xs text-muted-foreground">Écart estimé</p>
                  <p className={`text-sm font-semibold ${
                    parseInt(montantCompte) >= resume.solde_theorique_especes
                      ? "text-green-600" : "text-red-600"
                  }`}>
                    {formaterMontant(parseInt(montantCompte) - resume.solde_theorique_especes)}
                  </p>
                </div>
              )}
              <Button onClick={handleFermer} disabled={fermetureEnCours} variant="outline">
                {fermetureEnCours
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : "Fermer"
                }
              </Button>
            </CardContent>
          </Card>

          {/* Mouvements du jour */}
          <Card>
            <CardHeader className="pb-3">
              <CardTitle className="text-sm">Mouvements du jour</CardTitle>
            </CardHeader>
            <CardContent>
              {resume.mouvements.length === 0 ? (
                <p className="text-sm text-muted-foreground text-center py-6">
                  Aucun mouvement
                </p>
              ) : (
                <div className="space-y-1">
                  {resume.mouvements.map(m => (
                    <div key={m.id}
                      className="flex items-center justify-between py-2 px-3 rounded-md hover:bg-muted/40">
                      <div>
                        <p className="text-sm font-medium capitalize">
                          {m.motif.replace(/_/g, " ")}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {m.moyen.replace(/_/g, " ")} · {formaterHeure(m.date_mouvement)}
                        </p>
                      </div>
                      <span className={`text-sm font-semibold ${
                        m.sens === "entree" ? "text-green-600" : "text-red-600"
                      }`}>
                        {m.sens === "entree" ? "+" : "-"}{formaterMontant(m.montant)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
