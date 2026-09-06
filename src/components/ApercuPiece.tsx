// components/ApercuPiece.tsx — Aperçu avant impression d'une pièce
//
// L'aperçu affiche LE document, pas une reconstitution : le HTML vient
// de `genererImpression`, exactement celui qui part à l'imprimante.
// Redessiner la pièce en JSX aurait donné deux rendus du même document,
// qui auraient divergé au premier changement de mise en page.
//
// Le document embarque un script `window.onload → window.print()`
// (genererPDF.ts). Dans une iframe il ouvrirait la boîte d'impression
// dès l'ouverture de l'aperçu : `sandbox` SANS `allow-scripts` le
// neutralise. `allow-same-origin` reste nécessaire pour mesurer la
// hauteur du contenu — il n'autorise aucun script à s'exécuter.

import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import {
  Printer, Loader2, Eye, PackageCheck, AlertTriangle,
} from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { genererImpression } from "@/lib/genererPDF";
import type { FormatImpression, DonneesPiece } from "@/lib/genererPDF";

interface ApercuPieceProps {
  /** `null` ferme l'aperçu. */
  pieceId: string | null;
  /** Sert à nommer le fichier imprimé. */
  numero?: string;
  /** Le bon de sortie n'a de sens que sur une facture. */
  typePiece?: string;
  onFermer: () => void;
}

// Largeur du papier en pixels CSS (96 dpi). L'aperçu rend le document à
// sa taille réelle : c'est ce qui permet de juger si une désignation
// longue tient sur la ligne avant de sortir la feuille.
const LARGEUR_PAPIER: Record<FormatImpression, number> = {
  a4: 794, a4_et_bon: 794, bon_sortie: 794,
  a5: 559, a5_et_bon: 559,
  thermique_58: 220, thermique_80: 302,
};

const HAUTEUR_DEFAUT: Record<FormatImpression, number> = {
  a4: 1123, a4_et_bon: 2246, bon_sortie: 1123,
  a5: 794, a5_et_bon: 1588,
  thermique_58: 700, thermique_80: 700,
};

const FORMATS: { value: FormatImpression; label: string }[] = [
  { value: "a4",           label: "A4" },
  { value: "a5",           label: "A5" },
  { value: "thermique_80", label: "Ticket 80 mm" },
  { value: "thermique_58", label: "Ticket 58 mm" },
];

