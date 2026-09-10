import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import {
  Printer, Loader2, FileText, PackageCheck, LayoutTemplate,
} from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { message } from "@tauri-apps/plugin-dialog";
import { genererImpression } from "@/lib/genererPDF";
import type { FormatImpression, DonneesPiece } from "@/lib/genererPDF";
import { rendreModele } from "@/lib/modeles/rendu";
import { contextePiece } from "@/lib/modeles/contexte";
import type { Modele } from "@/lib/modeles/types";

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

  // Les modeles de l'atelier viennent s'ajouter aux formats d'origine.
  // Ils ne les REMPLACENT pas : le generateur historique imprime la
  // meme facture depuis des mois, et le jour ou l'on bascule doit etre
  // choisi par le commercant, apres avoir vu son modele a l'ecran.
  const [modeles, setModeles] = useState<Modele[]>([]);
  const [modeleChoisi, setModeleChoisi] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("lire_config_bon_sortie")
      .then(setBonSortieActif)
      .catch(() => setBonSortieActif(false));
    invoke<Modele[]>("lire_modeles", { genre: "facture" })
      .then(setModeles)
      .catch(() => setModeles([]));
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
      const [logo, entete, pied, signatures] = await Promise.all([
        invoke<string | null>("lire_logo_base64").catch(() => null),
        invoke<string | null>("lire_entete_base64").catch(() => null),
        invoke<string | null>("lire_pied_base64").catch(() => null),
        invoke<any>("lire_config_signatures").catch(() => null),
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

      const modele = modeleChoisi
        ? modeles.find(m => m.id === modeleChoisi) ?? null
        : null;

      const html = modele
        ? rendreModele(modele, contextePiece(donnees as any), {
            images: { logo, entete, pied },
          })
        : genererImpression(
            donnees, formatFinal, logo, entete, pied, signatures);

      await invoke<string>("imprimer_facture", {
        html,
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
              <button key={f.value}
                onClick={() => { setFormat(f.value); setModeleChoisi(null); }}
                className={`
                  w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border-2
                  text-left transition-all
                  ${format === f.value && !modeleChoisi
                    ? "border-primary bg-primary/5"
                    : "border-border hover:border-muted-foreground"}
                `}>
                <FileText className={`h-4 w-4 shrink-0 ${
                  format === f.value && !modeleChoisi
                    ? "text-primary" : "text-muted-foreground"
                }`} />
                <div>
                  <p className={`text-sm font-medium ${
                    format === f.value && !modeleChoisi ? "text-primary" : ""}`}>
                    {f.label}
                  </p>
                  <p className="text-xs text-muted-foreground">{f.desc}</p>
                </div>
              </button>
            ))}

            {modeles.length > 0 && (
              <>
                <p className="pt-1 text-[11px] font-semibold uppercase
                              tracking-wide text-muted-foreground">
                  Mes modèles
                </p>
                {modeles.map(m => (
                  <button key={m.id} onClick={() => setModeleChoisi(m.id)}
                    className={`
                      w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border-2
                      text-left transition-all
                      ${modeleChoisi === m.id
                        ? "border-primary bg-primary/5"
                        : "border-border hover:border-muted-foreground"}
                    `}>
                    <LayoutTemplate className={`h-4 w-4 shrink-0 ${
                      modeleChoisi === m.id ? "text-primary" : "text-muted-foreground"
                    }`} />
                    <div>
                      <p className={`text-sm font-medium ${
                        modeleChoisi === m.id ? "text-primary" : ""}`}>
                        {m.nom}{m.actif ? " ★" : ""}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Atelier des modèles — {m.format.replace("_", " ")}
                      </p>
                    </div>
                  </button>
                ))}
              </>
            )}
          </div>

          {/* Case affichée seulement si le réglage est actif : inutile
              là où le vendeur remet lui-même la marchandise. */}
          {/* Seulement en A4/A5 : le bon ne se greffe pas sur un ticket
              thermique, et une case ignorée en silence est pire que pas
              de case. */}
          {bonSortieActif && !modeleChoisi
            && (format === "a4" || format === "a5") && (
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
