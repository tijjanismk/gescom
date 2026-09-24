// equipe/ApercuBulletin.tsx — l'aperçu du bulletin de paie (G-3), qui est
// aussi ce qui s'imprime.
//
// Équipe n'a pas la commande d'impression de la caisse (sa coque ne
// sert que l'adresse du serveur) : on imprime l'iframe elle-même.
// `allow-modals` autorise la boîte d'impression ; sans `allow-scripts`,
// rien du document ne s'exécute.

import { useEffect, useRef, useState } from "react";
import { Loader2, Printer, X } from "lucide-react";
import { appeler as invoke } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import { chargerHabillage } from "@/lib/impression";
import { genererBulletinHTML, type DonneesBulletin } from "@/lib/genererBulletin";

export function ApercuBulletin({ ficheId, fermer }: { ficheId: string; fermer: () => void }) {
  const [html, setHtml] = useState("");
  const [erreur, setErreur] = useState<string | null>(null);
  const [a5, setA5] = useState(false);
  const cadre = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    let annule = false;
    Promise.all([invoke<DonneesBulletin>("lire_donnees_bulletin", { ficheId }), chargerHabillage("bulletin")])
      .then(([d, h]) => {
        if (annule) return;
        setA5(h.reglage?.format === "a5");
        setHtml(genererBulletinHTML(d, h));
      })
      .catch(e => { if (!annule) setErreur(String(e)); });
    return () => { annule = true; };
  }, [ficheId]);

  return (
    <div className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center p-4" role="dialog" aria-label="Bulletin de paie">
      <div className="bg-background rounded-lg shadow-xl flex flex-col" style={{ width: a5 ? 640 : 860, maxWidth: "96vw", height: "90vh" }}>
        <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
          <h3 className="font-semibold flex-1">Bulletin de paie</h3>
          <Button size="sm" disabled={!html} onClick={() => cadre.current?.contentWindow?.print()}>
            <Printer className="h-4 w-4 mr-1" /> Imprimer
          </Button>
          <Button size="sm" variant="ghost" aria-label="Fermer le bulletin" onClick={fermer}><X className="h-4 w-4" /></Button>
        </div>
        <div className="flex-1 overflow-auto bg-muted/40 p-4">
          {erreur ? <p className="text-sm text-red-600" role="status">{erreur}</p>
            : !html ? <Loader2 className="h-6 w-6 animate-spin text-muted-foreground mx-auto" />
            : (
              <iframe ref={cadre} title="Bulletin de paie" srcDoc={html} sandbox="allow-same-origin allow-modals"
                style={{ width: a5 ? 559 : 794, height: a5 ? 794 : 1123, border: "none", background: "#fff", display: "block", margin: "0 auto", boxShadow: "0 1px 8px rgba(0,0,0,.15)" }} />
            )}
        </div>
      </div>
    </div>
  );
}
