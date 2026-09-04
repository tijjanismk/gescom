import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Package, Tag, Building2, Users, HardDrive,
  Plus, Loader2, Eye, EyeOff, ShoppingCart,
  FolderOpen, ChevronDown, ChevronRight,
  Percent, Banknote, XCircle, Clock, Warehouse, Barcode, Pencil,
  FileSpreadsheet,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { message } from "@tauri-apps/plugin-dialog";
import { ParametresSociete } from "@/components/ParametresSociete";
import { OngletVentes } from "@/components/ParametresVentes";
import { MoneyInput, parseMontant } from "@/components/MoneyInput";
import { SelectUnite } from "@/components/SelectUnite";
import { OngletTVA, OngletDettes, OngletIrrecouvrable, OngletAvoirs } from "@/components/OngletChantiers";
import { OngletDepots } from "@/components/OngletDepots";
import { OngletCodesBarres } from "@/components/OngletCodesBarres";
import { OngletImportExport } from "@/components/OngletImportExport";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface Categorie { id: string; nom: string; }
interface Unite {
  id: string; libelle: string; facteur: number; prix_reference: number;
  // EAN propre au conditionnement : le carton a le sien (D45).
  code_barre?: string | null;
}
interface ArticleComplet {
  id: string; nom: string; unite_base: string;
  dernier_prix_achat?: number; unites: Unite[];
}
interface Utilisateur {
  id: string; nom: string; role: string;
  pseudo?: string; email?: string;
  derniere_connexion?: string; actif: boolean;
}
interface ConfigSauvegarde {
  dossier_sauvegarde?: string;
  sauvegarde_auto: boolean;
  derniere_sauvegarde?: string;
}

function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n) + " F";
}

// =====================================================================
//  Onglets disponibles selon le rôle
// =====================================================================

const ONGLETS_PATRON = [
  { key: "societe",       label: "Société",       icone: Building2  },
  { key: "depots",        label: "Dépôts",        icone: Warehouse  },
  { key: "articles",      label: "Articles",      icone: Package    },
  { key: "codesbarres",   label: "Codes-barres",  icone: Barcode    },
  { key: "importexport",  label: "Import/Export", icone: FileSpreadsheet },
  { key: "categories",    label: "Catégories",    icone: Tag        },
  { key: "ventes",        label: "Ventes",        icone: ShoppingCart },
  { key: "utilisateurs",  label: "Utilisateurs",  icone: Users      },
  { key: "sauvegarde",    label: "Sauvegarde",    icone: HardDrive  },
  { key: "tva",           label: "TVA",           icone: Percent    },
  { key: "dettes",        label: "Dettes fourn.", icone: Banknote   },
  { key: "irrecouvrable", label: "Irrécouvrable", icone: XCircle    },
  { key: "avoirs",        label: "Avoirs",        icone: Clock      },
];

// Un employe ne voit que 4 onglets — c'est voulu. Il n'a rien a faire
// dans la TVA, les dettes ou les utilisateurs.
//
// « Sauvegarde » a ete RETIRE : la commande produit une copie complete
// de la base, prix d'achat et marges compris. C'est au patron.
const ONGLETS_EMPLOYE = [
  { key: "articles",    label: "Articles",     icone: Package      },
  { key: "categories",  label: "Catégories",   icone: Tag          },
  { key: "ventes",      label: "Ventes",       icone: ShoppingCart },
  { key: "codesbarres", label: "Codes-barres", icone: Barcode      },
];

// =====================================================================
//  Modal : Nouvel utilisateur
// =====================================================================

