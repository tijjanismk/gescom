import { useState, useEffect } from "react";
import { appeler as invoke } from "@/lib/pont";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import {
  Loader2, Save, Building2, Upload, X, Image, PanelTop
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { message } from "@tauri-apps/plugin-dialog";

interface ParamsSociete {
  nom: string;
  adresse?: string;
  telephone?: string;
  telephone2?: string;
  email?: string;
  nif?: string;
  rccm?: string;
  site_web?: string;
  pied_facture?: string;
}

// Un fichier choisi par la caisse part EN OCTETS (D8) : un chemin
// local ne designe rien chez le serveur. Le pont envoie du base64.
function enBase64(octets: Uint8Array): string {
  let texte = "";
  for (let i = 0; i < octets.length; i += 0x8000) {
    texte += String.fromCharCode(...octets.subarray(i, i + 0x8000));
  }
  return btoa(texte);
}

// Lit le fichier choisi et l'envoie en contenu a la commande donnee.
async function envoyerImage(commande: string, fichier: string): Promise<void> {
  const octets = await readFile(fichier);
  const nom = fichier.split(/[\\/]/).pop() || "image.png";
  await invoke(commande, { nom, contenu: enBase64(octets) });
}

export function ParametresSociete() {
  const [params, setParams] = useState<ParamsSociete>({ nom: "" });
  const [logoBase64, setLogoBase64] = useState<string | null>(null);
  // Bandeau pleine largeur : présent, il remplace le logo ET le bloc
  // de coordonnées à l'impression (il les porte déjà).
  const [enteteBase64, setEnteteBase64] = useState<string | null>(null);
  const [piedBase64, setPiedBase64] = useState<string | null>(null);
  const [chargement, setChargement] = useState(true);
  const [sauvegarde, setSauvegarde] = useState(false);
  const [uploadLogo, setUploadLogo] = useState(false);
  useEffect(() => {
    async function charger() {
      try {
        const [data, logo, entete, pied] = await Promise.all([
          invoke<ParamsSociete>("lire_parametres_societe"),
          invoke<string | null>("lire_logo_base64"),
          invoke<string | null>("lire_entete_base64").catch(() => null),
          invoke<string | null>("lire_pied_base64").catch(() => null),
        ]);
        setParams(data);
        setLogoBase64(logo);
        setEnteteBase64(entete);
        setPiedBase64(pied);
      } catch (e) {
        console.error("Erreur chargement société :", e);
      } finally {
        setChargement(false);
      }
    }
    charger();
  }, []);

  function set(champ: keyof ParamsSociete, val: string) {
    setParams(prev => ({ ...prev, [champ]: val || undefined }));
  }

  async function handleSauvegarder() {
    if (!params.nom.trim()) return;
    setSauvegarde(true);
    try {
      await invoke("sauvegarder_parametres_societe", {
        nom: params.nom.trim(),
        adresse: params.adresse || null,
        telephone: params.telephone || null,
        telephone2: params.telephone2 || null,
        email: params.email || null,
        nif: params.nif || null,
        rccm: params.rccm || null,
        siteWeb: params.site_web || null,
        piedFacture: params.pied_facture || null,
      });
      await message("Paramètres sauvegardés", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setSauvegarde(false);
    }
  }

  async function handleChoisirLogo() {
    try {
      setUploadLogo(true);
      const fichier = await open({
        multiple: false,
        filters: [{
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "svg", "webp"],
        }],
      });

      if (!fichier || typeof fichier !== "string") return;

      await envoyerImage("sauvegarder_logo", fichier);

      // Recharger le logo en base64
      const logo = await invoke<string | null>("lire_logo_base64");
      setLogoBase64(logo);

      await message("Logo mis à jour", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setUploadLogo(false);
    }
  }

  async function handleSupprimerLogo() {
    try {
      await invoke("supprimer_logo");
      setLogoBase64(null);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  if (chargement) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-lg">
      <div className="flex items-center gap-2">
        <Building2 className="h-4 w-4 text-muted-foreground" />
        <p className="text-sm text-muted-foreground">
          Ces informations apparaissent sur toutes les factures imprimées.
        </p>
      </div>

      {/* Logo */}
      <div>
        <Label className="mb-2 block">Logo de la société</Label>
        <div className="flex items-center gap-3">
          {logoBase64 ? (
            <div className="relative">
              <img
                src={logoBase64}
                alt="Logo"
                className="h-16 w-auto max-w-[160px] object-contain
                           border border-border rounded-md p-1"
              />
              <button
                onClick={handleSupprimerLogo}
                className="absolute -top-2 -right-2 w-5 h-5 rounded-full
                           bg-destructive text-destructive-foreground
                           flex items-center justify-center hover:opacity-90"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ) : (
            <div className="h-16 w-32 border-2 border-dashed border-border
                            rounded-md flex items-center justify-center">
              <Image className="h-6 w-6 text-muted-foreground" />
            </div>
          )}

          <Button
            variant="outline"
            size="sm"
            onClick={handleChoisirLogo}
            disabled={uploadLogo}
          >
            {uploadLogo
              ? <Loader2 className="h-4 w-4 animate-spin" />
              : <><Upload className="h-4 w-4 mr-2" />
                {logoBase64 ? "Changer" : "Choisir un logo"}</>
            }
          </Button>
        </div>
        <p className="text-xs text-muted-foreground mt-1">
          PNG, JPG ou SVG — recommandé : fond transparent, max 500px de large
        </p>
      </div>

      {/* En-tête et pied ont quitté cet écran : ce sont des morceaux
          de DOCUMENT, et on ne voit qu'en le dessinant s'il manque un
          cachet ou une mention. Ils se posent maintenant dans l'atelier,
          en bloc Image, à la taille voulue sur la page. */}
      <div className="rounded-md border border-dashed border-border p-3">
        <p className="text-sm font-medium flex items-center gap-1.5">
          <PanelTop className="h-3.5 w-3.5" />
          En-tête et pied de page
        </p>
        <p className="text-xs text-muted-foreground mt-1">
          Ils se règlent dans <strong>Paramètres → Modèles de documents</strong>,
          en posant un bloc « Image » : on choisit le fichier et sa taille
          en millimètres, en voyant le document.
          {(enteteBase64 || piedBase64) && (
            <> Les images déjà posées restent en place
              {enteteBase64 && piedBase64
                ? " (en-tête et pied)"
                : enteteBase64 ? " (en-tête)" : " (pied)"}.</>
          )}
        </p>
      </div>

      {/* Champs société */}
      <div className="grid grid-cols-2 gap-3">
        <div className="col-span-2">
          <Label>Nom de la société *</Label>
          <Input value={params.nom}
            onChange={e => set("nom", e.target.value)}
            placeholder="Ma Boutique SARL" className="mt-1" />
        </div>

        <div className="col-span-2">
          <Label>Adresse</Label>
          <Input value={params.adresse ?? ""}
            onChange={e => set("adresse", e.target.value)}
            placeholder="Bamako, Rue Xyz, Quartier..." className="mt-1" />
        </div>

        <div>
          <Label>Téléphone principal</Label>
          <Input value={params.telephone ?? ""}
            onChange={e => set("telephone", e.target.value)}
            placeholder="76 00 00 00" className="mt-1" />
        </div>

        <div>
          <Label>Téléphone secondaire</Label>
          <Input value={params.telephone2 ?? ""}
            onChange={e => set("telephone2", e.target.value)}
            placeholder="65 00 00 00" className="mt-1" />
        </div>

        <div>
          <Label>Email</Label>
          <Input value={params.email ?? ""}
            onChange={e => set("email", e.target.value)}
            placeholder="contact@maboutique.ml" className="mt-1" />
        </div>

        <div>
          <Label>Site web</Label>
          <Input value={params.site_web ?? ""}
            onChange={e => set("site_web", e.target.value)}
            placeholder="www.maboutique.ml" className="mt-1" />
        </div>

        <div>
          <Label>NIF</Label>
          <Input value={params.nif ?? ""}
            onChange={e => set("nif", e.target.value)}
            placeholder="Numéro d'identification fiscale" className="mt-1" />
        </div>

        <div>
          <Label>RCCM</Label>
          <Input value={params.rccm ?? ""}
            onChange={e => set("rccm", e.target.value)}
            placeholder="Registre du commerce" className="mt-1" />
        </div>

        <div className="col-span-2">
          <Label>Pied de facture (texte)</Label>
          <Input value={params.pied_facture ?? ""}
            onChange={e => set("pied_facture", e.target.value)}
            placeholder="Merci de votre confiance" className="mt-1" />
          {/* Le texte est inséré tel quel dans la facture : les balises
              HTML fonctionnent. Une balise non fermée casse la mise en
              page — le dire vaut mieux que de le laisser découvrir. */}
          <p className="text-xs text-muted-foreground mt-1">
            Mise en forme possible : <code>&lt;b&gt;gras&lt;/b&gt;</code>,{" "}
            <code>&lt;br&gt;</code> pour aller à la ligne. Bien refermer
            chaque balise.
            {piedBase64 && " Ignoré tant qu'une image de pied est définie."}
          </p>
        </div>

        {/* Les noms à signer ne se règlent plus ici : chaque modèle de
            document porte son bloc « Signatures » (Paramètres → Modèles),
            avec ses deux noms. */}
      </div>

      <Button
        onClick={handleSauvegarder}
        disabled={!params.nom.trim() || sauvegarde}
        className="w-full"
      >
        {sauvegarde
          ? <Loader2 className="h-4 w-4 animate-spin" />
          : <><Save className="h-4 w-4 mr-2" /> Sauvegarder</>
        }
      </Button>
    </div>
  );
}