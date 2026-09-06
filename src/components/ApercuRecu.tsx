// components/ApercuRecu.tsx — Aperçu et impression d'un reçu de règlement
//
// Même mécanique qu'ApercuPiece : le document affiché EST celui qui
// s'imprime, rendu dans une iframe depuis le HTML du générateur. Le
// `sandbox` sans `allow-scripts` neutralise le window.print() embarqué
// dans le document, qui sinon ouvrirait la boîte d'impression à
// l'ouverture de l'aperçu.
//
// Pas de sélecteur de format ici : un reçu est un A5, point.

import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { Printer, Loader2, Eye, AlertTriangle } from "lucide-react";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { genererRecuHTML } from "@/lib/genererRecu";
import type { DonneesRecu } from "@/lib/genererRecu";

interface Props {
  /** `null` ferme l'aperçu. */
  paiementId: string | null;
  cote: "client" | "fournisseur";
  onFermer: () => void;
}

// A5 à 96 dpi : 148 mm de large.
const LARGEUR_A5 = 559;
const HAUTEUR_A5 = 794;

export function ApercuRecu({ paiementId, cote, onFermer }: Props) {
  const [html, setHtml] = useState("");
  const [chargement, setChargement] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);
  const [impression, setImpression] = useState(false);
  const [hauteur, setHauteur] = useState(HAUTEUR_A5);
  const [nom, setNom] = useState("");
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const ouvert = paiementId !== null;

  useEffect(() => {
    if (!ouvert) return;
    let annule = false;

    setChargement(true);
    setErreur(null);

    Promise.all([
      invoke<DonneesRecu>("lire_donnees_recu", { paiementId, cote }),
      invoke<string | null>("lire_logo_base64").catch(() => null),
      invoke<string | null>("lire_entete_base64").catch(() => null),
    ])
      .then(([d, logo, entete]) => {
        if (annule) return;
        setNom(d.tiers?.nom ?? "");
        setHtml(genererRecuHTML(d, logo, entete));
      })
      .catch(e => {
        if (!annule) setErreur(typeof e === "string" ? e : JSON.stringify(e));
      })
      .finally(() => { if (!annule) setChargement(false); });

    return () => { annule = true; };
  }, [paiementId, cote, ouvert]);

  // `allow-same-origin` autorise la lecture pour mesurer, sans jamais
  // permettre l'exécution : c'est `allow-scripts` qui l'aurait fait.
  const mesurer = useCallback(() => {
    const h = iframeRef.current?.contentDocument?.body?.scrollHeight;
    setHauteur(h && h > 50 ? h : HAUTEUR_A5);
  }, []);

  async function handleImprimer() {
    if (!html) return;
    setImpression(true);
    try {
      await invoke("imprimer_facture", {
        html,
        nomFichier: `recu_${nom || "reglement"}`
          .replace(/[\\/:*?"<>|]/g, "-") + ".html",
      });
      onFermer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Impression", kind: "error" });
    } finally {
      setImpression(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={o => { if (!o) onFermer(); }}>
      {/* D22 : shadcn ignore max-w-* sur DialogContent, d'où le style
          inline. */}
      <DialogContent
        style={{ width: "660px", maxWidth: "96vw", height: "88vh" }}
        className="flex flex-col p-0 gap-0">
        <DialogHeader className="px-4 py-3 border-b border-border shrink-0">
          <DialogTitle className="flex items-center gap-2 text-base">
            <Eye className="h-4 w-4" />
            Reçu de règlement
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-auto bg-muted/40 p-4">
          {chargement ? (
            <div className="flex items-center justify-center h-full">
              <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          ) : erreur ? (
            <div className="flex flex-col items-center justify-center h-full
                            gap-2 text-center px-6">
              <AlertTriangle className="h-8 w-8 text-orange-500" />
              <p className="text-sm font-medium">Reçu indisponible</p>
              <p className="text-xs text-muted-foreground">{erreur}</p>
            </div>
          ) : (
            <iframe
              ref={iframeRef}
              title="Aperçu du reçu"
              srcDoc={html}
              onLoad={mesurer}
              sandbox="allow-same-origin"
              style={{
                width: LARGEUR_A5, height: hauteur, border: "none",
                background: "#fff", display: "block", margin: "0 auto",
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
            disabled={!html || impression || chargement} className="flex-1">
            {impression
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : <><Printer className="h-4 w-4 mr-2" /> Imprimer le reçu</>}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
