import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Printer, Loader2, FileText, PackageCheck } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { message } from "@tauri-apps/plugin-dialog";
import { genererImpression } from "@/lib/genererPDF";
import type { FormatImpression, DonneesPiece } from "@/lib/genererPDF";

interface ModalImpressionProps {
  ouvert: boolean;
  venteId: string | null;
  onFermer: () => void;
}

const FORMATS: { value: FormatImpression; label: string; desc: string }[] = [
  { value: "a4",           label: "A4",             desc: "Facture classique — imprimante normale" },
  { value: "a5",           label: "A5",             desc: "Demi-page — économie de papier" },
  { value: "thermique_58", label: "Thermique 58mm", desc: "Petit reçu — caisse enregistreuse" },
  { value: "thermique_80", label: "Thermique 80mm", desc: "Reçu standard — imprimante de reçus" },
];

export function ModalImpression({ ouvert, venteId, onFermer }: ModalImpressionProps) {
  const [format, setFormat] = useState<FormatImpression>("a4");
  const [chargement, setChargement] = useState(false);
  // Bon de sortie : magasin séparé de la caisse. Le client repart avec
  // ce papier, le magasinier délivre contre lui.
  const [bonSortieActif, setBonSortieActif] = useState(false);
  const [avecBonSortie, setAvecBonSortie] = useState(true);

  useEffect(() => {
    invoke<boolean>("lire_config_bon_sortie")
      .then(setBonSortieActif)
      .catch(() => setBonSortieActif(false));
  }, []);

  async function handleImprimer() {
    if (!venteId) return;
    setChargement(true);
    try {
      // La vente et l'écran Pièces désignent le MÊME document depuis que
      // la table `facture` legacy n'est plus alimentée. On lit donc la
      // même source, et un seul générateur produit le HTML.
      const pieceId = await invoke<string | null>("lire_piece_de_vente", {
        venteId,
      });

      if (!pieceId) {
        await message(
          "Aucune facture n'est liée à cette vente. " +
          "Elle a probablement échoué à la création — voir la console.",
          { title: "Facture introuvable", kind: "warning" },
        );
        return;
      }

      const donnees = await invoke<DonneesPiece>("lire_donnees_piece", {
        pieceId,
      });
      const [logo, entete, pied] = await Promise.all([
        invoke<string | null>("lire_logo_base64").catch(() => null),
        invoke<string | null>("lire_entete_base64").catch(() => null),
        invoke<string | null>("lire_pied_base64").catch(() => null),
      ]);

      const numero = donnees.piece?.numero ?? venteId;

      // Un seul document, une seule boîte de dialogue : `imprimer_facture`
      // ferme toute fenêtre d'impression avant d'ouvrir la sienne, donc
      // enchaîner deux impressions tuait la première.
      const avecBon = bonSortieActif && avecBonSortie
        && (format === "a4" || format === "a5");
      const formatFinal: FormatImpression = avecBon
        ? (format === "a5" ? "a5_et_bon" : "a4_et_bon")
        : format;

      await invoke<string>("imprimer_facture", {
        html: genererImpression(donnees, formatFinal, logo, entete, pied),
        nomFichier: `gescom_${numero}.html`,
      });

      onFermer();

    } catch (e) {
      console.error("Erreur impression :", e);
      await message(
        `Erreur : ${typeof e === "string" ? e : JSON.stringify(e)}`,
        { title: "Erreur impression", kind: "error" },
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

          {/* Case affichée seulement si le réglage est actif : inutile
              là où le vendeur remet lui-même la marchandise. */}
          {/* Seulement en A4/A5 : le bon ne se greffe pas sur un ticket
              thermique, et une case ignorée en silence est pire que pas
              de case. */}
          {bonSortieActif && (format === "a4" || format === "a5") && (
            <label className="flex items-start gap-2.5 px-3 py-2.5 rounded-lg
                              border border-border cursor-pointer
                              hover:bg-muted/40 transition-colors">
              <input type="checkbox" checked={avecBonSortie}
                onChange={e => setAvecBonSortie(e.target.checked)}
                className="mt-0.5 w-4 h-4 rounded accent-primary shrink-0" />
              <div>
                <p className="text-sm font-medium flex items-center gap-1.5">
                  <PackageCheck className="h-3.5 w-3.5" />
                  Imprimer aussi le bon de sortie
                </p>
                <p className="text-xs text-muted-foreground">
                  Sans montants — à remettre au magasinier
                </p>
              </div>
            </label>
          )}

          <div className="flex gap-2">
            <Button variant="outline" onClick={onFermer} className="flex-1">
              Passer
            </Button>
            <Button onClick={handleImprimer}
              disabled={chargement || !venteId} className="flex-1">
              {chargement
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Printer className="h-4 w-4 mr-2" />
                    {bonSortieActif && avecBonSortie
                      && (format === "a4" || format === "a5")
                      ? "Imprimer les deux" : "Imprimer"}
                  </>
              }
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
