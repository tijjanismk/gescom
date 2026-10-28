// components/ImportExportFournisseurs.tsx
//
// D44 — les fournisseurs du dossier en CSV, même format que le
// catalogue (point-virgule, UTF-8 avec BOM, Excel l'ouvre d'un
// double-clic). L'import passe par un aperçu ; un fournisseur déjà
// présent (même NIF, ou même nom sans majuscules ni espaces en trop)
// n'est jamais renommé : on complète ce qui lui manque, ou on le laisse.

import { useState } from "react";
import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import { message, save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { Download, Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ImportCsvAvecApercu, type BilanImport } from "@/components/ImportCsvAvecApercu";

export function ImportExportFournisseurs({ onImporte }: { onImporte?: (b: BilanImport) => void }) {
  const [export_, setExport] = useState(false);

  async function exporter() {
    setExport(true);
    try {
      const csv = await invoke<string>("exporter_fournisseurs_csv");
      const chemin = await save({
        defaultPath: `fournisseurs_${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!chemin) return;
      await writeTextFile(chemin, csv);
      await message(`Fournisseurs exportés.\n${chemin}`, { title: "Export", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Export", kind: "error" });
    } finally {
      setExport(false);
    }
  }

  return (
    <div className="space-y-5">
      <section className="space-y-2">
        <h3 className="text-sm font-semibold flex items-center gap-2">
          <Download className="h-4 w-4 text-muted-foreground" /> Exporter
        </h3>
        <p className="text-xs text-muted-foreground">
          Les fournisseurs de ce dossier : nom, téléphone, adresse, NIF,
          email, voisin (oui/non).
        </p>
        <Button variant="outline" onClick={exporter} disabled={export_} className="w-full">
          {export_ ? <Loader2 className="h-4 w-4 animate-spin" /> : <><Download className="h-4 w-4 mr-2" /> Exporter en CSV</>}
        </Button>
      </section>

      <section className="space-y-2">
        <h3 className="text-sm font-semibold flex items-center gap-2">
          <Upload className="h-4 w-4 text-muted-foreground" /> Importer
        </h3>
        <pre className="text-xs bg-muted p-2 rounded-md overflow-x-auto">
{`Nom;Téléphone;Adresse;NIF;Email;Voisin
Sodima SARL;20 22 00 00;ACI 2000;084123456M;;non
Boutique Kané;76 00 00 00;Marché de Médine;;;oui`}
        </pre>
        <ul className="text-xs text-muted-foreground space-y-0.5">
          <li>· Seul le nom est obligatoire ; les colonnes peuvent être dans le désordre</li>
          <li>· Même NIF, ou même nom (sans majuscules ni espaces en trop) : c'est le même fournisseur</li>
          <li>· Un homonyme qui n'est pas le même se crée à la main, « créer quand même »</li>
        </ul>
        <ImportCsvAvecApercu
          commande="importer_fournisseurs_csv"
          quoi="fournisseurs"
          option={{ libelle: "Compléter les fournisseurs déjà présents (champs vides seulement)", cle: "completer", parDefaut: true }}
          libelleRepris="Complétés"
          desactive={!peut("fournisseurs:regler")}
          onImporte={onImporte}
        />
      </section>
    </div>
  );
}
