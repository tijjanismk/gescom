// equipe/WhatsApp.tsx — les boutons WhatsApp d'Équipe (PLAN-EQUIPE EQ-5, D51).
//
// Rien ne part tout seul : on ouvre WhatsApp sur la conversation, le
// texte deja ecrit, et la personne appuie sur Envoyer. Un numero douteux
// desactive le bouton et la raison s'affiche a cote (lib/whatsapp.ts).
//
// Dans la coque, le lien passe par le plugin opener, dont la permission
// n'ouvre que `https://wa.me/*` ; dans un navigateur, par window.open.

import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { MessageCircle } from "lucide-react";
import { appeler as invoke, estDansTauri } from "@/lib/pont";
import { Button } from "@/components/ui/button";
import {
  lireNumero, lienWhatsApp, raisonBulletin, texteBulletin, debutMessageClient, messageEcrit,
  type BulletinResume,
} from "@/lib/whatsapp";

/** Ouvre WhatsApp hors de la fenetre. Rejette avec une chaine. */
export async function ouvrirWhatsApp(lien: string): Promise<void> {
  if (estDansTauri()) {
    try {
      await openUrl(lien);
    } catch (e) {
      throw `Impossible d'ouvrir WhatsApp : ${e}`;
    }
    return;
  }
  window.open(lien, "_blank", "noopener");
}

// =====================================================================
//  Le bulletin (aperçu) — D51 § 3
// =====================================================================

type Bulletin = BulletinResume & { personne?: { telephone?: string | null } | null };

/**
 * Le resume du bulletin vers le telephone de l'employe. L'apercu n'a
 * ces donnees que si le serveur a accorde la lecture de la paie
 * (`lire_donnees_bulletin`, refusee sans paie:preparer ni paie:valider).
 * Rien n'est note : le personnel n'a pas de fil d'echanges (D34).
 */
export function BoutonBulletinWhatsApp({ d }: { d: Bulletin | null }) {
  const [erreur, setErreur] = useState<string | null>(null);
  if (!d) {
    return <Button size="sm" variant="outline" disabled><MessageCircle className="h-4 w-4 mr-1" /> WhatsApp</Button>;
  }
  const numero = lireNumero(d.personne?.telephone);
  const raison = raisonBulletin(d) ?? (numero.ok ? null : numero.raison);

  async function envoyer() {
    if (!numero.ok) return;
    setErreur(null);
    try {
      await ouvrirWhatsApp(lienWhatsApp(numero.chiffres, texteBulletin(d!)));
    } catch (e) {
      setErreur(String(e));
    }
  }

  return (
    <>
      {(erreur || raison) && (
        <span className={`text-xs max-w-80 ${erreur ? "text-red-700" : "text-muted-foreground"}`} role="status" data-testid="whatsapp-raison">
          {erreur ?? raison}
        </span>
      )}
      <Button size="sm" variant="outline" disabled={!!raison} onClick={envoyer}
        aria-label={numero.ok ? `Envoyer le résumé par WhatsApp au ${numero.affiche}` : "Envoyer le résumé par WhatsApp"}>
        <MessageCircle className="h-4 w-4 mr-1" /> WhatsApp
      </Button>
    </>
  );
}

// =====================================================================
//  Le client (fiche 360) — D51 § 4 et 5
// =====================================================================

/**
 * Ecrire au client : la personne ecrit apres le bonjour, WhatsApp
 * s'ouvre, puis la fiche demande si le message est parti. L'echange
 * `whatsapp` ne se note qu'a ce Oui — au clic, le fil garderait des
 * messages jamais partis.
 */
export function MessageClientWhatsApp({ client, onNote }: {
  client: { id: string; nom: string; telephone: string | null };
  onNote: () => Promise<void>;
}) {
  const [etape, setEtape] = useState<"ferme" | "ecrit" | "ouvert">("ferme");
  const [texte, setTexte] = useState("");
  const [enCours, setEnCours] = useState(false);
  const [erreur, setErreur] = useState<string | null>(null);
  const numero = lireNumero(client.telephone);

  async function ouvrir() {
    if (!numero.ok) return;
    setErreur(null);
    try {
      await ouvrirWhatsApp(lienWhatsApp(numero.chiffres, texte.trim()));
      setEtape("ouvert");
    } catch (e) {
      setErreur(String(e));
    }
  }

  async function noter() {
    setEnCours(true);
    setErreur(null);
    try {
      await invoke("creer_echange", { clientId: client.id, genre: "whatsapp", quoi: texte.trim(), suitePrevue: null });
      setEtape("ferme");
      setTexte("");
      await onNote();
    } catch (e) {
      setErreur(String(e));
    } finally {
      setEnCours(false);
    }
  }

  if (etape === "ferme" || !numero.ok) {
    return (
      <div className="flex items-center gap-2 flex-wrap" data-testid="whatsapp-client">
        <Button size="sm" variant="outline" disabled={!numero.ok}
          onClick={() => { setTexte(debutMessageClient(client.nom)); setEtape("ecrit"); }}>
          <MessageCircle className="h-4 w-4 mr-1" /> Écrire sur WhatsApp
        </Button>
        {!numero.ok && <span className="text-xs text-muted-foreground" role="status" data-testid="whatsapp-raison">{numero.raison}</span>}
      </div>
    );
  }

  return (
    <div className="border border-border rounded-lg p-4 space-y-2" data-testid="whatsapp-client">
      {etape === "ecrit" ? (
        <>
          <label className="text-xs text-muted-foreground flex flex-col gap-1">
            Message à {client.nom} ({numero.affiche})
            <textarea value={texte} onChange={e => setTexte(e.target.value)} rows={4} aria-label="Message WhatsApp"
              className="text-sm text-foreground border border-border rounded-md bg-background p-2" />
          </label>
          <p className="text-xs text-muted-foreground">WhatsApp s'ouvre avec ce texte : c'est vous qui appuyez sur Envoyer.</p>
          <div className="flex gap-2">
            <Button size="sm" disabled={!messageEcrit(texte, client.nom)} onClick={ouvrir}>
              <MessageCircle className="h-4 w-4 mr-1" /> Ouvrir WhatsApp
            </Button>
            <Button size="sm" variant="ghost" onClick={() => { setEtape("ferme"); setErreur(null); }}>Annuler</Button>
          </div>
        </>
      ) : (
        <>
          <p className="text-sm">WhatsApp est ouvert vers {numero.affiche}. Le message est-il parti ?</p>
          <div className="flex gap-2">
            <Button size="sm" disabled={enCours} onClick={noter}>Oui, noter l'échange</Button>
            <Button size="sm" variant="ghost" disabled={enCours} onClick={() => setEtape("ecrit")}>Non</Button>
          </div>
        </>
      )}
      {erreur && <p className="text-xs text-red-700" role="status">{erreur}</p>}
    </div>
  );
}
