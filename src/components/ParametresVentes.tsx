//! Onglet Ventes dans Paramètres — config scanner et code-barres articles.

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Scan, Loader2, Save, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { message } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

interface ArticleAvecCode {
  id: string;
  nom: string;
  unite_base: string;
  code_barre?: string;
}

export function OngletVentes() {
  const [scannerActif, setScannerActif] = useState(false);
  const [articles, setArticles] = useState<ArticleAvecCode[]>([]);
  const [chargement, setChargement] = useState(true);
  const [sauvegarde, setSauvegarde] = useState(false);
  const [codesEnEdition, setCodesEnEdition] = useState<Record<string, string>>({});

  useEffect(() => {
    async function charger() {
      setChargement(true);
      try {
        const [config, arts] = await Promise.all([
          invoke<boolean>("lire_config_scanner"),
          invoke<ArticleAvecCode[]>("lire_articles_avec_codes_barres"),
        ]);
        setScannerActif(config);
        setArticles(arts);
        // Pré-remplir les codes existants
        const codes: Record<string, string> = {};
        arts.forEach(a => { if (a.code_barre) codes[a.id] = a.code_barre; });
        setCodesEnEdition(codes);
      } catch (e) {
        console.error("Erreur chargement config ventes :", e);
      } finally {
        setChargement(false);
      }
    }
    charger();
  }, []);

  async function handleSauvegarderScanner(actif: boolean) {
    setSauvegarde(true);
    try {
      await invoke("sauvegarder_config_scanner", { actif });
      setScannerActif(actif);
      await message(
        actif ? "Scanner activé ✓" : "Scanner désactivé ✓",
        { title: "Succès", kind: "info" }
      );
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setSauvegarde(false);
    }
  }

  async function handleSauvegarderCode(articleId: string) {
    const code = codesEnEdition[articleId]?.trim() ?? "";
    try {
      await invoke("sauvegarder_code_barre_article", {
        articleId,
        codeBarre: code,
      });
      setArticles(prev => prev.map(a =>
        a.id === articleId ? { ...a, code_barre: code || undefined } : a
      ));
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  if (chargement) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-lg">

      {/* Scanner */}
      <div className="border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Scan className="h-4 w-4 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">Scanner de code-barres</p>
              <p className="text-xs text-muted-foreground">
                Détection automatique à la page Ventes
              </p>
            </div>
          </div>
          <Badge variant={scannerActif ? "default" : "outline"}>
            {scannerActif ? "Activé" : "Désactivé"}
          </Badge>
        </div>

        <div className="flex gap-2">
          <Button
            variant={scannerActif ? "outline" : "default"}
            size="sm"
            disabled={sauvegarde || scannerActif}
            onClick={() => handleSauvegarderScanner(true)}
            className="flex-1"
          >
            Activer
          </Button>
          <Button
            variant={!scannerActif ? "outline" : "destructive"}
            size="sm"
            disabled={sauvegarde || !scannerActif}
            onClick={() => handleSauvegarderScanner(false)}
            className="flex-1"
          >
            Désactiver
          </Button>
        </div>

        <p className="text-xs text-muted-foreground">
          Le scanner doit être configuré pour envoyer un retour chariot (Enter) 
          après chaque code. La plupart des scanners USB font ça par défaut.
        </p>
      </div>

      {/* Codes-barres articles */}
      <div>
        <div className="flex items-center gap-2 mb-3">
          <Package className="h-4 w-4 text-muted-foreground" />
          <p className="text-sm font-medium">Codes-barres des articles</p>
        </div>
        <p className="text-xs text-muted-foreground mb-3">
          Saisir ou scanner le code-barres de chaque article. 
          Appuyer sur Entrée pour valider chaque code.
        </p>

        <div className="space-y-2">
          {articles.map(a => (
            <div key={a.id}
              className="flex items-center gap-3 py-2 px-3 border border-border rounded-md">
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium truncate">{a.nom}</p>
                <p className="text-xs text-muted-foreground">{a.unite_base}</p>
              </div>
              <div className="flex items-center gap-2">
                <Input
                  value={codesEnEdition[a.id] ?? ""}
                  onChange={e => setCodesEnEdition(prev => ({
                    ...prev, [a.id]: e.target.value
                  }))}
                  placeholder="Code-barres"
                  className={cn(
                    "h-7 text-sm w-32 font-mono",
                    a.code_barre && "border-green-300"
                  )}
                  onKeyDown={e => {
                    if (e.key === "Enter") {
                      handleSauvegarderCode(a.id);
                    }
                  }}
                />
                <Button
                  size="sm" variant="ghost"
                  onClick={() => handleSauvegarderCode(a.id)}
                  className="h-7 px-2"
                >
                  <Save className="h-3 w-3" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
