// pages/Cheques.tsx
//
// Un chèque est une promesse, pas de l'argent. Il n'entre pas dans le
// rapprochement de caisse — comme le mobile money, il n'est pas dans le
// tiroir.
//
// Cycle :  reçu → déposé → encaissé
//                     ↘ rejeté  (le paiement est annulé, la créance rouvre)

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message, confirm } from "@tauri-apps/plugin-dialog";
import {
  FileCheck, Loader2, RefreshCw, AlertTriangle, Clock,
  Landmark, CheckCircle2, XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

interface Cheque {
  id: string; numero: string; banque: string; tireur: string;
  montant: number; date_emission: string | null;
  date_echeance: string | null; statut: string;
  cree_le: string; client: string; jours_detention: number;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}
function fmtDate(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleDateString("fr-ML", {
    day: "2-digit", month: "2-digit", year: "numeric",
  });
}

const STATUTS = [
  { value: "tous",     label: "Tous" },
  { value: "recu",     label: "Reçus" },
  { value: "depose",   label: "Déposés" },
  { value: "encaisse", label: "Encaissés" },
  { value: "rejete",   label: "Rejetés" },
];

function Badge({ statut }: { statut: string }) {
  const style = {
    recu:     "bg-blue-100 text-blue-700",
    depose:   "bg-orange-100 text-orange-700",
    encaisse: "bg-green-100 text-green-700",
    rejete:   "bg-red-100 text-red-700",
  }[statut] ?? "bg-muted text-muted-foreground";
  const label = {
    recu: "Reçu", depose: "Déposé",
    encaisse: "Encaissé", rejete: "Rejeté",
  }[statut] ?? statut;
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${style}`}>
      {label}
    </span>
  );
}

export function Cheques() {
  const [cheques, setCheques] = useState<Cheque[]>([]);
  const [total, setTotal] = useState(0);
  const [dormants, setDormants] = useState(0);
  const [filtre, setFiltre] = useState("tous");
  const [chargement, setChargement] = useState(true);

  const charger = useCallback(async () => {
    setChargement(true);
    try {
      const r = await invoke<{
        cheques: Cheque[]; total_en_attente: number; nb_dormants: number;
      }>("lire_cheques", { statut: filtre });
      setCheques(r.cheques);
      setTotal(r.total_en_attente);
      setDormants(r.nb_dormants);
    } catch (e) {
      console.error("Erreur chèques :", e);
    } finally {
      setChargement(false);
    }
  }, [filtre]);

  useEffect(() => { charger(); }, [charger]);

  async function changer(ch: Cheque, statut: string) {
    // Le rejet fait disparaître de l'argent déjà compté : on prévient.
    if (statut === "rejete") {
      const ok = await confirm(
        `Rejeter le chèque n° ${ch.numero} de ${fmt(ch.montant)} ?\n\n` +
        `Le paiement sera annulé et la créance du client se rouvrira. ` +
        `C'est le seul cas où de l'argent déjà encaissé disparaît.`,
        { title: "Chèque rejeté", kind: "warning" },
      );
      if (!ok) return;
    }
    try {
      const r = await invoke<{ creance_rouverte: boolean }>(
        "changer_statut_cheque", { chequeId: ch.id, statut, motif: null });
      if (r.creance_rouverte) {
        await message(
          `Créance rouverte pour ${ch.client}.\n` +
          `Le client apparaît de nouveau comme devant ${fmt(ch.montant)}.`,
          { title: "Créance rouverte", kind: "warning" });
      }
      await charger();
    } catch (e) {
      await message(`${e}`, { title: "Erreur", kind: "error" });
    }
  }

  if (chargement && cheques.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-6">

      <div className="flex flex-wrap items-center justify-between gap-3 mb-6">
        <div className="flex items-center gap-2">
          <FileCheck className="h-5 w-5" />
          <h1 className="text-2xl font-semibold">Chèques</h1>
        </div>
        <Button variant="outline" size="sm" onClick={charger}>
          <RefreshCw className="h-4 w-4 mr-2" /> Actualiser
        </Button>
      </div>

      {/* ── Indicateurs ── */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-6">
        <Card>
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground flex items-center gap-1">
              <Landmark className="h-3 w-3" /> En attente d'encaissement
            </p>
            <p className="text-2xl font-bold mt-1">{fmt(total)}</p>
            <p className="text-xs text-muted-foreground mt-1">
              Cet argent n'est pas dans le tiroir et n'entre pas dans le
              rapprochement de caisse.
            </p>
          </CardContent>
        </Card>

        <Card className={dormants > 0 ? "border-orange-300" : ""}>
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground flex items-center gap-1">
              <Clock className="h-3 w-3" /> Non déposés depuis 15 jours
            </p>
            <p className={`text-2xl font-bold mt-1 ${
              dormants > 0 ? "text-orange-600" : ""}`}>{dormants}</p>
            <p className="text-xs text-muted-foreground mt-1">
              Un chèque gardé trop longtemps perd sa valeur de recours.
            </p>
          </CardContent>
        </Card>
      </div>

      {/* ── Filtres ── */}
      <div className="flex flex-wrap gap-2 mb-4">
        {STATUTS.map(s => (
          <button key={s.value} onClick={() => setFiltre(s.value)}
            className={`px-3 py-1.5 rounded-full text-xs font-medium border
              ${filtre === s.value
                ? "bg-foreground text-background border-foreground"
                : "border-border hover:bg-muted"}`}>
            {s.label}
          </button>
        ))}
      </div>

      {/* ── Liste ── */}
      {cheques.length === 0 ? (
        <p className="text-sm text-muted-foreground py-8 text-center">
          Aucun chèque {filtre !== "tous" && "dans cet état"}.
        </p>
      ) : (
        <div className="border border-border rounded-lg divide-y divide-border">
          {cheques.map(ch => (
            <div key={ch.id}
              className="flex flex-wrap items-center gap-3 px-4 py-3 hover:bg-muted/30">
              <div className="flex-1 min-w-[180px]">
                <p className="text-sm font-medium flex items-center gap-2">
                  <span className="font-mono">n° {ch.numero}</span>
                  <Badge statut={ch.statut} />
                  {ch.statut === "recu" && ch.jours_detention > 15 && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded
                                     bg-orange-100 text-orange-700
                                     flex items-center gap-1">
                      <AlertTriangle className="h-2.5 w-2.5" />
                      {ch.jours_detention} j
                    </span>
                  )}
                </p>
                <p className="text-xs text-muted-foreground">
                  {ch.banque}
                  {ch.tireur && ` · ${ch.tireur}`}
                  {ch.client !== "—" && ` · ${ch.client}`}
                  {ch.date_echeance && ` · échéance ${fmtDate(ch.date_echeance)}`}
                </p>
              </div>

              <div className="text-right min-w-[110px]">
                <p className="text-sm font-semibold">{fmt(ch.montant)}</p>
                <p className="text-xs text-muted-foreground">
                  reçu le {fmtDate(ch.cree_le)}
                </p>
              </div>

              <div className="flex gap-1">
                {ch.statut === "recu" && (
                  <Button variant="outline" size="sm"
                    onClick={() => changer(ch, "depose")}>
                    Déposer
                  </Button>
                )}
                {ch.statut === "depose" && (
                  <Button size="sm" onClick={() => changer(ch, "encaisse")}>
                    <CheckCircle2 className="h-3.5 w-3.5 mr-1" /> Encaissé
                  </Button>
                )}
                {(ch.statut === "recu" || ch.statut === "depose") && (
                  <Button variant="outline" size="sm"
                    onClick={() => changer(ch, "rejete")}
                    className="text-destructive border-destructive/30
                               hover:bg-destructive/10">
                    <XCircle className="h-3.5 w-3.5 mr-1" /> Rejeté
                  </Button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      <p className="text-xs text-muted-foreground mt-4">
        Un chèque rejeté annule le paiement : la créance du client se
        rouvre et la facture repasse en « émis ».
      </p>
    </div>
  );
}
