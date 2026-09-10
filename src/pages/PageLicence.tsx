// pages/PageLicence.tsx — l'écran qui barre la route.
//
// Il ne s'affiche que quand le logiciel ne peut plus travailler :
// essai terminé, licence expirée, licence d'une autre machine. Le reste
// du temps, on ne parle pas de licence — un commerçant qui ouvre sa
// caisse à 7 h veut vendre, pas lire un rappel commercial.
//
// Deux choses doivent en sortir sans effort : le CODE DU POSTE, qu'il
// va dicter au téléphone, et un endroit où coller ce qu'il recevra. Le
// reste est du décor.

import { useEffect, useState } from "react";
import {
  AlertTriangle, Check, Copy, FileKey, Loader2, ShieldCheck, Upload,
} from "lucide-react";
import { message, open } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";
import { appeler as invoke } from "@/lib/pont";

export interface EtatLicence {
  etat:
    | "valide" | "essai" | "essai_termine" | "illisible"
    | "signature_invalide" | "autre_poste" | "expiree";
  empreinte?: string;
  jours_restants?: number | null;
  raison?: string;
  attendue?: string;
  le?: string;
  contenu?: {
    boutique: string;
    nif?: string | null;
    postes_max: number;
    emis_le: string;
    expire_le?: string | null;
  };
}

export function autorise(e: EtatLicence | null): boolean {
  return e?.etat === "valide" || e?.etat === "essai";
}

/** Ce qu'on dit au commerçant, selon la raison exacte du blocage. */
function explication(e: EtatLicence): { titre: string; texte: string } {
  switch (e.etat) {
    case "essai_termine":
      return {
        titre: "Période d'essai terminée",
        texte:
          "Les 30 jours d'essai sont écoulés. Vos données sont intactes : "
          + "activer la licence rouvre le logiciel tel que vous l'avez laissé.",
      };
    case "expiree":
      return {
        titre: `Licence expirée le ${e.le}`,
        texte:
          "La licence installée sur ce poste a atteint sa date de fin. "
          + "Demander son renouvellement avec le code ci-dessous.",
      };
    case "autre_poste":
      return {
        titre: "Licence émise pour un autre poste",
        texte:
          `Cette licence est rattachée au poste ${e.attendue}. Elle ne peut `
          + "pas servir ici. Si vous avez changé d'ordinateur ou réinstallé "
          + "Windows, demander une réémission avec le code ci-dessous.",
      };
    case "signature_invalide":
      return {
        titre: "Licence non authentique",
        texte:
          "Ce fichier n'a pas été émis par l'éditeur, ou il a été modifié "
          + "depuis. Redemander le fichier d'origine.",
      };
    default:
      return {
        titre: "Licence illisible",
        texte: e.raison ?? "Le fichier de licence n'a pas pu être lu.",
      };
  }
}

export function PageLicence({
  etat, onActive,
}: {
  etat: EtatLicence;
  onActive: (e: EtatLicence) => void;
}) {
  const [texte, setTexte] = useState("");
  const [travail, setTravail] = useState(false);
  const [copie, setCopie] = useState(false);
  const empreinte = etat.empreinte ?? "—";
  const { titre, texte: explique } = explication(etat);

  useEffect(() => {
    if (!copie) return;
    const t = setTimeout(() => setCopie(false), 2000);
    return () => clearTimeout(t);
  }, [copie]);

  async function copierCode() {
    try {
      await navigator.clipboard.writeText(empreinte);
      setCopie(true);
    } catch {
      // Le presse-papier peut être refusé selon le contexte ; le code
      // reste lisible à l'écran, c'est le seul canal qui compte.
      setCopie(false);
    }
  }

  async function activer(valeur: string) {
    if (!valeur.trim()) return;
    setTravail(true);
    try {
      const neuf = await invoke<EtatLicence>("activer_licence", { texte: valeur });
      if (neuf.etat === "valide") {
        onActive(neuf);
      } else {
        const { titre: t, texte: x } = explication(neuf);
        await message(x, { title: t, kind: "error" });
      }
    } catch (e) {
      await message(String(e), { title: "Activation", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  async function importerFichier() {
    try {
      const chemin = await open({
        title: "Choisir le fichier de licence",
        multiple: false,
        filters: [{ name: "Licence Gescom", extensions: ["licence", "txt"] }],
      });
      if (!chemin || typeof chemin !== "string") return;
      setTravail(true);
      const neuf = await invoke<EtatLicence>("importer_licence_fichier", { chemin });
      if (neuf.etat === "valide") {
        onActive(neuf);
      } else {
        const { titre: t, texte: x } = explication(neuf);
        await message(x, { title: t, kind: "error" });
      }
    } catch (e) {
      await message(String(e), { title: "Import de la licence", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-muted/30 p-6">
      <div className="w-full max-w-xl space-y-5 rounded-xl border bg-background p-7 shadow-sm">

        <div className="flex items-start gap-3">
          <AlertTriangle className="mt-0.5 h-6 w-6 shrink-0 text-amber-500" />
          <div>
            <h1 className="text-xl font-semibold">{titre}</h1>
            <p className="mt-1 text-sm text-muted-foreground">{explique}</p>
          </div>
        </div>

        {/* Le code du poste — la seule chose qu'il doit lire au téléphone. */}
        <div className="rounded-lg border bg-muted/40 p-4">
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Code de ce poste
          </p>
          <div className="mt-1.5 flex items-center gap-3">
            <code className="select-all font-mono text-2xl font-semibold tracking-widest">
              {empreinte}
            </code>
            <Button variant="outline" size="sm" onClick={copierCode}>
              {copie
                ? <><Check className="mr-1 h-3.5 w-3.5" /> Copié</>
                : <><Copy className="mr-1 h-3.5 w-3.5" /> Copier</>}
            </Button>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            Communiquer ce code à votre revendeur. Il vous renverra un
            fichier de licence valable pour cet ordinateur.
          </p>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Coller la licence reçue</label>
          <textarea
            className="min-h-[110px] w-full rounded-md border bg-transparent p-2.5
                       font-mono text-xs"
            placeholder="GESCOM1.…"
            value={texte}
            onChange={(e) => setTexte(e.target.value)}
          />
          <div className="flex gap-2">
            <Button
              className="flex-1" disabled={travail || !texte.trim()}
              onClick={() => activer(texte)}
            >
              {travail
                ? <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                : <ShieldCheck className="mr-2 h-4 w-4" />}
              Activer
            </Button>
            <Button variant="outline" disabled={travail} onClick={importerFichier}>
              <Upload className="mr-2 h-4 w-4" /> Importer un fichier
            </Button>
          </div>
        </div>

        <p className="flex items-start gap-2 border-t pt-4 text-xs text-muted-foreground">
          <FileKey className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          Aucune connexion Internet n'est nécessaire : la licence
          s'installe depuis un fichier. Vos données ne quittent pas cet
          ordinateur.
        </p>
      </div>
    </div>
  );
}

/** Le bandeau discret des derniers jours d'essai. */
export function BandeauEssai({ jours }: { jours: number }) {
  return (
    <div className="flex items-center justify-center gap-2 bg-amber-100 px-4 py-1.5
                    text-xs text-amber-900 dark:bg-amber-950 dark:text-amber-200">
      <AlertTriangle className="h-3.5 w-3.5" />
      Version d'essai — {jours} jour{jours > 1 ? "s" : ""} restant
      {jours > 1 ? "s" : ""}. Contacter votre revendeur pour activer la licence.
    </div>
  );
}