export function ApercuPiece({
  pieceId, numero, typePiece, onFermer,
}: ApercuPieceProps) {
  const [donnees, setDonnees] = useState<DonneesPiece | null>(null);
  const [logo, setLogo] = useState<string | null>(null);
  const [entete, setEntete] = useState<string | null>(null);
  const [pied, setPied] = useState<string | null>(null);
  const [format, setFormat] = useState<FormatImpression>("a4");
  const [bonSortieActif, setBonSortieActif] = useState(false);
  const [chargement, setChargement] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);
  const [impression, setImpression] = useState(false);
  const [hauteur, setHauteur] = useState(HAUTEUR_DEFAUT.a4);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const ouvert = pieceId !== null;

  useEffect(() => {
    if (!ouvert) return;
    let annule = false;

    setChargement(true);
    setErreur(null);
    setFormat("a4");

    Promise.all([
      invoke<DonneesPiece>("lire_donnees_piece", { pieceId }),
      invoke<string | null>("lire_logo_base64").catch(() => null),
      invoke<string | null>("lire_entete_base64").catch(() => null),
      invoke<string | null>("lire_pied_base64").catch(() => null),
      invoke<boolean>("lire_config_bon_sortie").catch(() => false),
    ])
      .then(([d, l, e, p, bs]) => {
        if (annule) return;
        setDonnees(d); setLogo(l); setEntete(e); setPied(p);
        setBonSortieActif(bs);
      })
      .catch(e => {
        if (!annule) setErreur(typeof e === "string" ? e : JSON.stringify(e));
      })
      .finally(() => { if (!annule) setChargement(false); });

    return () => { annule = true; };
  }, [pieceId, ouvert]);

  // Le même appel que l'impression : ce qui est affiché EST ce qui sort.
  const html = useMemo(
    () => (donnees ? genererImpression(donnees, format, logo, entete, pied) : ""),
    [donnees, format, logo, entete, pied],
  );

  // Hauteur réelle du contenu. Sans mesure, un document de deux pages
  // (facture + bon) serait coupé au milieu.
  const mesurer = useCallback(() => {
    const doc = iframeRef.current?.contentDocument;
    // `allow-same-origin` autorise la lecture, mais le moteur peut la
    // refuser selon le contexte : repli sur la hauteur théorique.
    const h = doc?.body?.scrollHeight;
    setHauteur(h && h > 50 ? h : HAUTEUR_DEFAUT[format]);
  }, [format]);

  async function handleImprimer() {
    if (!html) return;
    setImpression(true);
    try {
      await invoke("imprimer_facture", {
        html,
        nomFichier: `${(numero ?? "piece").replace(/[\\/:*?"<>|]/g, "-")}`
          + `${format === "bon_sortie" ? "-BS" : ""}.html`,
      });
      onFermer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    } finally {
      setImpression(false);
    }
  }

  const formats: { value: FormatImpression; label: string }[] = [
    ...FORMATS,
    ...(bonSortieActif && typePiece === "facture"
      ? [{ value: "bon_sortie" as FormatImpression, label: "Bon de sortie" }]
      : []),
  ];

  return (
    <Dialog open={ouvert} onOpenChange={o => { if (!o) onFermer(); }}>
      {/* D22 : shadcn ignore max-w-* sur DialogContent, d'où le style
          inline. Même contournement que ModalNouvellePiece. */}
      <DialogContent
        style={{ width: "980px", maxWidth: "96vw", height: "92vh" }}
        className="flex flex-col p-0 gap-0">
        <DialogHeader className="px-4 py-3 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-base">
            <Eye className="h-4 w-4" />
            Aperçu {numero ?? ""}
          </DialogTitle>
        </DialogHeader>

        {/* Barre de formats — l'aperçu se met à jour, on ne choisit
            plus le format à l'aveugle. */}
        <div className="flex items-center gap-1.5 px-4 py-2 border-b border-border
                        shrink-0 flex-wrap">
          {formats.map(f => (
            <button key={f.value} onClick={() => setFormat(f.value)}
              className={`px-3 py-1.5 rounded-md border text-xs font-medium
                          transition-colors ${
                format === f.value
                  ? "border-primary bg-primary/5 text-primary"
                  : "border-border text-muted-foreground hover:bg-muted"
              }`}>
              {f.value === "bon_sortie" && (
                <PackageCheck className="h-3 w-3 mr-1 inline-block" />
              )}
              {f.label}
            </button>
          ))}
        </div>

        <div className="flex-1 overflow-auto bg-muted/40 p-4">
          {chargement ? (
            <div className="flex items-center justify-center h-full">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          ) : erreur ? (
            <div className="flex flex-col items-center justify-center h-full gap-2
                            text-center px-6">
              <AlertTriangle className="h-8 w-8 text-orange-500" />
              <p className="text-sm font-medium">Aperçu indisponible</p>
              <p className="text-xs text-muted-foreground">{erreur}</p>
            </div>
          ) : (
            <iframe
              ref={iframeRef}
              title="Aperçu de la pièce"
              srcDoc={html}
              onLoad={mesurer}
              // Pas de `allow-scripts` : le script d'impression du
              // document ne doit jamais s'exécuter ici.
              sandbox="allow-same-origin"
              style={{
                width: LARGEUR_PAPIER[format],
                height: hauteur,
                border: "none",
                background: "#fff",
                display: "block",
                margin: "0 auto",
                boxShadow: "0 1px 8px rgba(0,0,0,.15)",
              }}
            />
          )}
        </div>

        <div className="flex gap-2 px-4 py-3 border-t border-border shrink-0">
          <Button variant="outline" onClick={onFermer} className="flex-1">
            Fermer
          </Button>
          <Button onClick={handleImprimer}
            disabled={!html || impression || chargement}
            className="flex-1">
            {impression
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : <><Printer className="h-4 w-4 mr-2" /> Imprimer</>}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
