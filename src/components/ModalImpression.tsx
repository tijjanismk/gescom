import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Printer, Loader2, FileText } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { message } from "@tauri-apps/plugin-dialog";
import { genererFactureHTML } from "@/lib/genererFacture";
import type { FormatImpression } from "@/lib/genererFacture";

interface ModalImpressionProps {
  ouvert: boolean;
  venteId: string | null;
  onFermer: () => void;
}

// Le type vit desormais dans genererFacture.ts — une seule definition,
// sinon ajouter un format ici ne suffit pas a le rendre imprimable.
const FORMATS: { value: FormatImpression; label: string; desc: string }[] = [
  { value: "a4",           label: "A4",             desc: "Facture classique — imprimante normale" },
  { value: "a5",           label: "A5",             desc: "Demi-page — économie de papier" },
  { value: "thermique_58", label: "Thermique 58mm", desc: "Petit reçu — caisse enregistreuse" },
  { value: "thermique_80", label: "Thermique 80mm", desc: "Reçu standard — imprimante de reçus" },
];

export function ModalImpression({ ouvert, venteId, onFermer }: ModalImpressionProps) {
  const [format, setFormat] = useState<FormatImpression>("a4");
  const [chargement, setChargement] = useState(false);

  async function handleImprimer() {
    if (!venteId) return;
    setChargement(true);
    try {
      // 1. Charger les données depuis Rust
      const donnees = await invoke<any>("lire_donnees_facture", { venteId });

      // 2. Générer le HTML
      const html = genererFactureHTML(donnees, format);

      // 3. Passer le HTML à Rust qui écrit le fichier et ouvre le navigateur
      const chemin = await invoke<string>("imprimer_facture", {
        html,
        nomFichier: `gescom_${donnees.vente?.numero_facture ?? venteId}.html`,
      });

      console.log("Facture ouverte :", chemin);
      onFermer();

    } catch (e) {
      console.error("Erreur impression :", e);
      await message(
        `Erreur : ${typeof e === "string" ? e : JSON.stringify(e)}`,
        { title: "Erreur impression", kind: "error" }
      );
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Printer className="h-4 w-4" />
            Imprimer la facture
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4 pt-2">

          <div className="space-y-2">
            {FORMATS.map(f => (
              <button key={f.value} onClick={() => setFormat(f.value)}
                className={`
                  w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border-2
                  text-left transition-all
                  ${format === f.value
                    ? "border-primary bg-primary/5"
                    : "border-border hover:border-muted-foreground"}
                `}>
                <FileText className={`h-4 w-4 shrink-0 ${
                  format === f.value ? "text-primary" : "text-muted-foreground"
                }`} />
                <div>
                  <p className={`text-sm font-medium ${format === f.value ? "text-primary" : ""}`}>
                    {f.label}
                  </p>
                  <p className="text-xs text-muted-foreground">{f.desc}</p>
                </div>
              </button>
            ))}
          </div>

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Passer
            </Button>
            <Button onClick={handleImprimer}
              disabled={chargement || !venteId} className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Printer className="h-4 w-4 mr-2" /> Imprimer</>
              }
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}