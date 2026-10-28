// components/OngletImportExport.tsx
//
// Import / export du catalogue en CSV.
//
// Le format est point-virgule + UTF-8 avec BOM : c'est ce qu'Excel
// francophone ouvre correctement en double-cliquant. Un CSV virgule
// sans BOM lui donne une colonne unique de caractères abîmés, et le
// commerçant conclut que l'export ne marche pas.

import { useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { message, save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { Download, Upload, Loader2, FileSpreadsheet } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { ImportCsvAvecApercu } from "@/components/ImportCsvAvecApercu";

export function OngletImportExport() {
  const [travail, setTravail] = useState(false);

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
              vente, prix d'achat, TVA, code-barres, stock et « suivre le
              stock » (oui/non).
            </p>
            {/* Le fichier porte les prix d'achat : il demande
                `achats:lire_prix` (v3, C-1), le serveur refuse sinon. */}
            <Button onClick={exporter} disabled={travail || !peut("achats:lire_prix")} className="w-full"
              title={peut("achats:lire_prix") ? undefined : "Demande la permission de voir les prix d'achat"}>
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
              l'en-tête est présent. Nom et prix sont obligatoires. Un
              aperçu dit ce qui sera fait avant que rien ne s'écrive.
            </p>
            <ImportCsvAvecApercu
              commande="importer_articles_csv"
              quoi="articles"
              option={{ libelle: "Mettre à jour les articles déjà présents", cle: "mettreAJour", parDefaut: true }}
              libelleRepris="Mis à jour"
            />
          </CardContent>
        </Card>
      </div>

      {/* Format */}
      <Card>
        <CardContent className="pt-4">
          <h3 className="text-sm font-semibold mb-2">Format attendu</h3>
          <pre className="text-xs bg-muted p-3 rounded-md overflow-x-auto">
{`Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock;Suivre le stock
Sucre;Alimentaire;kg;1500;1200;18;;50;oui
Riz local;Alimentaire;sac;35000;30000;0;;10;
Sachets;;paquet;100;;;;;non`}
          </pre>
          <ul className="text-xs text-muted-foreground mt-3 space-y-1">
            <li>· <strong>Nom</strong> et <strong>Prix</strong> sont
              obligatoires, le reste peut rester vide</li>
            <li>· <strong>Suivre le stock</strong> (facultative) : oui ou
              non. Vide, un nouvel article suit son stock. Une autre valeur
              rejette la ligne, comme une valeur qui changerait le réglage
              d'un article déjà présent : cela se fait dans Articles</li>
            <li>· Les prix acceptent les espaces et les points :
              « 12 500 » et « 12.500 » donnent 12500</li>
            <li>· Une catégorie inconnue est créée automatiquement</li>
            <li>· Un article de même nom est mis à jour, jamais dupliqué —
              sans tenir compte des majuscules ni des espaces en trop
              (« Riz 25 kg » = « riz  25 KG »)</li>
            <li>· Deux lignes pour le même article et la même unité : la
              seconde est refusée</li>
            <li>· Un code-barre déjà porté par un autre article refuse sa
              ligne (création) ou n'est pas repris (mise à jour)</li>
            <li>· TVA vide : un article existant garde la sienne, un
              nouveau prend celle du dossier</li>
            <li>· Séparateur point-virgule, encodage UTF-8</li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
