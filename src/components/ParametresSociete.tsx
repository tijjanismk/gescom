import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Loader2, Save, Building2, Upload, X, Image, PanelTop, PanelBottom
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

export function ParametresSociete() {
  const [params, setParams] = useState<ParamsSociete>({ nom: "" });
  const [logoBase64, setLogoBase64] = useState<string | null>(null);
  // Bandeau pleine largeur : présent, il remplace le logo ET le bloc
  // de coordonnées à l'impression (il les porte déjà).
  const [enteteBase64, setEnteteBase64] = useState<string | null>(null);
  const [uploadEntete, setUploadEntete] = useState(false);
  const [piedBase64, setPiedBase64] = useState<string | null>(null);
  const [uploadPied, setUploadPied] = useState(false);
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
      await message("Paramètres sauvegardés ✓", { title: "Succès", kind: "info" });
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

      // Copier le logo dans le répertoire de l'app
      await invoke("sauvegarder_logo", { cheminSource: fichier });

      // Recharger le logo en base64
      const logo = await invoke<string | null>("lire_logo_base64");
      setLogoBase64(logo);

      await message("Logo mis à jour ✓", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setUploadLogo(false);
    }
  }

  async function handleChoisirEntete() {
    try {
      setUploadEntete(true);
      const fichier = await open({
        multiple: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }],
      });
      if (!fichier || typeof fichier !== "string") return;
      await invoke("sauvegarder_entete", { cheminSource: fichier });
      setEnteteBase64(await invoke<string | null>("lire_entete_base64"));
      await message("En-tête mis à jour ✓", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setUploadEntete(false);
    }
  }

  async function handleSupprimerEntete() {
    try {
      await invoke("supprimer_entete");
      setEnteteBase64(null);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    }
  }

  async function handleChoisirPied() {
    try {
      setUploadPied(true);
      const fichier = await open({
        multiple: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "svg", "webp"] }],
      });
      if (!fichier || typeof fichier !== "string") return;
      await invoke("sauvegarder_pied", { cheminSource: fichier });
      setPiedBase64(await invoke<string | null>("lire_pied_base64"));
      await message("Pied de page mis à jour ✓", { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setUploadPied(false);
    }
  }

  async function handleSupprimerPied() {
    try {
      await invoke("supprimer_pied");
      setPiedBase64(null);
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
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

      {/* En-tête */}
      <div>
        <Label className="mb-2 block">En-tête de facture (optionnel)</Label>
        {enteteBase64 ? (
          <div className="relative">
            <img src={enteteBase64} alt="En-tête"
              className="w-full h-auto border border-border rounded-md" />
            <button onClick={handleSupprimerEntete}
              className="absolute -top-2 -right-2 w-5 h-5 rounded-full
                         bg-destructive text-destructive-foreground
                         flex items-center justify-center hover:opacity-90">
              <X className="h-3 w-3" />
            </button>
          </div>
        ) : (
          <div className="h-20 w-full border-2 border-dashed border-border
                          rounded-md flex items-center justify-center">
            <PanelTop className="h-6 w-6 text-muted-foreground" />
          </div>
        )}

        <Button variant="outline" size="sm" className="mt-2"
          onClick={handleChoisirEntete} disabled={uploadEntete}>
          {uploadEntete
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <><Upload className="h-4 w-4 mr-2" />
              {enteteBase64 ? "Changer l'en-tête" : "Choisir un en-tête"}</>}
        </Button>

        {/* Dire ce que ça remplace : sinon le commerçant croit à un
            bug en voyant ses coordonnées disparaître de la facture. */}
        <p className="text-xs text-muted-foreground mt-1">
          Bandeau pleine largeur, comme votre papier à en-tête.
          {enteteBase64
            ? " Il remplace le logo et les coordonnées ci-dessous sur les"
              + " factures A4 et A5 — jamais sur les tickets thermiques."
            : " Recommandé : environ 1600 × 220 px."}
        </p>
      </div>

      {/* Pied de page image */}
      <div>
        <Label className="mb-2 block">Pied de page (optionnel)</Label>
        {piedBase64 ? (
          <div className="relative">
            <img src={piedBase64} alt="Pied de page"
              className="w-full h-auto border border-border rounded-md" />
            <button onClick={handleSupprimerPied}
              className="absolute -top-2 -right-2 w-5 h-5 rounded-full
                         bg-destructive text-destructive-foreground
                         flex items-center justify-center hover:opacity-90">
              <X className="h-3 w-3" />
            </button>
          </div>
        ) : (
          <div className="h-14 w-full border-2 border-dashed border-border
                          rounded-md flex items-center justify-center">
            <PanelBottom className="h-5 w-5 text-muted-foreground" />
          </div>
        )}
        <Button variant="outline" size="sm" className="mt-2"
          onClick={handleChoisirPied} disabled={uploadPied}>
          {uploadPied
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <><Upload className="h-4 w-4 mr-2" />
              {piedBase64 ? "Changer le pied" : "Choisir un pied de page"}</>}
        </Button>
        <p className="text-xs text-muted-foreground mt-1">
          Mentions légales, coordonnées bancaires. Remplace le texte du
          pied de facture. Jamais sur les tickets thermiques.
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