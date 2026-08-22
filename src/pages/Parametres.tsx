import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Package, Tag, Building2, Users, HardDrive,
  Plus, Loader2, Save, Eye, EyeOff, ShoppingCart,
  FolderOpen, ChevronDown, ChevronRight,
  Percent, Banknote, XCircle, Clock,
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
import { OngletTVA, OngletDettes, OngletIrrecouvrable, OngletAvoirs } from "@/components/OngletChantiers";
import { UTILISATEUR_ACTIF } from "@/App";

// =====================================================================
//  Types
// =====================================================================

interface Categorie { id: string; nom: string; }
interface Unite { id: string; libelle: string; facteur: number; prix_reference: number; }
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
  { key: "articles",      label: "Articles",      icone: Package    },
  { key: "categories",    label: "Catégories",    icone: Tag        },
  { key: "ventes",        label: "Ventes",        icone: ShoppingCart },
  { key: "utilisateurs",  label: "Utilisateurs",  icone: Users      },
  { key: "sauvegarde",    label: "Sauvegarde",    icone: HardDrive  },
  { key: "tva",           label: "TVA",           icone: Percent    },
  { key: "dettes",        label: "Dettes fourn.", icone: Banknote   },
  { key: "irrecouvrable", label: "Irrécouvrable", icone: XCircle    },
  { key: "avoirs",        label: "Avoirs",        icone: Clock      },
];

const ONGLETS_EMPLOYE = [
  { key: "articles",   label: "Articles",  icone: Package    },
  { key: "categories", label: "Catégories", icone: Tag       },
  { key: "ventes",     label: "Ventes",    icone: ShoppingCart },
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

  const [nom, setNom] = useState("");
  const [categorieId, setCategorieId] = useState("");
  const [uniteBase, setUniteBase] = useState("unite");
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
      setNom(""); setCategorieId(""); setUniteBase("unite"); setPrixVente("");
      setModalArticle(false);
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
                    className="flex justify-between text-sm py-1 border-b border-border last:border-0">
                    <div>
                      <span className="text-muted-foreground">{u.libelle}</span>
                      <span className="text-xs text-muted-foreground ml-2">
                        × {u.facteur} {a.unite_base}
                      </span>
                    </div>
                    <span className="font-medium">{fmt(u.prix_reference)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

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
                <Select value={uniteBase} onValueChange={v => { if (v) setUniteBase(v); }}>
                  <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="unite">Unité</SelectItem>
                    <SelectItem value="kg">Kg</SelectItem>
                    <SelectItem value="litre">Litre</SelectItem>
                    <SelectItem value="metre">Mètre</SelectItem>
                    <SelectItem value="carton">Carton</SelectItem>
                    <SelectItem value="sachet">Sachet</SelectItem>
                  </SelectContent>
                </Select>
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