// components/OngletImportExport.tsx
//
// Import / export du catalogue en CSV.
//
// Le format est point-virgule + UTF-8 avec BOM : c'est ce qu'Excel
// francophone ouvre correctement en double-cliquant. Un CSV virgule
// sans BOM lui donne une colonne unique de caractères abîmés, et le
// commerçant conclut que l'export ne marche pas.

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message, save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import {
  Download, Upload, Loader2, FileSpreadsheet, AlertTriangle, CheckCircle2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

interface Resultat {
  crees: number; mis_a_jour: number;
  erreurs: string[]; nb_erreurs: number;
}

export function OngletImportExport() {
  const [travail, setTravail] = useState(false);
  const [resultat, setResultat] = useState<Resultat | null>(null);
  const [majExistants, setMajExistants] = useState(true);

  async function exporter() {
    setTravail(true);
    try {
      const csv = await invoke<string>("exporter_articles_csv");
      const chemin = await save({
        defaultPath: `catalogue_${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!chemin) return;
      await writeTextFile(chemin, csv);
      await message(`Catalogue exporté.\n${chemin}`,
        { title: "Export", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Export", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  async function importer() {
    setResultat(null);
    try {
      const chemin = await open({
        multiple: false,
        filters: [{ name: "CSV", extensions: ["csv", "txt"] }],
      });
      if (!chemin || typeof chemin !== "string") return;

      setTravail(true);
      const contenu = await readTextFile(chemin);
      const r = await invoke<Resultat>("importer_articles_csv", {
        contenu, mettreAJour: majExistants,
      });
      setResultat(r);

      if (r.nb_erreurs === 0) {
        await message(
          `${r.crees} article(s) créé(s), ${r.mis_a_jour} mis à jour.`,
          { title: "Import terminé", kind: "info" });
      }
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Import", kind: "error" });
    } finally {
      setTravail(false);
    }
  }

  return (
    <div className="space-y-6">

      <div>
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <FileSpreadsheet className="h-5 w-5" /> Import / export
        </h2>
        <p className="text-xs text-muted-foreground">
          Reprendre un catalogue existant sous Excel, ou le sortir pour
          le retravailler.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">

        {/* Export */}
        <Card>
          <CardContent className="pt-4 space-y-3">
            <div className="flex items-center gap-2">
              <Download className="h-4 w-4 text-muted-foreground" />
              <h3 className="text-sm font-semibold">Exporter le catalogue</h3>
            </div>
            <p className="text-xs text-muted-foreground">
              Tous les articles actifs, avec catégorie, unité, prix de
              vente, prix d'achat, TVA, code-barres et stock.
            </p>
            <Button onClick={exporter} disabled={travail} className="w-full">
              {travail
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Download className="h-4 w-4 mr-2" /> Exporter en CSV</>}
            </Button>
          </CardContent>
        </Card>

        {/* Import */}
        <Card>
          <CardContent className="pt-4 space-y-3">
            <div className="flex items-center gap-2">
              <Upload className="h-4 w-4 text-muted-foreground" />
              <h3 className="text-sm font-semibold">Importer un catalogue</h3>
            </div>
            <p className="text-xs text-muted-foreground">
              Les colonnes peuvent être dans le désordre du moment que
              l'en-tête est présent. Nom et prix sont obligatoires.
            </p>
            <label className="flex items-center gap-2 text-xs cursor-pointer">
              <input type="checkbox" checked={majExistants}
                onChange={e => setMajExistants(e.target.checked)}
                className="w-3.5 h-3.5 rounded accent-primary" />
              Mettre à jour les articles déjà présents
            </label>
            <Button onClick={importer} disabled={travail}
              variant="outline" className="w-full">
              {travail
                ? <Loader2 className="h-4 w-4 animate-spin" />
                : <><Upload className="h-4 w-4 mr-2" /> Choisir un fichier</>}
            </Button>
          </CardContent>
        </Card>
      </div>

      {/* Résultat */}
      {resultat && (
        <Card className={resultat.nb_erreurs > 0 ? "border-orange-300" : ""}>
          <CardContent className="pt-4 space-y-3">
            <div className="flex items-center gap-2">
              {resultat.nb_erreurs === 0
                ? <CheckCircle2 className="h-4 w-4 text-green-600" />
                : <AlertTriangle className="h-4 w-4 text-orange-500" />}
              <h3 className="text-sm font-semibold">Résultat de l'import</h3>
            </div>

            <div className="flex gap-6 text-sm">
              <span>Créés : <strong className="text-green-700">
                {resultat.crees}</strong></span>
              <span>Mis à jour : <strong>{resultat.mis_a_jour}</strong></span>
              {resultat.nb_erreurs > 0 && (
                <span>Rejetés : <strong className="text-orange-600">
                  {resultat.nb_erreurs}</strong></span>
              )}
            </div>

            {resultat.erreurs.length > 0 && (
              <div className="bg-orange-50 rounded-md p-3 max-h-52 overflow-auto">
                <p className="text-xs font-medium text-orange-900 mb-1">
                  Lignes rejetées — les autres ont bien été importées :
                </p>
                <ul className="text-xs text-orange-800 space-y-0.5">
                  {resultat.erreurs.map((e, i) => <li key={i}>· {e}</li>)}
                </ul>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Format */}
      <Card>
        <CardContent className="pt-4">
          <h3 className="text-sm font-semibold mb-2">Format attendu</h3>
          <pre className="text-xs bg-muted p-3 rounded-md overflow-x-auto">
{`Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock
Sucre;Alimentaire;kg;1500;1200;18;;50
Riz local;Alimentaire;sac;35000;30000;0;;10
Huile;;litre;2500;;;;`}
          </pre>
          <ul className="text-xs text-muted-foreground mt-3 space-y-1">
            <li>· <strong>Nom</strong> et <strong>Prix</strong> sont
              obligatoires, le reste peut rester vide</li>
            <li>· Les prix acceptent les espaces et les points :
              « 12 500 » et « 12.500 » donnent 12500</li>
            <li>· Une catégorie inconnue est créée automatiquement</li>
            <li>· Un article de même nom est mis à jour, jamais dupliqué</li>
            <li>· Séparateur point-virgule, encodage UTF-8</li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