function ModalNouvelUtilisateur({
  ouvert, onFermer, onCreer,
}: { ouvert: boolean; onFermer: () => void; onCreer: () => void }) {
  const [nom, setNom] = useState("");
  const [pseudo, setPseudo] = useState("");
  const [email, setEmail] = useState("");
  const [mdp, setMdp] = useState("");
  const [role, setRole] = useState("employe");
  const [visible, setVisible] = useState(false);
  const [chargement, setChargement] = useState(false);

  async function handleCreer() {
    if (!nom.trim() || !pseudo.trim() || mdp.length < 6) return;
    setChargement(true);
    try {
      await invoke("creer_utilisateur", {
        nom: nom.trim(),
        pseudo: pseudo.trim(),
        email: email.trim() || null,
        motDePasse: mdp,
        roleNom: role,
        auteurId: UTILISATEUR_ACTIF?.id ?? "system",
      });
      setNom(""); setPseudo(""); setEmail(""); setMdp(""); setRole("employe");
      onCreer();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargement(false);
    }
  }

  return (
    <Dialog open={ouvert} onOpenChange={onFermer}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Nouvel utilisateur</DialogTitle></DialogHeader>
        <div className="space-y-3 pt-2">
          <div>
            <Label>Nom complet *</Label>
            <Input value={nom} onChange={e => setNom(e.target.value)}
              placeholder="Prénom Nom" autoFocus className="mt-1" />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <Label>Pseudo *</Label>
              <Input value={pseudo} onChange={e => setPseudo(e.target.value)}
                placeholder="jean" className="mt-1" />
            </div>
            <div>
              <Label>Rôle *</Label>
              <Select value={role} onValueChange={v => { if (v) setRole(v); }}>
                <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="patron">Patron</SelectItem>
                  <SelectItem value="employe">Employé</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div>
            <Label>Email</Label>
            <Input value={email} onChange={e => setEmail(e.target.value)}
              placeholder="jean@boutique.ml" className="mt-1" />
          </div>
          <div>
            <Label>
              Mot de passe *
              <span className="text-xs text-muted-foreground ml-1">(min. 6 car.)</span>
            </Label>
            <div className="relative mt-1">
              <Input type={visible ? "text" : "password"}
                value={mdp} onChange={e => setMdp(e.target.value)}
                placeholder="••••••" />
              <button type="button" onClick={() => setVisible(!visible)}
                className="absolute right-3 top-2.5 text-muted-foreground">
                {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            {mdp && mdp.length < 6 && (
              <p className="text-xs text-red-500 mt-1">Minimum 6 caractères</p>
            )}
          </div>
          <div className="flex gap-2 pt-1">
            <Button variant="outline" onClick={onFermer} className="flex-1">Annuler</Button>
            <Button
              onClick={handleCreer}
              disabled={!nom.trim() || !pseudo.trim() || mdp.length < 6 || chargement}
              className="flex-1">
              {chargement ? <Loader2 className="h-4 w-4 animate-spin" /> : "Créer"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================================
//  Onglet : Utilisateurs
// =====================================================================

function OngletUtilisateurs() {
  const [utilisateurs, setUtilisateurs] = useState<Utilisateur[]>([]);
  const [chargement, setChargement] = useState(true);
  const [modalNouvel, setModalNouvel] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<Utilisateur[]>("lire_utilisateurs");
      setUtilisateurs(data);
    } catch (e) {
      console.error("Erreur utilisateurs :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-4 max-w-lg">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {utilisateurs.length} utilisateur{utilisateurs.length > 1 ? "s" : ""}
        </p>
        <Button size="sm" onClick={() => setModalNouvel(true)}>
          <Plus className="h-4 w-4 mr-1" /> Nouveau
        </Button>
      </div>

      <div className="space-y-2">
        {utilisateurs.map(u => (
          <div key={u.id}
            className="flex items-center justify-between px-4 py-3
                       border border-border rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-primary/10
                              flex items-center justify-center">
                <span className="text-sm font-medium text-primary">
                  {u.nom[0].toUpperCase()}
                </span>
              </div>
              <div>
                <p className="text-sm font-medium">{u.nom}</p>
                <p className="text-xs text-muted-foreground">
                  {u.pseudo ? `@${u.pseudo}` : ""}
                  {u.email ? ` · ${u.email}` : ""}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant={u.role === "patron" ? "default" : "secondary"}>
                {u.role}
              </Badge>
              {!u.actif && <Badge variant="outline">Inactif</Badge>}
            </div>
          </div>
        ))}
      </div>

      <ModalNouvelUtilisateur
        ouvert={modalNouvel}
        onFermer={() => setModalNouvel(false)}
        onCreer={() => { setModalNouvel(false); charger(); }}
      />
    </div>
  );
}

// =====================================================================
//  Onglet : Sauvegarde
// =====================================================================

function OngletSauvegarde() {
  const [config, setConfig] = useState<ConfigSauvegarde>({ sauvegarde_auto: false });
  const [chargement, setChargement] = useState(true);
  const [enCours, setEnCours] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<ConfigSauvegarde>("lire_config_sauvegarde");
      setConfig(data);
    } catch (e) {
      console.error("Erreur config sauvegarde :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  async function choisirDossier() {
    try {
      const dossier = await open({ directory: true, multiple: false });
      if (!dossier || typeof dossier !== "string") return;
      setConfig(prev => ({ ...prev, dossier_sauvegarde: dossier }));
      await invoke("sauvegarder_config_sauvegarde", {
        dossierSauvegarde: dossier,
        sauvegardeAuto: config.sauvegarde_auto,
      });
    } catch (e) {
      console.error("Erreur sélection dossier :", e);
    }
  }

  async function lancerSauvegarde() {
    if (!config.dossier_sauvegarde) {
      await message("Veuillez d'abord choisir un dossier de sauvegarde.",
        { title: "Attention", kind: "warning" });
      return;
    }
    setEnCours(true);
    try {
      const chemin = await invoke<string>("sauvegarder_base", {
        dossierDestination: config.dossier_sauvegarde,
      });
      await charger();
      await message(`Sauvegarde réussie ✓\n${chemin}`,
        { title: "Succès", kind: "info" });
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setEnCours(false);
    }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-6 max-w-lg">
      <div className="bg-muted rounded-lg p-4 text-sm text-muted-foreground">
        <p className="font-medium text-foreground mb-1">Protection de vos données</p>
        <p>La sauvegarde copie la base de données complète vers le dossier de votre choix
          (clé USB, disque externe, autre partition).</p>
      </div>

      <div>
        <Label>Dossier de sauvegarde</Label>
        <div className="flex gap-2 mt-1">
          <Input value={config.dossier_sauvegarde ?? ""}
            readOnly placeholder="Aucun dossier sélectionné"
            className="flex-1 text-sm" />
          <Button variant="outline" onClick={choisirDossier}>
            <FolderOpen className="h-4 w-4 mr-2" /> Choisir
          </Button>
        </div>
      </div>

      {config.derniere_sauvegarde && (
        <p className="text-xs text-muted-foreground">
          Dernière sauvegarde : {config.derniere_sauvegarde}
        </p>
      )}

      <Button
        onClick={lancerSauvegarde}
        disabled={enCours || !config.dossier_sauvegarde}
        className="w-full">
        {enCours
          ? <><Loader2 className="h-4 w-4 animate-spin mr-2" /> Sauvegarde en cours...</>
          : <><HardDrive className="h-4 w-4 mr-2" /> Sauvegarder maintenant</>
        }
      </Button>
    </div>
  );
}

// =====================================================================
//  Onglet : Articles
// =====================================================================

function OngletArticles() {
  const [articles, setArticles] = useState<ArticleComplet[]>([]);
  const [categories, setCategories] = useState<Categorie[]>([]);
  const [chargement, setChargement] = useState(true);
  const [expandeId, setExpandeId] = useState<string | null>(null);
  const [modalArticle, setModalArticle] = useState(false);

  // Packs (D39) : facteur TOUJOURS en unités de base, jamais emboîté.
  // Base = pièce, carton de 12, pack de 6 cartons -> 1, 12, 72. Pas 6.
  const [modalUnite, setModalUnite] = useState<ArticleComplet | null>(null);
  const [uLibelle, setULibelle] = useState("");
  const [uFacteur, setUFacteur] = useState("");
  const [uPrix, setUPrix] = useState("");
  const [uCodeBarre, setUCodeBarre] = useState("");
  const [uEnCours, setUEnCours] = useState(false);
  // Non-null = on modifie cette unité au lieu d'en créer une.
  const [uEdition, setUEdition] = useState<Unite | null>(null);

  const [nom, setNom] = useState("");
  const [categorieId, setCategorieId] = useState("");
  const [uniteBase, setUniteBase] = useState("pièce");
  const [prixVente, setPrixVente] = useState("");
  const [chargementCreer, setChargementCreer] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const [a, c] = await Promise.all([
        invoke<ArticleComplet[]>("lire_articles_complets"),
        invoke<Categorie[]>("lire_categories"),
      ]);
      setArticles(a);
      setCategories(c);
    } catch (e) {
      console.error("Erreur articles :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  async function handleCreerArticle() {
    if (!nom.trim() || !prixVente) return;
    setChargementCreer(true);
    try {
      await invoke("creer_article_complet", {
        nom: nom.trim(),
        categorieId: categorieId || null,
        uniteBase,
        prixReference: parseMontant(prixVente),
      });
      setNom(""); setCategorieId(""); setUniteBase("pièce"); setPrixVente("");
      setModalArticle(false);
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargementCreer(false);
    }
  }

  function ouvrirModalUnite(a: ArticleComplet) {
    setUEdition(null);
    setULibelle(""); setUFacteur(""); setUPrix(""); setUCodeBarre("");
    setModalUnite(a);
  }

  function ouvrirModalEdition(a: ArticleComplet, u: Unite) {
    setUEdition(u);
    setULibelle(u.libelle);
    setUFacteur(String(u.facteur));
    setUPrix(String(u.prix_reference));
    setUCodeBarre(u.code_barre ?? "");
    setModalUnite(a);
  }

  async function handleAjouterUnite() {
    if (!modalUnite || !uLibelle.trim() || !uFacteur || !uPrix) return;
    const facteur = parseFloat(uFacteur.replace(",", "."));
    if (!(facteur > 0)) {
      await message("Le facteur doit être supérieur à zéro.",
        { title: "Facteur invalide", kind: "error" });
      return;
    }
    setUEnCours(true);
    try {
      if (uEdition) {
        await invoke("modifier_unite_vente", {
          uniteId: uEdition.id,
          libelle: uLibelle.trim(),
          // Le serveur refuse de changer le facteur de l'unité de base.
          facteur: uEdition.facteur === 1 ? null : facteur,
          prixReference: parseMontant(uPrix),
          codeBarre: uCodeBarre.trim() || null,
        });
        setModalUnite(null);
        await charger();
      } else {
        const r = await invoke<{ id: string; alerte: string | null }>(
          "ajouter_unite_vente", {
            articleId: modalUnite.id,
            libelle: uLibelle.trim(),
            facteur,
            prixReference: parseMontant(uPrix),
            codeBarre: uCodeBarre.trim() || null,
          });
        setModalUnite(null);
        await charger();
        // Avertissement, pas un refus : les remises de gros sont réelles.
        if (r?.alerte) {
          await message(r.alerte, { title: "Prix à vérifier", kind: "warning" });
        }
      }
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setUEnCours(false);
    }
  }

  async function handleDesactiverUnite(u: Unite) {
    try {
      await invoke("desactiver_unite_vente", { uniteId: u.id });
      await charger();
    } catch (e) {
      await message(`${e}`, { title: "Impossible", kind: "error" });
    }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">{articles.length} articles</p>
        <Button size="sm" onClick={() => setModalArticle(true)}>
          <Plus className="h-4 w-4 mr-1" /> Nouvel article
        </Button>
      </div>

      <div className="space-y-1">
        {articles.map(a => (
          <div key={a.id} className="border border-border rounded-lg overflow-hidden">
            <button
              onClick={() => setExpandeId(expandeId === a.id ? null : a.id)}
              className="w-full flex items-center justify-between px-4 py-2.5
                         hover:bg-muted/40 transition-colors text-left">
              <div className="flex items-center gap-2">
                {expandeId === a.id
                  ? <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" />
                  : <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />
                }
                <div>
                  <p className="text-sm font-medium">{a.nom}</p>
                  <p className="text-xs text-muted-foreground">{a.unite_base}</p>
                </div>
              </div>
              <Badge variant="secondary" className="text-xs">
                {a.unites.length} unité{a.unites.length > 1 ? "s" : ""}
              </Badge>
            </button>

            {expandeId === a.id && (
              <div className="border-t border-border bg-muted/20 px-4 py-3 space-y-1">
                {a.unites.map(u => (
                  <div key={u.id}
                    className="flex items-center justify-between text-sm py-1 border-b border-border last:border-0">
                    <div>
                      <span className="text-muted-foreground">{u.libelle}</span>
                      <span className="text-xs text-muted-foreground ml-2">
                        × {u.facteur} {a.unite_base}
                      </span>
                      {u.facteur === 1 && (
                        <Badge variant="outline" className="ml-2 text-[10px] py-0">
                          base
                        </Badge>
                      )}
                      {u.code_barre && (
                        <span className="block text-[10px] text-muted-foreground
                                         font-mono mt-0.5">
                          <Barcode className="h-2.5 w-2.5 inline mr-1" />
                          {u.code_barre}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{fmt(u.prix_reference)}</span>
                      <button
                        onClick={() => ouvrirModalEdition(a, u)}
                        title="Modifier"
                        className="text-muted-foreground hover:text-foreground
                                   transition-colors">
                        <Pencil className="h-3.5 w-3.5" />
                      </button>
                      {/* L'unité de base porte le stock et les états :
                          le serveur refuse de la désactiver, autant ne
                          pas proposer le bouton. */}
                      {u.facteur !== 1 && (
                        <button
                          onClick={() => handleDesactiverUnite(u)}
                          title="Retirer ce conditionnement"
                          className="text-muted-foreground hover:text-destructive transition-colors">
                          <XCircle className="h-3.5 w-3.5" />
                        </button>
                      )}
                    </div>
                  </div>
                ))}
                <Button size="sm" variant="outline" className="mt-2 h-7 text-xs"
                  onClick={() => ouvrirModalUnite(a)}>
                  <Plus className="h-3 w-3 mr-1" /> Conditionnement
                </Button>
              </div>
            )}
          </div>
        ))}
      </div>

      <Dialog open={!!modalUnite} onOpenChange={o => !o && setModalUnite(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>
              {uEdition ? "Modifier" : "Conditionnement"} — {modalUnite?.nom}
            </DialogTitle>
          </DialogHeader>
          <div className="space-y-3 pt-2">
            <div>
              <Label>Nom du conditionnement *</Label>
              <Input value={uLibelle} onChange={e => setULibelle(e.target.value)}
                placeholder="carton, sac 25kg, palette…" autoFocus className="mt-1" />
            </div>
            <div className="grid grid-cols-2 gap-2">
              {/* Facteur figé sur l'unité de base : le serveur refuse de
                  le changer, les états de stock la cherchent par
                  facteur = 1. */}
              <div>
                <Label>Combien de {modalUnite?.unite_base} *</Label>
                <Input value={uFacteur} onChange={e => setUFacteur(e.target.value)}
                  type="number" step="any" min="0" placeholder="12"
                  disabled={uEdition?.facteur === 1}
                  className="mt-1" />
              </div>
              <div>
                <Label>Prix de vente (F) *</Label>
                <MoneyInput value={uPrix} onChange={setUPrix}
                  placeholder="0" className="mt-1" />
              </div>
            </div>

            <div>
              <Label>Code-barres (optionnel)</Label>
              <Input value={uCodeBarre} onChange={e => setUCodeBarre(e.target.value)}
                placeholder="EAN du conditionnement"
                className="mt-1 font-mono text-sm" />
              <p className="text-xs text-muted-foreground mt-1">
                Le carton a souvent son propre code, différent de celui
                de l'unité. Scanné, il vend directement ce conditionnement.
              </p>
            </div>

            {/* Rendre l'incohérence visible AVANT de valider : le
                serveur avertit après coup, c'est trop tard au comptoir. */}
            {(() => {
              const f = parseFloat(uFacteur.replace(",", "."));
              const base = modalUnite?.unites.find(u => u.facteur === 1);
              if (!base || !(f > 0)) return null;
              const attendu = base.prix_reference * f;
              const saisi = parseMontant(uPrix) || 0;
              const ecart = saisi > 0 ? Math.abs(saisi - attendu) / attendu : 0;
              return (
                <p className={`text-xs ${ecart > 0.3
                  ? "text-orange-600 font-medium" : "text-muted-foreground"}`}>
                  {f} × {fmt(base.prix_reference)} = {fmt(attendu)}
                  {saisi > 0 && ecart > 0.3 && " — écart important, à vérifier"}
                </p>
              );
            })()}

            <p className="text-xs text-muted-foreground">
              Le nombre s'exprime toujours en {modalUnite?.unite_base}, jamais
              par rapport à un autre conditionnement.
            </p>

            <div className="flex gap-2 pt-1">
              <Button variant="outline" className="flex-1"
                onClick={() => setModalUnite(null)}>Annuler</Button>
              <Button className="flex-1" onClick={handleAjouterUnite}
                disabled={uEnCours || !uLibelle.trim() || !uFacteur || !uPrix}>
                {uEnCours
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : uEdition ? "Enregistrer" : "Ajouter"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={modalArticle} onOpenChange={setModalArticle}>
        <DialogContent className="max-w-sm">
          <DialogHeader><DialogTitle>Nouvel article</DialogTitle></DialogHeader>
          <div className="space-y-3 pt-2">
            <div>
              <Label>Nom *</Label>
              <Input value={nom} onChange={e => setNom(e.target.value)}
                placeholder="Nom de l'article" autoFocus className="mt-1" />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <Label>Unité de base</Label>
                {/* Même liste que le POS et les Achats. Les cinq valeurs
                    codées ici ("unite", "metre" sans accent) divergeaient
                    de celles créées ailleurs : deux orthographes pour la
                    même unité sur deux articles. */}
                <div className="mt-1">
                  <SelectUnite valeur={uniteBase} onChange={setUniteBase} />
                </div>
              </div>
              <div>
                <Label>Prix de vente (F)</Label>
                <MoneyInput value={prixVente} onChange={setPrixVente}
                  placeholder="0" className="mt-1" />
              </div>
            </div>
            <div>
              <Label>Catégorie</Label>
              <Select value={categorieId}
                onValueChange={v => { if (v) setCategorieId(v === "aucune" ? "" : v); }}>
                <SelectTrigger className="mt-1">
                  <SelectValue placeholder="Aucune">
                    {categories.find(c => c.id === categorieId)?.nom ?? "Aucune"}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="aucune">Aucune</SelectItem>
                  {categories.map(c => (
                    <SelectItem key={c.id} value={c.id}>{c.nom}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex gap-2 pt-1">
              <Button variant="outline" onClick={() => setModalArticle(false)} className="flex-1">
                Annuler
              </Button>
              <Button
                onClick={handleCreerArticle}
                disabled={!nom.trim() || !prixVente || chargementCreer}
                className="flex-1">
                {chargementCreer
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : "Créer"
                }
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// =====================================================================
//  Onglet : Catégories
// =====================================================================

function OngletCategories() {
  const [categories, setCategories] = useState<Categorie[]>([]);
  const [chargement, setChargement] = useState(true);
  const [nom, setNom] = useState("");
  const [chargementCreer, setChargementCreer] = useState(false);

  async function charger() {
    setChargement(true);
    try {
      const data = await invoke<Categorie[]>("lire_categories");
      setCategories(data);
    } catch (e) {
      console.error("Erreur catégories :", e);
    } finally {
      setChargement(false);
    }
  }

  useEffect(() => { charger(); }, []);

  async function handleCreer() {
    if (!nom.trim()) return;
    setChargementCreer(true);
    try {
      await invoke("creer_categorie", { nom: nom.trim() });
      setNom("");
      await charger();
    } catch (e) {
      await message(`Erreur : ${e}`, { title: "Erreur", kind: "error" });
    } finally {
      setChargementCreer(false);
    }
  }

  if (chargement) return (
    <div className="flex justify-center py-8">
      <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
    </div>
  );

  return (
    <div className="space-y-4 max-w-md">
      <div className="flex gap-2">
        <Input value={nom} onChange={e => setNom(e.target.value)}
          placeholder="Nouvelle catégorie" autoFocus
          onKeyDown={e => e.key === "Enter" && handleCreer()} />
        <Button onClick={handleCreer} disabled={!nom.trim() || chargementCreer}>
          {chargementCreer
            ? <Loader2 className="h-4 w-4 animate-spin" />
            : <Plus className="h-4 w-4" />
          }
        </Button>
      </div>
      <div className="space-y-1">
        {categories.length === 0 ? (
          <p className="text-sm text-muted-foreground text-center py-4">Aucune catégorie</p>
        ) : (
          categories.map(c => (
            <div key={c.id}
              className="flex items-center justify-between px-3 py-2
                         border border-border rounded-md">
              <span className="text-sm">{c.nom}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

// =====================================================================
//  Page Paramètres
// =====================================================================

export function Parametres() {
  const estPatron = UTILISATEUR_ACTIF?.role === "patron";
  const onglets = estPatron ? ONGLETS_PATRON : ONGLETS_EMPLOYE;
  const [onglet, setOnglet] = useState(onglets[0].key);

  return (
    <div className="flex-1 overflow-auto p-6">
      <h1 className="text-2xl font-semibold mb-6">Paramètres</h1>

      {/* Onglets */}
      <div className="flex gap-1 mb-6 border-b border-border flex-wrap">
        {onglets.map(o => {
          const Icone = o.icone;
          return (
            <button key={o.key}
              onClick={() => setOnglet(o.key)}
              className={`
                flex items-center gap-2 px-4 py-2 text-sm font-medium
                border-b-2 transition-colors
                ${onglet === o.key
                  ? "border-primary text-primary"
                  : "border-transparent text-muted-foreground hover:text-foreground"
                }
              `}>
              <Icone className="h-4 w-4" />
              {o.label}
            </button>
          );
        })}
      </div>

      {/* Contenu */}
      {onglet === "societe"       && <ParametresSociete />}
      {onglet === "depots"        && <OngletDepots />}
      {onglet === "codesbarres"   && <OngletCodesBarres />}
      {onglet === "importexport"  && <OngletImportExport />}
      {onglet === "articles"      && <OngletArticles />}
      {onglet === "categories"    && <OngletCategories />}
      {onglet === "ventes"        && <OngletVentes />}
      {onglet === "utilisateurs"  && <OngletUtilisateurs />}
      {onglet === "sauvegarde"    && <OngletSauvegarde />}
      {onglet === "tva"           && <OngletTVA />}
      {onglet === "dettes"        && <OngletDettes />}
      {onglet === "irrecouvrable" && <OngletIrrecouvrable />}
      {onglet === "avoirs"        && <OngletAvoirs />}
    </div>
  );
}