// components/ImportCsvAvecApercu.tsx
//
// D44 — un import CSV en deux temps : l'APERÇU (le serveur joue tout
// l'import dans une transaction qu'il laisse retomber), puis, sur
// confirmation, l'import pour de vrai. L'aperçu dit ce qui sera créé,
// mis à jour ou complété, laissé tel quel (doublon) ou refusé, ligne par
// ligne — c'est le même code côté serveur, donc la même réponse.
//
// Sert aux articles (Paramètres → Import / export, assistant) et aux
// fournisseurs (page Fournisseurs, assistant).

import { useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { message, open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { AlertTriangle, CheckCircle2, Eye, Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

/** Ce que rendent `importer_articles_csv` et `importer_fournisseurs_csv`. */
export interface BilanImport {
  crees: number;
  mis_a_jour?: number;
  completes?: number;
  ignores: string[];
  nb_ignores: number;
  erreurs: string[];
  nb_erreurs: number;
  avertissements?: string[];
  simule: boolean;
}

interface Props {
  /** La commande du serveur. */
  commande: "importer_articles_csv" | "importer_fournisseurs_csv";
  /** Ce qu'on importe, au pluriel : « articles », « fournisseurs ». */
  quoi: string;
  /** La case à cocher : que faire d'un doublon. */
  option: { libelle: string; cle: "mettreAJour" | "completer"; parDefaut: boolean };
  /** Libellé du compteur des doublons repris : « Mis à jour », « Complétés ». */
  libelleRepris: string;
  desactive?: boolean;
  onImporte?: (b: BilanImport) => void;
}

export function ImportCsvAvecApercu({ commande, quoi, option, libelleRepris, desactive, onImporte }: Props) {
  const [travail, setTravail] = useState(false);
  const [choix, setChoix] = useState(option.parDefaut);
  const [fichier, setFichier] = useState<{ chemin: string; contenu: string } | null>(null);
  const [bilan, setBilan] = useState<BilanImport | null>(null);

  async function appelerImport(contenu: string, simuler: boolean) {
    return invoke<BilanImport>(commande, { contenu, [option.cle]: choix, simuler });
  }

  async function choisir() {
    setBilan(null);
    setFichier(null);
    try {
      const chemin = await open({ multiple: false, filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
      if (!chemin || typeof chemin !== "string") return;
      setTravail(true);
      const contenu = await readTextFile(chemin);
      setFichier({ chemin, contenu });
      setBilan(await appelerImport(contenu, true));
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Import", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  // La case changée après l'aperçu : on le refait, sinon il mentirait.
  async function changerChoix(v: boolean) {
    setChoix(v);
    if (!fichier) return;
    setTravail(true);
    try {
      setBilan(await invoke<BilanImport>(commande, { contenu: fichier.contenu, [option.cle]: v, simuler: true }));
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Import", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  async function confirmer() {
    if (!fichier) return;
    setTravail(true);
    try {
      const b = await appelerImport(fichier.contenu, false);
      setBilan(b);
      setFichier(null);
      onImporte?.(b);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Import", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  const repris = bilan ? (bilan.mis_a_jour ?? bilan.completes ?? 0) : 0;
  const rienAFaire = bilan && bilan.crees + repris === 0;

  return (
    <div className="space-y-3">
      <label className="flex items-center gap-2 text-xs cursor-pointer">
        <input type="checkbox" checked={choix} disabled={travail}
          onChange={e => changerChoix(e.target.checked)}
          className="w-3.5 h-3.5 rounded accent-primary" />
        {option.libelle}
      </label>
      <Button onClick={choisir} disabled={travail || desactive} variant="outline" className="w-full">
        {travail && !bilan
          ? <Loader2 className="h-4 w-4 animate-spin" />
          : <><Upload className="h-4 w-4 mr-2" /> Choisir un fichier</>}
      </Button>

      {bilan && (
        <Card className={bilan.nb_erreurs > 0 ? "border-orange-400" : ""}>
          <CardContent className="pt-4 space-y-3">
            <div className="flex items-center gap-2">
              {bilan.simule
                ? <Eye className="h-4 w-4 text-primary" />
                : bilan.nb_erreurs === 0
                  ? <CheckCircle2 className="h-4 w-4 text-green-600" />
                  : <AlertTriangle className="h-4 w-4 text-orange-800" />}
              <h3 className="text-sm font-semibold">
                {bilan.simule ? `Aperçu — rien n'est encore écrit` : `Import terminé`}
              </h3>
            </div>
            {fichier && (
              <p className="text-xs text-muted-foreground truncate font-mono">{fichier.chemin}</p>
            )}

            <div className="flex flex-wrap gap-x-6 gap-y-1 text-sm">
              <span>{bilan.simule ? "À créer" : "Créés"} : <strong className="text-green-700">{bilan.crees}</strong></span>
              <span>{libelleRepris} : <strong>{repris}</strong></span>
              <span>Déjà présents, laissés : <strong>{bilan.nb_ignores}</strong></span>
              {bilan.nb_erreurs > 0 && (
                <span>Refusés : <strong className="text-orange-800">{bilan.nb_erreurs}</strong></span>
              )}
            </div>

            <Liste titre="Lignes refusées — les autres passent :" lignes={bilan.erreurs} ton="orange" />
            <Liste titre="À savoir :" lignes={bilan.avertissements ?? []} ton="bleu" />
            <Liste titre={`Doublons laissés tels quels (${quoi} déjà présents) :`} lignes={bilan.ignores} ton="gris" />

            {bilan.simule && (
              <div className="flex gap-2">
                <Button onClick={confirmer} disabled={travail || !!rienAFaire} className="flex-1">
                  {travail
                    ? <Loader2 className="h-4 w-4 animate-spin" />
                    : rienAFaire ? "Rien à importer" : `Importer pour de vrai`}
                </Button>
                <Button variant="outline" disabled={travail}
                  onClick={() => { setBilan(null); setFichier(null); }}>
                  Annuler
                </Button>
              </div>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function Liste({ titre, lignes, ton }: { titre: string; lignes: string[]; ton: "orange" | "bleu" | "gris" }) {
  if (lignes.length === 0) return null;
  const couleurs = {
    orange: "bg-card border-l-4 border-l-orange-600 text-foreground",
    bleu: "bg-blue-50 text-blue-900",
    gris: "bg-muted text-muted-foreground",
  }[ton];
  return (
    <div className={`rounded-md p-3 max-h-44 overflow-auto ${couleurs}`}>
      <p className="text-xs font-medium mb-1">{titre}</p>
      <ul className="text-xs space-y-0.5">
        {lignes.map((e, i) => <li key={i}>· {e}</li>)}
      </ul>
    </div>
  );
}
